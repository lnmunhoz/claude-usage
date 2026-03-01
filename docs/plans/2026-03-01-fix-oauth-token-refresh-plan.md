---
title: "fix: OAuth token refresh fails with 400 causing daily re-login"
type: fix
status: implemented
date: 2026-03-01
---

# fix: OAuth token refresh fails with 400 causing daily re-login

## Overview

After authenticating with Claude OAuth, the next day users get a 400 bad request error and must manually re-login. The root cause is that the token refresh request uses the wrong format (form-urlencoded instead of JSON) and is missing the required `client_id` field. Additionally, when the usage API rejects an expired token, there is no retry-with-refresh logic, so even a successful background refresh can't recover mid-request.

## Problem Statement

The `refresh_claude_token` function in `src-tauri/src/lib.rs:359-437` sends the refresh request as `application/x-www-form-urlencoded`:

```rust
// Current (broken) implementation — lib.rs:366-372
let response = client
    .post("https://platform.claude.com/v1/oauth/token")
    .header("Content-Type", "application/x-www-form-urlencoded")
    .body(format!(
        "grant_type=refresh_token&refresh_token={}",
        refresh_token
    ))
```

**Two problems:**
1. The Anthropic OAuth endpoint requires **JSON** (`Content-Type: application/json`), not form-urlencoded
2. The `client_id` field is missing — it is required for the refresh endpoint

The correct format, verified against multiple external sources:

```json
{
  "grant_type": "refresh_token",
  "refresh_token": "{refresh_token}",
  "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e"
}
```

The initial token exchange (`login_oauth`, lib.rs:996-1005) correctly uses `.json(...)` with `client_id`, but the refresh path was never written to match.

**Secondary issue:** When the usage API at `https://api.anthropic.com/api/oauth/usage` returns 401 due to an expired token, `fetch_claude_usage_impl` (lib.rs:473-609) returns the error immediately with no attempt to refresh and retry.

## External Verification

The refresh request format was verified against these external sources (not just project docs):

