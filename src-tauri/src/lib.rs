mod models;
mod settings;
mod keychain;
mod oauth;
mod usage;
mod updater;

use models::*;
use settings::*;
use keychain::*;
use oauth::*;
use usage::*;
use updater::*;
use rand::Rng;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_positioner::{Position, WindowExt as PositionerExt};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

// Wrapper so we can manage TrayIcon separately from Settings
struct TrayState(TrayIcon);

// Debounce: prevent re-showing panel right after focus-loss hide
static PANEL_HIDDEN_AT_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// --- Tauri Commands ---

#[tauri::command]
async fn fetch_claude_usage() -> Result<ClaudeUsageData, String> {
    fetch_claude_usage_impl().await
}

#[tauri::command]
async fn save_token(input: SaveTokenInput) -> Result<(), String> {
    validate_claude_oauth_access_token(&input.access_token, "user input")?;
    let blob = ClaudeOAuthBlob {
        access_token: Some(input.access_token),
        refresh_token: input.refresh_token,
        expires_at: input.expires_at,
        scopes: None,
        subscription_type: None,
        rate_limit_tier: None,
    };
    write_keychain_oauth_blob(&blob)
}

#[tauri::command]
fn has_token() -> bool {
    read_keychain_oauth_blob().is_ok()
}

#[tauri::command]
fn clear_token() -> Result<(), String> {
    delete_keychain_oauth_blob()
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Mutex<Settings>>) -> Settings {
    state.lock().unwrap().clone()
}

#[tauri::command]
fn save_poll_interval(
    interval_value: u64,
    interval_unit: String,
    app_handle: AppHandle,
    state: tauri::State<'_, Mutex<Settings>>,
) -> Result<(), String> {
    let multiplier: u64 = match interval_unit.as_str() {
        "minutes" => 60,
        "hours" => 3600,
        _ => 1,
    };
    let total_seconds = (interval_value * multiplier).max(10);

    let mut settings = state.lock().unwrap();
    settings.poll_interval_seconds = total_seconds;
    save_settings(&settings);
    let _ = app_handle.emit("settings-changed", settings.clone());
    println!("[claude-usage] Poll interval changed to {}s", total_seconds);
    Ok(())
}

#[tauri::command]
fn update_tray_title(
    title: Option<String>,
    state: tauri::State<'_, Mutex<TrayState>>,
) -> Result<(), String> {
    let tray = state.lock().unwrap();
    tray.0
        .set_title(title.as_deref())
        .map_err(|e| format!("Failed to set tray title: {}", e))
}

// --- Debug / Testing Commands ---

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

#[tauri::command]
async fn login_oauth(app_handle: AppHandle) -> Result<(), String> {
    login_oauth_impl(app_handle).await
}

#[tauri::command]
async fn force_refresh_token() -> Result<serde_json::Value, String> {
    println!("[claude-usage] Force refresh triggered...");
    let blob = read_keychain_oauth_blob()?;
    let refresh_token = blob
        .refresh_token
        .as_deref()
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

// --- Main ---

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
            fetch_claude_usage,
            save_token,
            has_token,
            clear_token,
            get_settings,
            save_poll_interval,
            login_oauth,
            update_tray_title,
            debug_token_info,
            force_refresh_token
        ])
        .setup(|app| {
            let settings = load_settings();

            // Build tray context menu
            let remaining_mode_item =
                CheckMenuItemBuilder::with_id("remaining_mode", "Show Remaining (Juice Mode)")
                    .checked(settings.display_mode == "remaining")
                    .build(app)?;

            let autostart_manager = app.autolaunch();
            let is_autostart = autostart_manager.is_enabled().unwrap_or(false);
            let launch_at_login_item =
                CheckMenuItemBuilder::with_id("launch_at_login", "Launch at Login")
                    .checked(is_autostart)
                    .build(app)?;

            let quit_item = PredefinedMenuItem::quit(app, Some("Quit Claude Usage"))?;

            let tray_menu = MenuBuilder::new(app)
                .item(&remaining_mode_item)
                .separator()
                .text("configure_interval", "Configure Refresh Interval...")
                .separator()
                .text("check_for_updates", "Check for Updates...")
                .separator()
                .item(&launch_at_login_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Build tray icon (use dedicated tray icon – black silhouette on transparent)
            let tray_icon_bytes = include_bytes!("../icons/tray-icon@2x.png");
            let tray_icon_image = tauri::image::Image::from_bytes(tray_icon_bytes)
                .expect("Failed to load tray icon");
            let tray = TrayIconBuilder::new()
                .icon(tray_icon_image)
                .icon_as_template(true)
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                // Debounce: don't re-show if just hidden by focus loss
                                let last_hide = PANEL_HIDDEN_AT_MS.load(Ordering::SeqCst);
                                if now_ms() - last_hide < 300 {
                                    return;
                                }
                                let _ = window.move_window(Position::TrayCenter);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .on_menu_event(move |app_handle, event| {
                    let id = event.id().0.as_str();
                    match id {
                        "remaining_mode" => {
                            let state = app_handle.state::<Mutex<Settings>>();
                            let mut settings = state.lock().unwrap();
                            let checked = remaining_mode_item.is_checked().unwrap_or(true);
                            settings.display_mode = if checked {
                                "remaining".to_string()
                            } else {
                                "usage".to_string()
                            };
                            save_settings(&settings);
                            let _ = app_handle.emit("settings-changed", settings.clone());
                        }
                        "configure_interval" => {
                            let _ = app_handle.emit("show-settings", ());
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "check_for_updates" => {
                            let handle = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                check_for_update(handle, true).await;
                            });
                        }
                        "launch_at_login" => {
                            let manager = app_handle.autolaunch();
                            let checked = launch_at_login_item.is_checked().unwrap_or(false);
                            if checked {
                                let _ = manager.enable();
                            } else {
                                let _ = manager.disable();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            app.manage(Mutex::new(TrayState(tray)));

            // Focus-loss handler: hide panel when it loses focus
            if let Some(window) = app.get_webview_window("main") {
                let win = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        PANEL_HIDDEN_AT_MS.store(now_ms(), Ordering::SeqCst);
                        let _ = win.hide();
                    }
                });
            }

            // Auto-check for updates on startup (silent)
            let startup_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_update(startup_handle, false).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

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
