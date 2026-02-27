# Anthropic OAuth PKCE Flow — Implementation Reference

This document describes the complete OAuth 2.0 PKCE (Proof Key for Code Exchange) flow used to authenticate with Anthropic's API and obtain tokens with the correct scopes for the usage API.

This was reverse-engineered from Claude Code's (`@anthropic-ai/claude-code`) bundled source and validated through trial and error during Claude Usage development.

---

## Endpoints

| Purpose            | URL                                                  |
| ------------------ | ---------------------------------------------------- |
| Authorization      | `https://claude.ai/oauth/authorize`                  |
| Token Exchange     | `https://platform.claude.com/v1/oauth/token`         |
| Usage API          | `https://api.anthropic.com/api/oauth/usage`          |

> **Important:** The token endpoint was previously `https://console.anthropic.com/v1/oauth/token` but has been moved to `https://platform.claude.com/v1/oauth/token`. The old endpoint may redirect or return errors.

## Client ID

Claude Code's public OAuth client ID (reusable for third-party apps):

```
9d1c250a-e61b-44d9-88ed-5944d1962f5e
```

## Required Scopes

```
user:profile user:inference
```

- `user:profile` — Required by the usage API (`/api/oauth/usage`). Without this scope, the API returns **403 Forbidden**.
- `user:inference` — Required for inference access.

Claude Code itself requests additional scopes: `user:sessions:claude_code user:mcp_servers`. These are not needed for usage tracking.

---

## Flow Overview

```
┌─────────┐          ┌─────────┐          ┌──────────────┐
│  App    │          │ Browser │          │ Anthropic    │
└────┬────┘          └────┬────┘          └──────┬───────┘
     │                    │                      │
     │ 1. Generate PKCE   │                      │
     │    verifier +      │                      │
     │    challenge +     │                      │
     │    state           │                      │
     │                    │                      │
     │ 2. Bind ephemeral  │                      │
     │    localhost port   │                      │
     │                    │                      │
     │ 3. Open browser ──►│                      │
     │    with auth URL   │ 4. GET /authorize ──►│
     │                    │                      │
     │                    │    5. User logs in    │
     │                    │       and authorizes  │
     │                    │                      │
     │                    │◄── 6. 302 redirect ──│
     │                    │    to localhost       │
     │                    │    ?code=...&state=.. │
     │                    │                      │
     │◄── 7. Receive ────│                      │
     │    callback on     │                      │
     │    local server    │                      │
     │                    │                      │
     │ 8. Validate state  │                      │
     │                    │                      │
     │ 9. Send HTML ─────►│                      │
     │    "You can close" │                      │
     │                    │                      │
     │ 10. POST /token ──────────────────────────►│
     │     (exchange code)│                      │
     │                    │                      │
     │◄────────────────── 11. access_token ──────│
     │                        refresh_token      │
     │                        expires_in         │
     │                    │                      │
     │ 12. Save to        │                      │
     │     keychain       │                      │
     └───────────────────────────────────────────┘
```

---

## Step-by-Step Details

### Step 1: Generate PKCE Parameters

Generate three random values:

- **`code_verifier`**: 32 random bytes, base64url-encoded (no padding). Used to prove we initiated the flow.
- **`code_challenge`**: SHA-256 hash of the verifier, base64url-encoded (no padding).
- **`state`**: Another 32 random bytes, base64url-encoded. CSRF protection — the authorization server will echo this back.

```rust
fn generate_code_verifier() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash)
}
```

### Step 2: Bind Ephemeral Localhost Port

Bind a TCP listener on `127.0.0.1:0` to get a random available port. This becomes the redirect URI:

```
http://localhost:{port}/callback
```

### Step 3: Build & Open Authorization URL

Open the user's default browser with:

```
https://claude.ai/oauth/authorize
  ?response_type=code
  &client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e
  &redirect_uri=http://localhost:{port}/callback
  &scope=user:profile%20user:inference
  &code_challenge={challenge}
  &code_challenge_method=S256
  &state={state}
```

**Required parameters:**

| Parameter               | Value                                          |
| ----------------------- | ---------------------------------------------- |
| `response_type`         | `code`                                         |
| `client_id`             | `9d1c250a-e61b-44d9-88ed-5944d1962f5e`         |
| `redirect_uri`          | `http://localhost:{port}/callback`              |
| `scope`                 | `user:profile user:inference`                   |
| `code_challenge`        | Base64url SHA-256 of verifier                  |
| `code_challenge_method` | `S256`                                         |
| `state`                 | Random base64url string                        |

> **Gotcha:** The `state` parameter is **required**. Omitting it produces an "Invalid OAuth Request — Missing state parameter" error page.

### Step 4–6: User Authorizes in Browser

The user sees the Anthropic login/consent screen. After authorizing, the browser redirects to:

```
http://localhost:{port}/callback?code={authorization_code}&state={state}
```

If the user denies, the callback includes `error` and `error_description` query parameters instead.

### Step 7–8: Receive & Validate Callback

The local HTTP server accepts the connection, reads the HTTP request line, and extracts query parameters.

