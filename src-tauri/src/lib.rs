use base64::Engine;
use rand::Rng;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serde::{Deserialize, Serialize};
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

const KEYCHAIN_SERVICE: &str = "app.claudeusage.desktop";
const KEYCHAIN_ACCOUNT: &str = "claude-oauth";

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

// --- Claude Usage Response ---

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUsageData {
    pub session_percent_used: f64,
    pub weekly_percent_used: f64,
    pub session_reset: Option<String>,
    pub weekly_reset: Option<String>,
    pub plan_type: Option<String>,
    pub extra_usage_spend: Option<f64>,
    pub extra_usage_limit: Option<f64>,
}

// --- Settings ---

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_display_mode")]
    pub display_mode: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
}

fn default_display_mode() -> String {
    "remaining".to_string()
}

fn default_poll_interval() -> u64 {
    60
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display_mode: default_display_mode(),
            poll_interval_seconds: default_poll_interval(),
        }
    }
}

// --- Credentials ---

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeCredentials {
    #[serde(rename = "accessToken", alias = "access_token")]
    access_token: Option<String>,
    #[serde(rename = "rateLimitTier", alias = "rate_limit_tier")]
    rate_limit_tier: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeOAuthBlob {
    #[serde(rename = "accessToken", alias = "access_token")]
    access_token: Option<String>,
    #[serde(rename = "refreshToken", alias = "refresh_token")]
    refresh_token: Option<String>,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: Option<i64>,
    scopes: Option<Vec<String>>,
    #[serde(rename = "subscriptionType", alias = "subscription_type")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier", alias = "rate_limit_tier")]
    rate_limit_tier: Option<String>,
}

// --- Usage API models ---

#[derive(Debug, Deserialize)]
struct ClaudeUsageWindow {
    utilization: Option<f64>,
    percent_used: Option<f64>,
    percent_left: Option<f64>,
    used: Option<f64>,
    limit: Option<f64>,
    reset_at: Option<String>,
    resets_at: Option<String>,
    reset_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeExtraUsage {
    is_enabled: Option<bool>,
    used_credits: Option<f64>,
    monthly_limit: Option<f64>,
    utilization: Option<f64>,
    spend: Option<f64>,
    limit: Option<f64>,
    used: Option<f64>,
    monthly_spend: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ClaudeOAuthUsageResponse {
    five_hour: Option<ClaudeUsageWindow>,
    seven_day: Option<ClaudeUsageWindow>,
    extra_usage: Option<ClaudeExtraUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeProfileResponse {
    organization: Option<ClaudeProfileOrganization>,
}

#[derive(Debug, Deserialize)]
struct ClaudeProfileOrganization {
    organization_type: Option<String>,
    rate_limit_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveTokenInput {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}

// --- Settings path ---

fn settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claude-usage");
    config_dir.join("settings.json")
}

fn load_settings() -> Settings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn save_settings(settings: &Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(&path, json);
    }
}

// --- Keychain (security-framework) ---

fn read_keychain_oauth_blob() -> Result<ClaudeOAuthBlob, String> {
    let data = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain lookup failed: {}", e))?;
    let json = String::from_utf8(data.to_vec())
        .map_err(|_| "Keychain data was not valid UTF-8.".to_string())?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to parse keychain JSON: {}", e))
}

fn write_keychain_oauth_blob(blob: &ClaudeOAuthBlob) -> Result<(), String> {
    let json = serde_json::to_string(blob)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, json.as_bytes())
        .map_err(|e| format!("Failed to write keychain: {}", e))
}

// --- Token validation ---

fn validate_claude_oauth_access_token(access_token: &str, source: &str) -> Result<(), String> {
    if access_token.starts_with("sk-ant-oat") {
        return Ok(());
    }
    Err(format!(
        "Claude OAuth token from {} is not an OAuth access token.",
        source
    ))
}

fn plan_display_from_profile(org: &ClaudeProfileOrganization) -> Option<String> {
    let tier = org.rate_limit_tier.as_deref().unwrap_or("");
    let org_type = org.organization_type.as_deref().unwrap_or("");

    // Extract multiplier from tier string like "default_claude_max_20x"
    let multiplier = tier
        .split('_')
        .rev()
        .find(|s| s.ends_with('x') && s[..s.len() - 1].parse::<u32>().is_ok())
        .map(|s| s.to_uppercase());

    // Determine base plan name
    let base = if org_type.contains("max") || tier.contains("max") {
        "Max"
    } else if org_type.contains("pro") || tier.contains("pro") {
        "Pro"
    } else if org_type.contains("team") || tier.contains("team") {
        "Team"
    } else if org_type.contains("enterprise") || tier.contains("enterprise") {
        "Enterprise"
    } else if !org_type.is_empty() {
        return Some(org_type.to_string());
    } else {
        return None;
    };

    match multiplier {
        Some(m) => Some(format!("{} {}", base, m)),
        None => Some(base.to_string()),
    }
}

// --- Usage parsing helpers ---

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn usage_window_percent(window: &ClaudeUsageWindow) -> Option<f64> {
    if let Some(v) = window.utilization {
        return Some(clamp_percent(v));
    }
    if let Some(v) = window.percent_used {
        return Some(clamp_percent(v));
    }
    if let Some(v) = window.percent_left {
        return Some(clamp_percent(100.0 - v));
    }
    match (window.used, window.limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some(clamp_percent((used / limit) * 100.0)),
        _ => None,
    }
}

fn usage_window_reset(window: &ClaudeUsageWindow) -> Option<String> {
    window
        .reset_at
        .clone()
        .or_else(|| window.resets_at.clone())
        .or_else(|| window.reset_time.clone())
}

fn value_to_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(v) = value.get(*key) {
            if let Some(n) = v.as_f64() {
                return Some(n);
            }
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn extract_percent_from_window_value(window: &Value) -> Option<f64> {
    if let Some(n) = window.as_f64() {
        return Some(clamp_percent(n));
    }
    if let Some(n) = value_to_f64(window, &["utilization"]) {
        return Some(clamp_percent(n));
    }
    if let Some(n) = value_to_f64(window, &["percent_used", "used_percent", "usage_percent"]) {
        return Some(clamp_percent(n));
    }
    if let Some(n) = value_to_f64(window, &["percent_left", "remaining_percent"]) {
        return Some(clamp_percent(100.0 - n));
    }
    match (
        value_to_f64(window, &["used", "value", "spend"]),
        value_to_f64(window, &["limit", "total"]),
    ) {
        (Some(used), Some(limit)) if limit > 0.0 => Some(clamp_percent((used / limit) * 100.0)),
        _ => None,
    }
}

fn extract_reset_from_window_value(window: &Value) -> Option<String> {
    for key in ["reset_at", "resets_at", "reset_time", "resets_in"] {
        if let Some(v) = window.get(key).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

fn extract_window_percent(root: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(window) = root.get(*key) {
            if let Some(percent) = extract_percent_from_window_value(window) {
                return Some(percent);
            }
        }
    }
    None
}

fn extract_window_reset(root: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(window) = root.get(*key) {
            if let Some(reset) = extract_reset_from_window_value(window) {
                return Some(reset);
            }
        }
    }
    None
}

// --- Token refresh ---

async fn refresh_claude_token(
    refresh_token: &str,
    blob: &ClaudeOAuthBlob,
) -> Result<ClaudeCredentials, String> {
    println!("[claude-usage] Claude auth: attempting OAuth token refresh...");

    let client = reqwest::Client::new();
    let response = client
        .post("https://platform.claude.com/v1/oauth/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=refresh_token&refresh_token={}",
            refresh_token
        ))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Claude token refresh request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Claude token refresh returned HTTP {}.", status));
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

    println!("[claude-usage] Claude auth: token refresh succeeded.");
    Ok(ClaudeCredentials {
        access_token: Some(new_access_token),
        rate_limit_tier: updated_blob.rate_limit_tier,
    })
}

// --- Load credentials from keychain ---

async fn load_claude_credentials() -> Result<ClaudeCredentials, String> {
    let oauth = read_keychain_oauth_blob()?;

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
    }

    Ok(ClaudeCredentials {
        access_token: Some(access_token.to_string()),
        rate_limit_tier: oauth.rate_limit_tier,
    })
}

// --- Fetch usage ---

async fn fetch_claude_usage_impl() -> Result<ClaudeUsageData, String> {
    let credentials = load_claude_credentials().await?;
    let access_token = credentials
        .access_token
        .as_deref()
        .ok_or_else(|| "Claude credentials are missing accessToken.".to_string())?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Claude OAuth request failed: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Claude OAuth response body: {}", e))?;

    if !status.is_success() {
        println!(
            "[claude-usage] Claude OAuth API error: HTTP {} — {}",
            status,
            &body[..body.len().min(500)]
        );

        // Parse error body for a user-friendly message
        if let Ok(err_val) = serde_json::from_str::<Value>(&body) {
            if let Some(msg) = err_val
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                if msg.contains("scope") {
                    return Err(format!(
                        "Token missing required scope. {}. Re-run `claude login` or create a token with the user:profile scope.",
                        msg
                    ));
                }
                return Err(format!("Claude API error: {}", msg));
            }
        }

        return Err(format!(
            "Claude OAuth API returned HTTP {}. {}",
            status,
            &body[..body.len().min(200)]
        ));
    }

    let value: Value =
        serde_json::from_str(&body).map_err(|e| format!("Invalid Claude OAuth JSON: {}", e))?;

    let typed = serde_json::from_value::<ClaudeOAuthUsageResponse>(value.clone()).ok();

    let session_percent_used = typed
        .as_ref()
        .and_then(|t| t.five_hour.as_ref())
        .and_then(usage_window_percent)
        .or_else(|| extract_window_percent(&value, &["five_hour", "current_session"]))
        .unwrap_or(0.0);

    let weekly_percent_used = typed
        .as_ref()
        .and_then(|t| t.seven_day.as_ref())
        .and_then(usage_window_percent)
        .or_else(|| extract_window_percent(&value, &["seven_day", "current_week"]))
        .unwrap_or(0.0);

    let session_reset = typed
        .as_ref()
        .and_then(|t| t.five_hour.as_ref())
        .and_then(usage_window_reset)
        .or_else(|| extract_window_reset(&value, &["five_hour", "current_session"]));

    let weekly_reset = typed
        .as_ref()
        .and_then(|t| t.seven_day.as_ref())
        .and_then(usage_window_reset)
        .or_else(|| extract_window_reset(&value, &["seven_day", "current_week"]));

    let (extra_usage_spend, extra_usage_limit) =
        if let Some(extra) = typed.as_ref().and_then(|t| t.extra_usage.as_ref()) {
            let is_enabled = extra.is_enabled.unwrap_or(false);
            if is_enabled {
                (
                    extra
                        .used_credits
                        .or(extra.spend)
                        .or(extra.used)
                        .or(extra.monthly_spend),
                    extra.monthly_limit.or(extra.limit),
                )
            } else {
                (None, None)
            }
        } else {
            let spend = value_to_f64(&value, &["extra_usage_spend", "spend", "monthly_spend"]);
            let limit = value_to_f64(&value, &["extra_usage_limit", "limit", "monthly_limit"]);
            (spend, limit)
        };

    // Fetch plan type from the profile endpoint
    let plan_type = match client
        .get("https://api.anthropic.com/api/oauth/profile")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp
            .text()
            .await
            .ok()
            .and_then(|b| serde_json::from_str::<ClaudeProfileResponse>(&b).ok())
            .and_then(|p| p.organization)
            .and_then(|org| plan_display_from_profile(&org)),
        _ => None,
    };

    Ok(ClaudeUsageData {
        session_percent_used: clamp_percent(session_percent_used),
        weekly_percent_used: clamp_percent(weekly_percent_used),
        session_reset,
        weekly_reset,
        plan_type,
        extra_usage_spend,
        extra_usage_limit,
    })
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
    delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Failed to clear token: {}", e))
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