| Source | Endpoint | Format | Fields | `scope` required? |
|--------|----------|--------|--------|--------------------|
| [Anthropic OAuth CLI gist](https://gist.github.com/changjonathanc/9f9d635b2f8692e0520a884eaf098351) | `console.anthropic.com/v1/oauth/token` | JSON | `grant_type`, `refresh_token`, `client_id` | No |
| [Claude OAuth API guide](https://www.alif.web.id/posts/claude-oauth-api-key) | `console.anthropic.com/api/oauth/token` | JSON | `grant_type`, `refresh_token`, `client_id` | No |
| [GitHub issue #27933](https://github.com/anthropics/claude-code/issues/27933) | (internal) | JSON | confirms single-use refresh tokens | N/A |
| Project docs (`docs/anthropic-oauth-pkce.md:257-263`) | `platform.claude.com/v1/oauth/token` | JSON | `grant_type`, `refresh_token`, `client_id`, `scope` | Listed but not in external sources |

**Key findings:**
- **`scope` is NOT required** for token refresh — no external source includes it. Our project docs list it, but this appears to be incorrect. We'll omit it to match the verified pattern.
- **Refresh tokens are single-use** — after one is used, a new one is returned and the old is invalidated server-side.
- **Access tokens expire after ~8 hours** (not 1 hour).
- **Endpoint URL:** Our code uses `platform.claude.com/v1/oauth/token` which already works for the initial exchange. External sources reference `console.anthropic.com` variants — these are likely the same backend. We'll keep our existing URL since it works.

## Proposed Solution

Three targeted changes:

### 1. Fix token refresh request format (`lib.rs:366-376`)

Replace the form-urlencoded body with JSON, including `client_id`:

```rust
// Fixed implementation
let response = client
    .post("https://platform.claude.com/v1/oauth/token")
    .json(&serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": OAUTH_CLIENT_ID,
    }))
    .timeout(std::time::Duration::from_secs(15))
    .send()
    .await
    .map_err(|e| format!("Claude token refresh request failed: {}", e))?;
```

### 2. Add better error logging on refresh failure (`lib.rs:378-381`)

Currently the error response body is not read on failure:

```rust
// Current: only logs status code
if !status.is_success() {
    return Err(format!("Claude token refresh returned HTTP {}.", status));
}
```

Change to read and log the body. Note: this must happen *before* the existing `response.text().await` call at lib.rs:383, since `reqwest::Response::text()` consumes the response body.

```rust
if !status.is_success() {
    let body = response.text().await.unwrap_or_default();
    println!(
        "[claude-usage] Token refresh failed: HTTP {} — {}",
        status,
        &body[..body.len().min(500)]
    );
    return Err(format!(
        "Token refresh failed (HTTP {}): {}",
        status,
        &body[..body.len().min(200)]
    ));
}
```

### 3. Add retry-with-refresh on 401 from usage API (`lib.rs:497-526`)

When the usage API returns 401 (Unauthorized), force a token refresh and retry the request once.

**Integration approach:** The simplest way is to check for 401 right after receiving the response (before reading the body). On 401, refresh the token, re-run the request, and reassign `status`, `body`, and `access_token` so the rest of the function proceeds normally. This avoids restructuring the parse logic.

```rust
// In fetch_claude_usage_impl, after sending the request and getting `response`:
let status = response.status();

// On 401, refresh and retry before reading the body
let (status, body, access_token) = if status == reqwest::StatusCode::UNAUTHORIZED {
    println!("[claude-usage] Usage API returned 401, attempting token refresh and retry...");

    let oauth_blob = read_keychain_oauth_blob()
        .map_err(|e| format!("Re-auth failed: {}", e))?;
    let refresh_token = oauth_blob.refresh_token.as_deref()
        .ok_or_else(|| "No refresh token available for retry.".to_string())?;
    let refreshed = refresh_claude_token(refresh_token, &oauth_blob).await?;
    let new_token = refreshed.access_token.as_deref()
        .ok_or_else(|| "Refresh succeeded but no access token returned.".to_string())?
        .to_string();

    let retry_response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", new_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Retry request failed: {}", e))?;

    let retry_status = retry_response.status();
    let retry_body = retry_response.text().await
        .map_err(|e| format!("Failed to read retry response: {}", e))?;
    (retry_status, retry_body, new_token)
} else {
    let body = response.text().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    (status, body, access_token.to_string())
};

// Rest of the function uses status, body, access_token as before
```

This replaces the current body-read at lib.rs:492-495 and the status check at lib.rs:497. The remaining parse logic (lib.rs:528 onward) works unchanged since `status`, `body`, and `access_token` are the same variables it already uses.

### 4. Add debug logging throughout the token lifecycle

Add `println!` logging at key points so you can trace exactly what's happening when you run the app from the terminal (`cargo tauri dev`):

```rust
// In load_claude_credentials, after reading keychain (lib.rs:442-448):
println!(
    "[claude-usage] Token loaded: expires_at={:?}, has_refresh={}, now={}",
    oauth.expires_at,
    oauth.refresh_token.is_some(),
    now_ms()
);

// In load_claude_credentials, when token is still valid (lib.rs:463-468):
println!("[claude-usage] Token still valid, expires in {}s",
    (oauth.expires_at.unwrap_or(0) - now_ms()) / 1000
);

// In refresh_claude_token, on success (lib.rs:432):
println!(
    "[claude-usage] Token refreshed: new_expires_at={:?}, new_refresh={}",
    updated_blob.expires_at,
    updated_blob.refresh_token.is_some()
);

// In fetch_claude_usage_impl, before the API call (lib.rs:480):
println!("[claude-usage] Fetching usage with token starting with: {}...",
    &access_token[..access_token.len().min(20)]
);
```

### 5. Add a Tauri command to inspect token state (for testing)

Add a debug command that returns the current token's expiry info without exposing the actual token:

```rust
#[tauri::command]
fn debug_token_info() -> Result<serde_json::Value, String> {
    let blob = read_keychain_oauth_blob()?;
    let now = now_ms();
    Ok(serde_json::json!({
        "has_access_token": blob.access_token.is_some(),
        "has_refresh_token": blob.refresh_token.is_some(),
        "expires_at": blob.expires_at,
        "now": now,
        "expires_in_seconds": blob.expires_at.map(|e| (e - now) / 1000),
        "is_expired": blob.expires_at.map(|e| e <= now + 60_000),
    }))
}
```

Register it in the invoke handler (lib.rs:1069-1078) and call from the browser console: `window.__TAURI__.invoke('debug_token_info')`.

### 6. Add a `force_refresh` Tauri command for manual testing

Add a command that forces a token refresh regardless of expiry, so you can test the refresh flow on demand:

```rust
#[tauri::command]
async fn force_refresh_token() -> Result<serde_json::Value, String> {
    println!("[claude-usage] Force refresh triggered...");
    let blob = read_keychain_oauth_blob()?;
    let refresh_token = blob.refresh_token.as_deref()
        .ok_or_else(|| "No refresh token stored.".to_string())?;

    let result = refresh_claude_token(refresh_token, &blob).await?;

    let new_blob = read_keychain_oauth_blob()?;
    let now = now_ms();
    Ok(serde_json::json!({
        "success": true,
        "has_new_access_token": result.access_token.is_some(),
        "has_new_refresh_token": new_blob.refresh_token.is_some(),
        "new_expires_at": new_blob.expires_at,
        "expires_in_seconds": new_blob.expires_at.map(|e| (e - now) / 1000),
    }))
}
```

Register in invoke handler alongside `debug_token_info`. Call from browser console: `window.__TAURI__.invoke('force_refresh_token')`.

### 7. Add integration tests (`src-tauri/src/lib.rs`)

Add a `#[cfg(test)]` module at the bottom of `lib.rs`. These tests use real keychain credentials — you must be logged in first (`login_oauth`). Run with `cargo test -p claude-usage -- --ignored` (ignored by default so CI doesn't fail).

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Verify we can read a stored OAuth blob from the keychain.
    /// Requires: prior login via the app.
    #[test]
    #[ignore] // requires real keychain credentials
    fn test_read_keychain_blob() {
        let blob = read_keychain_oauth_blob()
            .expect("Should read OAuth blob from keychain — did you login first?");

        assert!(blob.access_token.is_some(), "Blob should have access_token");
        assert!(blob.refresh_token.is_some(), "Blob should have refresh_token");
        assert!(blob.expires_at.is_some(), "Blob should have expires_at");

        let token = blob.access_token.as_deref().unwrap();
        assert!(token.starts_with("sk-ant-oat"), "Access token should start with sk-ant-oat");

        println!("Access token prefix: {}...", &token[..20.min(token.len())]);
        println!("Has refresh token: {}", blob.refresh_token.is_some());
        println!("Expires at: {:?}", blob.expires_at);
        println!("Expires in: {}s", (blob.expires_at.unwrap() - now_ms()) / 1000);
    }

    /// Test the full token refresh flow against the real API.
    /// Requires: prior login with a valid refresh token.
    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_refresh_token_flow() {
        let blob = read_keychain_oauth_blob()
            .expect("Should read OAuth blob from keychain — did you login first?");

        let refresh_token = blob.refresh_token.as_deref()
            .expect("Blob should have a refresh token");

        println!("Refresh token prefix: {}...", &refresh_token[..20.min(refresh_token.len())]);

        let result = refresh_claude_token(refresh_token, &blob).await;

        match &result {
            Ok(creds) => {
                println!("Refresh succeeded!");
                assert!(creds.access_token.is_some(), "Should get new access token");
                let new_token = creds.access_token.as_deref().unwrap();
                assert!(new_token.starts_with("sk-ant-oat"), "New token format valid");
                println!("New token prefix: {}...", &new_token[..20.min(new_token.len())]);

                // Verify the keychain was updated
                let updated_blob = read_keychain_oauth_blob()
                    .expect("Should still be able to read keychain");
                assert!(updated_blob.refresh_token.is_some(),
                    "Keychain should have new refresh token (rotation)");
                println!("New expires_at: {:?}", updated_blob.expires_at);
            }
            Err(e) => {
                panic!("Token refresh failed: {}", e);
            }
        }
    }

    /// Test that a refreshed token works for the usage API.
    /// Requires: prior login + network.
    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_refreshed_token_fetches_usage() {
        // First refresh the token to ensure we have a fresh one
        let blob = read_keychain_oauth_blob()
            .expect("Should read OAuth blob from keychain");
        let refresh_token = blob.refresh_token.as_deref()
            .expect("Blob should have a refresh token");

        let creds = refresh_claude_token(refresh_token, &blob).await
            .expect("Refresh should succeed");
        let access_token = creds.access_token.as_deref()
            .expect("Should have new access token");

        // Now use the refreshed token to fetch usage
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.anthropic.com/api/oauth/usage")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("anthropic-beta", "oauth-2025-04-20")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .expect("Usage API request should succeed");

        let status = response.status();
        let body = response.text().await.expect("Should read response body");

        println!("Usage API status: {}", status);
        println!("Usage API body: {}", &body[..body.len().min(500)]);

        assert!(status.is_success(),
            "Usage API should return 200 with refreshed token, got {}: {}",
            status, &body[..body.len().min(200)]
        );
    }

    /// Test that the full fetch_claude_usage_impl works end-to-end.
    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_fetch_usage_end_to_end() {
        let result = fetch_claude_usage_impl().await;

        match &result {
            Ok(data) => {
                println!("Usage fetch succeeded!");
                println!("Session: {:.1}%", data.session_percent_used);
                println!("Weekly: {:.1}%", data.weekly_percent_used);
                println!("Session reset: {:?}", data.session_reset);
                println!("Weekly reset: {:?}", data.weekly_reset);
                println!("Plan type: {:?}", data.plan_type);
            }
            Err(e) => {
                panic!("Usage fetch failed: {}", e);
            }
        }
    }
}
```

### Testing Strategy

**Run order** (each test requires the previous to pass):

1. `test_read_keychain_blob` — Verifies keychain has credentials. If this fails, you need to login first.
2. `test_refresh_token_flow` — Tests the core fix: JSON format, `client_id` included. **This is the critical test.** If this passes, the refresh format is correct.
3. `test_refreshed_token_fetches_usage` — Verifies the refreshed token actually works against the usage API.
4. `test_fetch_usage_end_to_end` — Full integration test of the entire flow.

**How to run:**

```bash
# Run all integration tests (requires prior login)
# IMPORTANT: --test-threads=1 is required because refresh tokens are single-use.
# Running tests in parallel would cause the second test to use an invalidated token.
cd src-tauri && cargo test -- --ignored --nocapture --test-threads=1

# Run just the refresh test
cd src-tauri && cargo test test_refresh_token_flow -- --ignored --nocapture
```

**Note:** `--nocapture` shows the `println!` output so you can see token prefixes, expiry times, and API responses.

**Additional manual tests:**

1. **Force refresh from the app**: Run `cargo tauri dev`, open console, run `window.__TAURI__.invoke('force_refresh_token')`. Verifies the fix works in the real Tauri context.
2. **Test retry on 401**: Temporarily corrupt the access token in the keychain, then trigger a poll. The debug logs should show the 401 retry path.
3. **Overnight test**: Leave the app running and check terminal logs the next morning to confirm automatic refresh worked.

## Technical Considerations

- **No concurrency guard needed for MVP:** The polling interval is typically 60s and the refresh takes <1s. Concurrent refreshes are unlikely. If this becomes an issue (refresh tokens are single-use, so races could invalidate tokens), a `tokio::sync::Mutex` can be added later.
- **Only retry on 401, not 400:** A 400 from the usage API could mean many things. Only 401 clearly indicates an auth problem.
- **Profile endpoint not retried:** The `/api/oauth/profile` call (lib.rs:581-598) already swallows errors and returns `None`. Missing plan type is acceptable.
- **No frontend changes:** The retry happens server-side. No changes needed to `App.tsx`.

## Acceptance Criteria

- [x] `refresh_claude_token` sends JSON body with `grant_type`, `refresh_token`, and `client_id`
- [x] `refresh_claude_token` logs the response body on failure (truncated to 500 chars)
- [x] `fetch_claude_usage_impl` retries once with a fresh token when usage API returns 401
- [x] Retry does NOT trigger on 400, 403, 429, or other non-401 status codes
- [x] Debug logging shows token expiry, refresh attempts, and API call results in terminal
- [x] `debug_token_info` command returns current token state without exposing secrets
- [x] `force_refresh_token` command triggers a real token refresh and returns results
- [x] `test_refresh_token_flow` integration test passes (refresh with JSON format + `client_id`)
- [x] `test_refreshed_token_fetches_usage` integration test passes (refreshed token works for API)
- [ ] Token persists across overnight sessions without requiring manual re-login
- [ ] The app correctly recovers when opened after the access token has expired
- [x] Existing login flow (`login_oauth`) is not affected

## Success Metrics

- Users no longer need to re-login daily
- Token refresh succeeds silently in the background
- Refresh failure logs include the HTTP status and response body for debugging

## Dependencies & Risks

- **API contract:** The fix is verified against multiple external sources showing the JSON format with `client_id`. The risk of the format being wrong is low.
- **Refresh token validity:** If the refresh token itself has a limited lifetime (e.g., 30 days), users will still need to re-login periodically. This fix only addresses the daily expiry of the ~8-hour access token.
- **`scope` omitted:** External sources consistently omit `scope` from the refresh request. If the API requires it, the improved error logging will show the exact failure and we can add it back quickly.

## Files to Modify

| File | Changes |
|------|---------|
| `src-tauri/src/lib.rs:366-376` | Fix refresh request to use JSON with `client_id` |
| `src-tauri/src/lib.rs:378-381` | Add response body to refresh error logging |
| `src-tauri/src/lib.rs:442-468` | Add debug logging in `load_claude_credentials` |
| `src-tauri/src/lib.rs:432` | Add debug logging in `refresh_claude_token` on success |
| `src-tauri/src/lib.rs:480` | Add debug logging before usage API call |
| `src-tauri/src/lib.rs:497-526` | Add retry-with-refresh on 401 from usage API |
| `src-tauri/src/lib.rs` (new fn) | Add `debug_token_info` command + register in invoke handler |
| `src-tauri/src/lib.rs` (new fn) | Add `force_refresh_token` command + register in invoke handler |
| `src-tauri/src/lib.rs` (new mod) | Add `#[cfg(test)] mod tests` with 4 integration tests |

## Sources & References

### External (verified)
- [Anthropic OAuth CLI implementation (gist)](https://gist.github.com/changjonathanc/9f9d635b2f8692e0520a884eaf098351) — shows JSON format with `client_id`, no `scope`
- [Claude OAuth API key guide](https://www.alif.web.id/posts/claude-oauth-api-key) — confirms JSON format, `client_id` required, 8-hour token lifetime
- [Claude Code OAuth race condition issue #27933](https://github.com/anthropics/claude-code/issues/27933) — confirms single-use refresh tokens
- [Claude Code OAuth expiration issue #12447](https://github.com/anthropics/claude-code/issues/12447) — confirms tokens expire and need refresh
- [Claude Code authentication docs](https://code.claude.com/docs/en/authentication) — confirms keychain storage, refresh on 401

### Internal
- Token refresh format: `docs/anthropic-oauth-pkce.md:257-263`
- Initial token exchange (correct pattern): `src-tauri/src/lib.rs:996-1005`
- Current broken refresh: `src-tauri/src/lib.rs:366-376`
- Usage API error handling: `src-tauri/src/lib.rs:497-526`
