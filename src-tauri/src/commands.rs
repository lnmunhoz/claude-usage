use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::keychain::{
    delete_keychain_oauth_blob, read_keychain_oauth_blob, validate_claude_oauth_access_token,
    write_keychain_oauth_blob,
};
use crate::models::{ClaudeOAuthBlob, ClaudeUsageData, SaveTokenInput, Settings};
use crate::now_ms;
use crate::oauth::{login_oauth_impl, refresh_claude_token};
use crate::settings::save_settings;
use crate::tray::TrayState;
use crate::usage::fetch_claude_usage_impl;

#[tauri::command]
pub(crate) async fn fetch_claude_usage() -> Result<ClaudeUsageData, String> {
    fetch_claude_usage_impl().await
}

#[tauri::command]
pub(crate) async fn save_token(input: SaveTokenInput) -> Result<(), String> {
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
pub(crate) fn has_token() -> bool {
    read_keychain_oauth_blob().is_ok()
}

#[tauri::command]
pub(crate) fn clear_token() -> Result<(), String> {
    delete_keychain_oauth_blob()
}

#[tauri::command]
pub(crate) fn get_settings(state: tauri::State<'_, Mutex<Settings>>) -> Settings {
    state.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn save_poll_interval(
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
pub(crate) fn update_tray_title(
    title: Option<String>,
    state: tauri::State<'_, Mutex<TrayState>>,
) -> Result<(), String> {
    let tray = state.lock().unwrap();
    tray.0
        .set_title(title.as_deref())
        .map_err(|e| format!("Failed to set tray title: {}", e))
}

#[tauri::command]
pub(crate) fn debug_token_info() -> Result<serde_json::Value, String> {
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
pub(crate) async fn login_oauth(app_handle: AppHandle) -> Result<(), String> {
    login_oauth_impl(app_handle).await
}

#[tauri::command]
pub(crate) async fn force_refresh_token() -> Result<serde_json::Value, String> {
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
