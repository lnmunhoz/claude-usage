mod commands;
mod keychain;
mod models;
mod oauth;
mod poller;
mod settings;
mod tray;
mod updater;
mod usage;

use std::sync::Mutex;

use tauri::Manager;

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
        .manage(Mutex::new(None::<poller::PollerHandle>))
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

            // Start background polling if user already has a token
            if keychain::read_keychain_oauth_blob().is_ok() {
                let settings = app.state::<Mutex<models::Settings>>();
                let interval = settings.lock().unwrap().poll_interval_seconds;
                let handle = poller::start_poller(app.handle().clone(), interval);
                let poller_state = app.state::<Mutex<Option<poller::PollerHandle>>>();
                *poller_state.lock().unwrap() = Some(handle);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use crate::keychain::read_keychain_oauth_blob;
    use crate::oauth::refresh_claude_token;

    /// Integration test: refreshed token works for the usage API.
    /// Exercises oauth + keychain + usage API together.
    /// Requires: prior login + network.
    #[tokio::test]
    #[ignore] // requires real keychain credentials + network
    async fn test_refreshed_token_fetches_usage() {
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
}
