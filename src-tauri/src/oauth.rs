use base64::Engine;
use rand::Rng;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::net::TcpListener;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::keychain::{read_keychain_oauth_blob, validate_claude_oauth_access_token, write_keychain_oauth_blob};
use crate::models::{ClaudeCredentials, ClaudeOAuthBlob, OAuthTokenResponse};
use crate::now_ms;

pub(crate) const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

fn generate_code_verifier() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash)
}

pub(crate) async fn refresh_claude_token(
    refresh_token: &str,
    blob: &ClaudeOAuthBlob,
) -> Result<ClaudeCredentials, String> {
    println!("[claude-usage] Claude auth: attempting OAuth token refresh...");

    let client = reqwest::Client::new();
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

    let status = response.status();
    if !status.is_success() {
        let err_body = response.text().await.unwrap_or_default();
        println!(
            "[claude-usage] Token refresh failed: HTTP {} — {}",
            status,
            &err_body[..err_body.len().min(500)]
        );
        return Err(format!(
            "Token refresh failed (HTTP {}): {}",
            status,
            &err_body[..err_body.len().min(200)]
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read token refresh response: {}", e))?;

    let token_resp: OAuthTokenResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse token refresh response: {}", e))?;

    // Try to extract rate_limit_tier from the full refresh response
    let refresh_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let refreshed_tier = refresh_value
        .get("rate_limit_tier")
        .or_else(|| refresh_value.get("rateLimitTier"))
        .or_else(|| refresh_value.get("membership_type"))
        .or_else(|| refresh_value.get("subscription_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let new_access_token = token_resp
        .access_token
        .ok_or_else(|| "Token refresh response missing access_token.".to_string())?;
    validate_claude_oauth_access_token(&new_access_token, "refreshed token")?;

    let now = now_ms();
    let new_expires_at = token_resp
        .expires_in
        .map(|secs| now + secs * 1000)
        .or(blob.expires_at);

    let new_refresh = token_resp
        .refresh_token
        .unwrap_or_else(|| refresh_token.to_string());

    let mut updated_blob = blob.clone();
    updated_blob.access_token = Some(new_access_token.clone());
    updated_blob.refresh_token = Some(new_refresh);
    updated_blob.expires_at = new_expires_at;
    // Prefer freshly-returned tier; fall back to what was already stored
    if refreshed_tier.is_some() {
        updated_blob.rate_limit_tier = refreshed_tier;
    }

    if let Err(e) = write_keychain_oauth_blob(&updated_blob) {
        println!(
            "[claude-usage] Claude auth: warning - could not update keychain ({}). Token will work for this session only.",
            e
        );
    }

    println!(
        "[claude-usage] Token refreshed: new_expires_at={:?}, new_refresh={}",
        updated_blob.expires_at,
        updated_blob.refresh_token.is_some()
    );
    Ok(ClaudeCredentials {
        access_token: Some(new_access_token),
        rate_limit_tier: updated_blob.rate_limit_tier,
    })
}

pub(crate) async fn load_claude_credentials() -> Result<ClaudeCredentials, String> {
    let oauth = read_keychain_oauth_blob()?;

    println!(
        "[claude-usage] Token loaded: expires_at={:?}, has_refresh={}, now={}",
        oauth.expires_at,
        oauth.refresh_token.is_some(),
        now_ms()
    );

    let access_token = oauth
        .access_token
        .as_deref()
        .ok_or_else(|| "Keychain entry missing access token.".to_string())?;
    validate_claude_oauth_access_token(access_token, "keychain")?;

    // Check expiry — if expired, attempt refresh
    if let Some(expires_at_ms) = oauth.expires_at {
        let now = now_ms();
        if expires_at_ms <= now + 60_000 {
            println!("[claude-usage] Claude auth: token expired, attempting refresh...");
            if let Some(ref refresh_token) = oauth.refresh_token {
                return refresh_claude_token(refresh_token, &oauth).await;
            } else {
                return Err(
                    "Claude token is expired and no refresh token available.".to_string(),
                );
            }
        }
        println!(
            "[claude-usage] Token still valid, expires in {}s",
            (expires_at_ms - now) / 1000
        );
    }

    Ok(ClaudeCredentials {
        access_token: Some(access_token.to_string()),
        rate_limit_tier: oauth.rate_limit_tier,
    })
}

pub(crate) async fn login_oauth_impl(app_handle: AppHandle) -> Result<(), String> {
    let verifier = generate_code_verifier();
    let challenge = generate_code_challenge(&verifier);

    // Bind to a random available port
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local server: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?
        .port();

    let redirect_uri = format!("http://localhost:{}/callback", port);

    // Generate state parameter for CSRF protection
    let state = generate_code_verifier(); // reuse the same random generator

    // Build authorization URL
    let mut auth_url = Url::parse("https://claude.ai/oauth/authorize")
        .map_err(|e| format!("Failed to parse auth URL: {}", e))?;
    auth_url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", OAUTH_CLIENT_ID)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", "user:profile user:inference")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);

    // Open browser
    app_handle
        .opener()
        .open_url(auth_url.as_str(), None::<&str>)
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    // Set timeout on the listener
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("Failed to configure listener: {}", e))?;

    // Wait for callback with timeout
    let redirect_uri_clone = redirect_uri.clone();
    let expected_state = state.clone();
    let (code, callback_stream) = tokio::task::spawn_blocking(move || {
        listener
            .set_nonblocking(false)
            .map_err(|e| format!("Failed to configure listener: {}", e))?;

        // Use a polling approach with timeout
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return Err(
                            "Login timed out. Please try again.".to_string()
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(format!("Failed to accept connection: {}", e)),
            }
        };

        // Read the HTTP request
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| format!("Failed to read request: {}", e))?;

        // Parse the request URL to extract the code
        let path = request_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| "Invalid HTTP request".to_string())?;

        let full_url = format!("http://localhost{}", path);
        let parsed = Url::parse(&full_url)
            .map_err(|e| format!("Failed to parse callback URL: {}", e))?;

        // Check for error parameter
        if let Some(error) = parsed.query_pairs().find(|(k, _)| k == "error") {
            let error_desc = parsed
                .query_pairs()
                .find(|(k, _)| k == "error_description")
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| error.1.to_string());
            return Err(format!("Authorization denied: {}", error_desc));
        }

        // Validate state parameter
        let returned_state = parsed
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| "Missing state parameter in callback.".to_string())?;
        if returned_state != expected_state {
            return Err("State parameter mismatch — possible CSRF attack.".to_string());
        }

        let code = parsed
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| "No authorization code received.".to_string())?;

        // Drain remaining headers before writing response
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.trim().is_empty() {
                        break;
                    }
                }
            }
        }

        Ok((code, stream))
    })
    .await
    .map_err(|e| format!("Callback task failed: {}", e))?
    .map_err(|e: String| e)?;

    // Send success response to browser before exchanging the code
    let _ = tokio::task::spawn_blocking(move || {
        let html = r#"<!DOCTYPE html>
<html><head><title>Claude Usage</title>
<style>body{font-family:-apple-system,system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#1c1c1e;color:#fff}
.container{text-align:center}.check{font-size:48px;margin-bottom:16px}h1{font-size:20px;font-weight:600;margin-bottom:8px}p{color:rgba(255,255,255,0.5);font-size:14px}</style>
</head><body><div class="container"><div class="check">&#10003;</div><h1>Connected!</h1><p>You can close this tab and return to Claude Usage.</p></div></body></html>"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let mut stream = callback_stream;
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    })
    .await;

    // Exchange authorization code for tokens
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://platform.claude.com/v1/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri_clone,
            "code_verifier": verifier,
            "client_id": OAUTH_CLIENT_ID,
            "state": state,
        }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Token exchange request failed: {}", e))?;

    let status = token_response.status();
    let body = token_response
        .text()
        .await
        .map_err(|e| format!("Failed to read token response: {}", e))?;

    if !status.is_success() {
        println!("[claude-usage] Token exchange failed: HTTP {} — {}", status, &body[..body.len().min(500)]);
        return Err(format!("Token exchange failed (HTTP {}). Please try again.", status));
    }

    let token_data: OAuthTokenResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    // Try to extract rate_limit_tier from the full token response
    let token_value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let rate_limit_tier = token_value
        .get("rate_limit_tier")
        .or_else(|| token_value.get("rateLimitTier"))
        .or_else(|| token_value.get("membership_type"))
        .or_else(|| token_value.get("subscription_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let access_token = token_data
        .access_token
        .ok_or_else(|| "Token response missing access_token.".to_string())?;

    let now = now_ms();
    let expires_at = token_data.expires_in.map(|secs| now + secs * 1000);

    let blob = ClaudeOAuthBlob {
        access_token: Some(access_token),
        refresh_token: token_data.refresh_token,
        expires_at,
        scopes: Some(vec!["user:profile".to_string(), "user:inference".to_string()]),
        subscription_type: None,
        rate_limit_tier,
    };

    write_keychain_oauth_blob(&blob)?;
    println!("[claude-usage] OAuth login succeeded, token saved to keychain.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keychain::read_keychain_oauth_blob;

    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_refresh_token_flow() {
        let blob = read_keychain_oauth_blob()
            .expect("Should read OAuth blob from keychain — did you login first?");

        let refresh_token = blob
            .refresh_token
            .as_deref()
            .expect("Blob should have a refresh token");

        println!(
            "Refresh token prefix: {}...",
            &refresh_token[..20.min(refresh_token.len())]
        );

        let result = refresh_claude_token(refresh_token, &blob).await;

        match &result {
            Ok(creds) => {
                println!("Refresh succeeded!");
                assert!(creds.access_token.is_some(), "Should get new access token");
                let new_token = creds.access_token.as_deref().unwrap();
                assert!(
                    new_token.starts_with("sk-ant-oat"),
                    "New token format valid"
                );
                println!(
                    "New token prefix: {}...",
                    &new_token[..20.min(new_token.len())]
                );

                // Verify the keychain was updated
                let updated_blob = read_keychain_oauth_blob()
                    .expect("Should still be able to read keychain");
                assert!(
                    updated_blob.refresh_token.is_some(),
                    "Keychain should have new refresh token (rotation)"
                );
                println!("New expires_at: {:?}", updated_blob.expires_at);
            }
            Err(e) => {
                panic!("Token refresh failed: {}", e);
            }
        }
    }
}
