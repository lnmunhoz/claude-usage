mod commands;
mod keychain;
mod models;
mod oauth;
mod settings;
mod tray;
mod updater;
mod usage;

use std::sync::Mutex;

use settings::load_settings;

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(load_settings()))
        .invoke_handler(tauri::generate_handler![
            commands::fetch_claude_usage,
            commands::save_token,
            commands::has_token,
            commands::clear_token,
            commands::get_settings,
            commands::save_poll_interval,
            commands::login_oauth,
            commands::update_tray_title,
            commands::debug_token_info,
            commands::force_refresh_token
        ])
        .setup(|app| {
            tray::setup_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    use keychain::read_keychain_oauth_blob;
    use oauth::refresh_claude_token;
    use usage::fetch_claude_usage_impl;

    /// Verify we can read a stored OAuth blob from the keychain.
    /// Requires: prior login via the app.
    #[test]
    #[ignore] // requires real keychain credentials
    fn test_read_keychain_blob() {
        let blob = read_keychain_oauth_blob()
            .expect("Should read OAuth blob from keychain — did you login first?");

        assert!(blob.access_token.is_some(), "Blob should have access_token");
        assert!(
            blob.refresh_token.is_some(),
            "Blob should have refresh_token"
        );
        assert!(blob.expires_at.is_some(), "Blob should have expires_at");

        let token = blob.access_token.as_deref().unwrap();
        assert!(
            token.starts_with("sk-ant-oat"),
            "Access token should start with sk-ant-oat"
        );

        println!(
            "Access token prefix: {}...",
            &token[..20.min(token.len())]
        );
        println!("Has refresh token: {}", blob.refresh_token.is_some());
        println!("Expires at: {:?}", blob.expires_at);
        println!(
            "Expires in: {}s",
            (blob.expires_at.unwrap() - now_ms()) / 1000
        );
    }

    /// Test the full token refresh flow against the real API.
    /// Requires: prior login with a valid refresh token.
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

    /// Test that a refreshed token works for the usage API.
    /// Requires: prior login + network.
    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_refreshed_token_fetches_usage() {
        // First refresh the token to ensure we have a fresh one
        let blob = read_keychain_oauth_blob().expect("Should read OAuth blob from keychain");
        let refresh_token = blob
            .refresh_token
            .as_deref()
            .expect("Blob should have a refresh token");

        let creds = refresh_claude_token(refresh_token, &blob)
            .await
            .expect("Refresh should succeed");
        let access_token = creds
            .access_token
            .as_deref()
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

        assert!(
            status.is_success(),
            "Usage API should return 200 with refreshed token, got {}: {}",
            status,
            &body[..body.len().min(200)]
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