**Validation steps:**
1. Check for `error` parameter — if present, abort with the error message.
2. Validate `state` matches the value sent in step 3 — if not, abort (CSRF protection).
3. Extract the `code` parameter.

### Step 9: Respond to Browser

Send back an HTML page telling the user they can close the tab:

```http
HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8
Content-Length: {len}
Connection: close

<!DOCTYPE html>
<html>...You can close this tab...</html>
```

### Step 10: Exchange Code for Tokens

**POST** to `https://platform.claude.com/v1/oauth/token`

**Content-Type:** `application/json` (NOT `application/x-www-form-urlencoded`)

**Body (JSON):**

```json
{
  "grant_type": "authorization_code",
  "code": "{authorization_code}",
  "redirect_uri": "http://localhost:{port}/callback",
  "code_verifier": "{verifier}",
  "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
  "state": "{state}"
}
```

**Required fields:**

| Field            | Description                                     |
| ---------------- | ----------------------------------------------- |
| `grant_type`     | Must be `authorization_code`                    |
| `code`           | The authorization code from the callback        |
| `redirect_uri`   | Must exactly match the one used in the auth URL |
| `code_verifier`  | The original PKCE verifier (not the challenge)  |
| `client_id`      | The OAuth client ID                             |
| `state`          | The state parameter from the auth request       |

> **Gotcha:** The body must be **JSON**. Sending `application/x-www-form-urlencoded` returns HTTP 400 "Invalid request format".

> **Gotcha:** The `state` field is **required** in the token exchange body. Omitting it returns HTTP 400 "Invalid request format".

### Step 11: Parse Token Response

**Success response (200 OK):**

```json
{
  "access_token": "sk-ant-oat...",
  "refresh_token": "sk-ant-ort...",
  "expires_in": 3600
}
```

| Field            | Type    | Description                              |
| ---------------- | ------- | ---------------------------------------- |
| `access_token`   | string  | OAuth access token (starts with `sk-ant-oat`) |
| `refresh_token`  | string  | Used to get new access tokens            |
| `expires_in`     | integer | Token lifetime in seconds                |

### Step 12: Store Credentials

Save to macOS Keychain (or platform equivalent) as a JSON blob:

```json
{
  "accessToken": "sk-ant-oat...",
  "refreshToken": "sk-ant-ort...",
  "expiresAt": 1709123456000,
  "scopes": ["user:profile", "user:inference"]
}
```

Where `expiresAt` = current timestamp in milliseconds + (`expires_in` * 1000).

---

## Token Refresh

When the access token expires, use the refresh token to get a new one.

**POST** to `https://platform.claude.com/v1/oauth/token`

Claude Code sends refresh as JSON with these fields:

```json
{
  "grant_type": "refresh_token",
  "refresh_token": "{refresh_token}",
  "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
  "scope": "user:profile user:inference"
}
```

> **Note:** Our current implementation uses `application/x-www-form-urlencoded` for refresh and it works. Claude Code uses JSON. Both may be accepted, but JSON is the canonical format.

The response has the same shape as the initial token exchange.
After refreshing, update the stored credentials in keychain with the new access token, refresh token (if rotated), and expiry.

---

## Using the Access Token (Usage API)

**GET** `https://api.anthropic.com/api/oauth/usage`

**Headers:**

```http
Accept: application/json
Authorization: Bearer {access_token}
anthropic-beta: oauth-2025-04-20
```

**Response:**

```json
{
  "five_hour": {
    "utilization": 12.5,
    "reset_at": "2025-03-01T12:00:00Z"
  },
  "seven_day": {
    "utilization": 45.0,
    "reset_at": "2025-03-05T00:00:00Z"
  },
  "extra_usage": {
    "is_enabled": true,
    "used_credits": 2.50,
    "monthly_limit": 100.00
  }
}
```

The `utilization` field is a percentage (0–100) representing how much of the rate limit has been used.

---

## Common Errors & Gotchas

| Error                                          | Cause                                                        | Fix                                                    |
| ---------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------ |
| "Missing state parameter" (auth page)          | `state` query param not included in authorization URL        | Add `state` parameter to the auth URL                  |
| HTTP 400 "Invalid request format" (token)      | Token exchange body is form-encoded instead of JSON, or missing `state` field | Use `Content-Type: application/json` and include `state` |
| HTTP 403 "scope requirement user:profile"      | Token was created without `user:profile` scope               | Re-authenticate with `scope=user:profile user:inference` |
| Token exchange succeeds but token doesn't work | Using old `console.anthropic.com` endpoint                   | Use `platform.claude.com` endpoint                     |

---

## Keychain Details (macOS)

| Field   | Value                    |
| ------- | ------------------------ |
| Service | `app.claudeusage.desktop`     |
| Account | `claude-oauth`           |

To manually clear a stored token:

```bash
security delete-generic-password -s "app.claudeusage.desktop" -a "claude-oauth"
```

---

## References

- Claude Code source: `@anthropic-ai/claude-code` npm package (cli.js bundle)
- OAuth 2.0 PKCE: [RFC 7636](https://tools.ietf.org/html/rfc7636)
- OAuth 2.0 Authorization Code Grant: [RFC 6749 Section 4.1](https://tools.ietf.org/html/rfc6749#section-4.1)