// --- Updates ---

static UPDATE_CHECK_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

async fn check_for_update(app: AppHandle, manual: bool) {
    if UPDATE_CHECK_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }
    check_for_update_inner(&app, manual).await;
    UPDATE_CHECK_IN_PROGRESS.store(false, Ordering::SeqCst);
}

async fn check_for_update_inner(app: &AppHandle, manual: bool) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            println!("[claude-usage] Updater unavailable: {}", e);
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            println!("[claude-usage] No update available.");
            if manual {
                app.dialog()
                    .message("You're on the latest version.")
                    .title("No Updates")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
            return;
        }
        Err(e) => {
            println!("[claude-usage] Update check failed: {}", e);
            if manual {
                app.dialog()
                    .message("Failed to check for updates. Please try again later.")
                    .title("Update Error")
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
            return;
        }
    };

    println!(
        "[claude-usage] Update available: v{} (notes: {:?})",
        update.version,
        update.body
    );

    let version = update.version.clone();
    let body = update.body.clone().unwrap_or_default();

    if let Some(win) = app.get_webview_window("update") {
        let _ = win.close();
    }

    let update_window = match WebviewWindowBuilder::new(
        app,
        "update",
        WebviewUrl::App("index.html".into()),
    )
    .title("Claude Usage Update")
    .inner_size(340.0, 400.0)
    .resizable(false)
    .center()
    .background_color(tauri::window::Color(28, 28, 30, 255))
    .build()
    {
        Ok(w) => w,
        Err(e) => {
            println!("[claude-usage] Failed to create update window: {}", e);
            return;
        }
    };

    let ready_handle = app.clone();
    let ready_id = update_window.listen("update-ready", move |_: tauri::Event| {
        if let Some(win) = ready_handle.get_webview_window("update") {
            let payload = serde_json::json!({
                "version": version,
                "body": body,
            });
            let _ = win.emit("update-info", payload);
        }
    });

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    let tx = std::sync::Mutex::new(Some(tx));
    let response_id = update_window.listen("update-response", move |event: tauri::Event| {
        if let Some(tx) = tx.lock().unwrap().take() {
            let accepted = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|v| v.get("accepted").and_then(|a| a.as_bool()))
                .unwrap_or(false);
            let _ = tx.send(accepted);
        }
    });

    let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();
    let close_tx = std::sync::Mutex::new(Some(close_tx));
    let close_id = update_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(tx) = close_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    });

    let accepted = tokio::select! {
        result = rx => result.unwrap_or(false),
        _ = close_rx => false,
    };

    update_window.unlisten(response_id);
    update_window.unlisten(ready_id);
    let _ = close_id;

    if let Some(win) = app.get_webview_window("update") {
        let _ = win.close();
    }

    if accepted {
        println!("[claude-usage] User accepted update, downloading...");
        if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
            println!("[claude-usage] Update install failed: {}", e);
            app.dialog()
                .message(format!("Failed to install update: {}", e))
                .title("Update Error")
                .kind(MessageDialogKind::Error)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
            return;
        }
        app.restart();
    } else {
        println!("[claude-usage] User declined update.");
    }
}

// --- OAuth PKCE ---

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

const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

#[tauri::command]
async fn login_oauth(app_handle: AppHandle) -> Result<(), String> {
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
            update_tray_title
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
