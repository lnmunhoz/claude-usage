use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

// Cookie names that Cursor uses for session auth
const SESSION_COOKIE_NAMES: &[&str] = &[
    "WorkosCursorSessionToken",
    "__Secure-next-auth.session-token",
    "next-auth.session-token",
];

const CURSOR_DOMAINS: &[&str] = &["cursor.com", "cursor.sh"];
const CLAUDE_DOMAIN: &str = "claude.ai";

// --- API Response Models (matching CodexBar's CursorUsageSummary) ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CursorUsageSummary {
    billing_cycle_start: Option<String>,
    billing_cycle_end: Option<String>,
    membership_type: Option<String>,
    individual_usage: Option<CursorIndividualUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorIndividualUsage {
    plan: Option<CursorPlanUsage>,
    on_demand: Option<CursorOnDemandUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPlanUsage {
    used: Option<i64>,
    limit: Option<i64>,
    remaining: Option<i64>,
    total_percent_used: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorOnDemandUsage {
    used: Option<i64>,
    limit: Option<i64>,
}

// --- Response sent to the frontend ---

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageData {
    pub percent_used: f64,
    pub used_usd: f64,
    pub limit_usd: f64,
    pub remaining_usd: f64,
    pub on_demand_percent_used: f64,
    pub on_demand_used_usd: f64,
    pub on_demand_limit_usd: Option<f64>,
    pub billing_cycle_end: Option<String>,
    pub membership_type: Option<String>,
}

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
    pub show_plan: bool,
    pub show_on_demand: bool,
    pub show_claude_window: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_plan: true,
            show_on_demand: true,
            show_claude_window: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeCredentials {
    #[serde(rename = "accessToken", alias = "access_token")]
    access_token: Option<String>,
    #[serde(rename = "rateLimitTier", alias = "rate_limit_tier")]
    rate_limit_tier: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeKeychainPayload {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeOAuthBlob>,
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

#[derive(Debug, Deserialize)]
struct ClaudeUsageWindow {
    /// Primary field from the real API response
    utilization: Option<f64>,
    /// Fallback fields for alternative response shapes
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
    /// Fallback fields
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

fn settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cursor-juice");
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

// --- Cookie Import ---

fn find_cursor_cookie_header() -> Result<String, String> {
    // Try loading cookies from all browsers at once
    let domains: Vec<String> = CURSOR_DOMAINS.iter().map(|d| d.to_string()).collect();
    println!("[cursor-juice] Searching for cookies in domains: {:?}", domains);

    let cookies = rookie::load(Some(domains)).map_err(|e| {
        let msg = format!("Failed to read browser cookies: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;

    println!(
        "[cursor-juice] Found {} cookies from cursor domains",
        cookies.len()
    );
    for cookie in &cookies {
        println!(
            "[cursor-juice]   cookie: name={}, domain={}",
            cookie.name, cookie.domain
        );
    }

    // Find a session cookie
    for cookie in &cookies {
        if SESSION_COOKIE_NAMES.contains(&cookie.name.as_str()) {
            println!(
                "[cursor-juice] Found session cookie: {}",
                cookie.name
            );
            // Build a cookie header with all cookies from cursor domains
            let cookie_header: String = cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            return Ok(cookie_header);
        }
    }

    let msg =
        "No Cursor session cookie found. Make sure you are logged into cursor.com in your browser."
            .to_string();
    eprintln!("[cursor-juice] {}", msg);
    Err(msg)
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn usage_window_percent(window: &ClaudeUsageWindow) -> Option<f64> {
    // Real API returns "utilization" as 0-100 percentage
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
    // Real API uses "utilization" as the percentage field
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

fn extract_org_id(orgs_payload: &Value) -> Option<String> {
    let pick_from_object = |obj: &Value| -> Option<String> {
        for key in ["uuid", "id", "organization_uuid", "organizationId"] {
            if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
        None
    };

    if let Some(arr) = orgs_payload.as_array() {
        return arr.first().and_then(pick_from_object);
    }

    if let Some(arr) = orgs_payload.get("organizations").and_then(|v| v.as_array()) {
        return arr.first().and_then(pick_from_object);
    }

    pick_from_object(orgs_payload)
}

fn plan_type_from_rate_tier(rate_limit_tier: Option<&str>) -> Option<String> {
    let tier = rate_limit_tier?.to_lowercase();
    if tier.contains("enterprise") {
        Some("enterprise".to_string())
    } else if tier.contains("team") {
        Some("team".to_string())
    } else if tier.contains("max") {
        Some("max".to_string())
    } else if tier.contains("pro") {
        Some("pro".to_string())
    } else {
        Some(tier)
    }
}

fn claude_credentials_path() -> Result<PathBuf, String> {
    if let Ok(config_roots) = std::env::var("CLAUDE_CONFIG_DIR") {
        for root in config_roots.split(',') {
            let trimmed = root.trim();
            if trimmed.is_empty() {
                continue;
            }
            let candidate = PathBuf::from(trimmed).join(".credentials.json");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    let home = dirs::home_dir()
        .ok_or_else(|| "Could not resolve home directory for Claude credentials.".to_string())?;
    let candidates = [
        home.join(".claude").join(".credentials.json"),
        home.join(".config").join("claude").join(".credentials.json"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(
        "Claude OAuth credentials file not found. Tried CLAUDE_CONFIG_DIR roots, ~/.claude/.credentials.json, and ~/.config/claude/.credentials.json."
            .to_string(),
    )
}

fn validate_claude_oauth_access_token(access_token: &str, source: &str) -> Result<(), String> {
    if access_token.starts_with("sk-ant-oat") {
        return Ok(());
    }
    Err(format!(
        "Claude OAuth token from {} is not an OAuth access token.",
        source
    ))
}

// Token metadata validation is now done inline in load_claude_credentials_from_keychain.

fn run_security_lookup(account: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("/usr/bin/security");
    cmd.arg("find-generic-password")
        .arg("-s")
        .arg("Claude Code-credentials")
        .arg("-w");
    if let Some(acct) = account {
        cmd.arg("-a").arg(acct);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Claude keychain lookup failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .next()
            .unwrap_or("security command returned non-zero status");
        return Err(format!("Claude keychain lookup failed: {}", reason));
    }

    String::from_utf8(output.stdout)
        .map_err(|_| "Claude keychain credentials were not valid UTF-8.".to_string())
}

#[cfg(target_os = "macos")]
fn read_keychain_oauth_blob() -> Result<(ClaudeOAuthBlob, String), String> {
    println!("[cursor-juice] Claude auth: checking macOS Keychain service 'Claude Code-credentials'...");
    let account = std::env::var("USER").ok();
    let raw = if let Some(ref acct) = account {
        println!(
            "[cursor-juice] Claude auth: trying keychain lookup scoped to account '{}'.",
            acct
        );
        match run_security_lookup(Some(acct)) {
            Ok(raw) => raw,
            Err(account_err) => {
                println!(
                    "[cursor-juice] Claude auth: account-scoped lookup failed ({}). Trying unscoped lookup...",
                    account_err
                );
                run_security_lookup(None)?
            }
        }
    } else {
        println!(
            "[cursor-juice] Claude auth: USER env var unavailable. Trying unscoped keychain lookup..."
        );
        run_security_lookup(None)?
    };

    let payload: ClaudeKeychainPayload = serde_json::from_str(raw.trim())
        .map_err(|e| format!("Failed to parse Claude keychain credentials JSON: {}", e))?;

    let oauth = payload
        .claude_ai_oauth
        .ok_or_else(|| "Claude keychain entry missing claudeAiOauth object.".to_string())?;

    let acct = account.unwrap_or_default();
    Ok((oauth, acct))
}

#[cfg(target_os = "macos")]
fn write_keychain_oauth_blob(blob: &ClaudeOAuthBlob, account: &str) -> Result<(), String> {
    let payload = ClaudeKeychainPayload {
        claude_ai_oauth: Some(blob.clone()),
    };
    let json = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize keychain payload: {}", e))?;

    let acct_arg = if account.is_empty() {
        std::env::var("USER").unwrap_or_default()
    } else {
        account.to_string()
    };

    let output = Command::new("/usr/bin/security")
        .arg("add-generic-password")
        .arg("-U") // update if exists
        .arg("-s")
        .arg("Claude Code-credentials")
        .arg("-a")
        .arg(&acct_arg)
        .arg("-w")
        .arg(&json)
        .output()
        .map_err(|e| format!("Failed to update keychain: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to update keychain: {}", stderr.trim()));
    }

    println!("[cursor-juice] Claude auth: keychain entry updated with refreshed token.");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

async fn refresh_claude_token(
    refresh_token: &str,
    blob: &ClaudeOAuthBlob,
    account: &str,
) -> Result<ClaudeCredentials, String> {
    println!("[cursor-juice] Claude auth: attempting OAuth token refresh...");

    let client = reqwest::Client::new();
    let response = client
        .post("https://console.anthropic.com/v1/oauth/token")
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
    println!(
        "[cursor-juice] Claude auth: token refresh endpoint returned HTTP {}.",
        status
    );

    if !status.is_success() {
        return Err(format!(
            "Claude token refresh returned HTTP {}.",
            status
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read token refresh response: {}", e))?;
    let token_resp: OAuthTokenResponse = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse token refresh response: {}", e))?;

    let new_access_token = token_resp
        .access_token
        .ok_or_else(|| "Token refresh response missing access_token.".to_string())?;
    validate_claude_oauth_access_token(&new_access_token, "refreshed token")?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let new_expires_at = token_resp
        .expires_in
        .map(|secs| now_ms + secs * 1000)
        .or(blob.expires_at);

    let new_refresh = token_resp
        .refresh_token
        .unwrap_or_else(|| refresh_token.to_string());

    // Update the keychain with refreshed credentials
    let mut updated_blob = blob.clone();
    updated_blob.access_token = Some(new_access_token.clone());
    updated_blob.refresh_token = Some(new_refresh);
    updated_blob.expires_at = new_expires_at;

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = write_keychain_oauth_blob(&updated_blob, account) {
            println!(
                "[cursor-juice] Claude auth: warning - could not update keychain ({}). Token will work for this session only.",
                e
            );
        }
    }

    println!("[cursor-juice] Claude auth: token refresh succeeded.");
    Ok(ClaudeCredentials {
        access_token: Some(new_access_token),
        rate_limit_tier: updated_blob.rate_limit_tier,
    })
}

#[cfg(target_os = "macos")]
fn load_claude_credentials_from_keychain_sync() -> Result<(ClaudeOAuthBlob, String), String> {
    read_keychain_oauth_blob()
}

#[cfg(not(target_os = "macos"))]
fn load_claude_credentials_from_keychain_sync() -> Result<(ClaudeOAuthBlob, String), String> {
    Err("Claude keychain OAuth is only supported on macOS.".to_string())
}

/// Reads keychain, validates token, refreshes if expired (async).
async fn load_claude_credentials_from_keychain() -> Result<ClaudeCredentials, String> {
    let (oauth, account) = load_claude_credentials_from_keychain_sync()?;

    let access_token = oauth
        .access_token
        .as_deref()
        .ok_or_else(|| "Claude keychain entry missing access token.".to_string())?;
    validate_claude_oauth_access_token(access_token, "keychain")?;

    // Check scope
    if let Some(ref scopes) = oauth.scopes {
        let has_profile_scope = scopes.iter().any(|s| s == "user:profile");
        if !has_profile_scope {
            return Err("Claude OAuth token from keychain is missing user:profile scope.".to_string());
        }
        println!(
            "[cursor-juice] Claude auth: keychain token has user:profile scope."
        );
    }

    // Check expiry — if expired, attempt refresh
    if let Some(expires_at_ms) = oauth.expires_at {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        if expires_at_ms <= now_ms + 60_000 {
            println!(
                "[cursor-juice] Claude auth: keychain access token is expired (expired {}ms ago). Checking for refresh token...",
                now_ms - expires_at_ms
            );
            if let Some(ref refresh_token) = oauth.refresh_token {
                return refresh_claude_token(refresh_token, &oauth, &account).await;
            } else {
                return Err("Claude keychain token is expired and no refresh token available.".to_string());
            }
        }
    }

    println!(
        "[cursor-juice] Claude auth: keychain credentials are usable (scope + expiry checks passed)."
    );

    Ok(ClaudeCredentials {
        access_token: Some(access_token.to_string()),
        rate_limit_tier: oauth.rate_limit_tier,
    })
}

#[cfg(not(target_os = "macos"))]
fn read_keychain_oauth_blob() -> Result<(ClaudeOAuthBlob, String), String> {
    Err("Claude keychain OAuth is only supported on macOS.".to_string())
}

fn load_claude_credentials_from_file() -> Result<ClaudeCredentials, String> {
    println!("[cursor-juice] Claude auth: checking file-based OAuth credentials...");
    let path = claude_credentials_path()?;
    println!(
        "[cursor-juice] Claude auth: using credentials file at {}",
        path.display()
    );
    let raw = fs::read_to_string(&path).map_err(|e| {
        format!(
            "Failed to read Claude credentials at {}: {}",
            path.display(),
            e
        )
    })?;

    let credentials = serde_json::from_str::<ClaudeCredentials>(&raw)
        .map_err(|e| format!("Failed to parse Claude credentials JSON: {}", e))?;
    if let Some(access_token) = credentials.access_token.as_deref() {
        validate_claude_oauth_access_token(access_token, "credentials file")?;
    }
    println!("[cursor-juice] Claude auth: credentials file parsed successfully.");
    Ok(credentials)
}

async fn load_claude_credentials() -> Result<ClaudeCredentials, String> {
    println!("[cursor-juice] Claude auth: starting credential source resolution (keychain -> file).");
    match load_claude_credentials_from_keychain().await {
        Ok(credentials) => {
            println!("[cursor-juice] Claude auth: using keychain credentials.");
            return Ok(credentials);
        }
        Err(keychain_err) => {
            println!(
                "[cursor-juice] Claude auth: keychain unavailable/unusable ({}). Trying credentials file...",
                keychain_err
            );
            match load_claude_credentials_from_file() {
                Ok(credentials) => {
                    println!("[cursor-juice] Claude auth: using file credentials.");
                    return Ok(credentials);
                }
                Err(file_err) => {
                    return Err(format!(
                        "No usable Claude OAuth credentials available. keychain={} file={}",
                        keychain_err, file_err
                    ))
                }
            }
        }
    }
}

fn find_claude_session_cookies() -> Result<Vec<String>, String> {
    let cookies = rookie::load(Some(vec![CLAUDE_DOMAIN.to_string()]))
        .map_err(|e| format!("Failed to read browser cookies for claude.ai: {}", e))?;

    let mut session_keys = Vec::new();
    for cookie in cookies {
        if cookie.name == "sessionKey" && !cookie.value.is_empty() {
            session_keys.push(cookie.value);
        }
    }

    if session_keys.is_empty() {
        return Err(
            "No claude.ai sessionKey cookie found. Log into claude.ai in your browser.".to_string(),
        );
    }

    Ok(session_keys)
}

async fn fetch_claude_usage_oauth() -> Result<ClaudeUsageData, String> {
    println!("[cursor-juice] Claude auth: OAuth path selected.");
    let credentials = load_claude_credentials().await?;
    let access_token = credentials
        .access_token
        .as_deref()
        .ok_or_else(|| "Claude credentials are missing accessToken.".to_string())?;
    println!(
        "[cursor-juice] Claude auth: requesting OAuth usage endpoint with resolved credentials."
    );

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
    println!("[cursor-juice] Claude OAuth usage endpoint status: {}", status);
    if !status.is_success() {
        return Err(format!("Claude OAuth API returned HTTP {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Claude OAuth response body: {}", e))?;
    println!(
        "[cursor-juice] Claude OAuth raw response: {}",
        &body[..body.len().min(500)]
    );

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

    let (extra_usage_spend, extra_usage_limit) = if let Some(extra) = typed
        .as_ref()
        .and_then(|t| t.extra_usage.as_ref())
    {
        let is_enabled = extra.is_enabled.unwrap_or(false);
        if is_enabled {
            (
                extra
                    .used_credits
                    .or(extra.spend)
                    .or(extra.used)
                    .or(extra.monthly_spend),
                extra
                    .monthly_limit
                    .or(extra.limit),
            )
        } else {
            (None, None)
        }
    } else {
        let spend = value_to_f64(&value, &["extra_usage_spend", "spend", "monthly_spend"]);
        let limit = value_to_f64(&value, &["extra_usage_limit", "limit", "monthly_limit"]);
        (spend, limit)
    };

    println!(
        "[cursor-juice] Claude usage parsed: session={:.1}%, weekly={:.1}%, session_reset={:?}, weekly_reset={:?}, extra_spend={:?}, extra_limit={:?}",
        session_percent_used, weekly_percent_used, session_reset, weekly_reset, extra_usage_spend, extra_usage_limit
    );

    Ok(ClaudeUsageData {
        session_percent_used: clamp_percent(session_percent_used),
        weekly_percent_used: clamp_percent(weekly_percent_used),
        session_reset,
        weekly_reset,
        plan_type: plan_type_from_rate_tier(credentials.rate_limit_tier.as_deref()),
        extra_usage_spend,
        extra_usage_limit,
    })
}

async fn fetch_claude_usage_web() -> Result<ClaudeUsageData, String> {
    println!("[cursor-juice] Claude auth: entering web-cookie fallback.");
    let session_keys = find_claude_session_cookies()?;
    println!(
        "[cursor-juice] Claude auth: found {} claude.ai sessionKey candidate(s).",
        session_keys.len()
    );
    let client = reqwest::Client::new();

    let mut last_org_error: Option<String> = None;
    for (idx, session_key) in session_keys.iter().enumerate() {
        let cookie_header = format!("sessionKey={}", session_key);
        println!(
            "[cursor-juice] Claude web: trying sessionKey candidate {}/{}...",
            idx + 1,
            session_keys.len()
        );

        let org_response = client
            .get("https://claude.ai/api/organizations")
            .header("Accept", "application/json")
            .header("Cookie", &cookie_header)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Claude organizations: {}", e))?;
        println!(
            "[cursor-juice] Claude web organizations endpoint status: {}",
            org_response.status()
        );

        if !org_response.status().is_success() {
            let msg = format!(
                "Claude organizations endpoint returned HTTP {} for sessionKey candidate {}/{}",
                org_response.status(),
                idx + 1,
                session_keys.len()
            );
            last_org_error = Some(msg);
            continue;
        }

        let org_body = org_response
            .text()
            .await
            .map_err(|e| format!("Failed to read Claude organizations body: {}", e))?;
        let org_value: Value = serde_json::from_str(&org_body)
            .map_err(|e| format!("Invalid Claude organizations JSON: {}", e))?;
        let org_id = extract_org_id(&org_value).ok_or_else(|| {
            "Could not find a Claude organization ID in organizations response.".to_string()
        })?;

        let usage_url = format!("https://claude.ai/api/organizations/{}/usage", org_id);
        let usage_response = client
            .get(&usage_url)
            .header("Accept", "application/json")
            .header("Cookie", &cookie_header)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Claude usage: {}", e))?;
        println!(
            "[cursor-juice] Claude web usage endpoint status: {}",
            usage_response.status()
        );

        if !usage_response.status().is_success() {
            let msg = format!(
                "Claude usage endpoint returned HTTP {} for sessionKey candidate {}/{}",
                usage_response.status(),
                idx + 1,
                session_keys.len()
            );
            last_org_error = Some(msg);
            continue;
        }

        let usage_body = usage_response
            .text()
            .await
            .map_err(|e| format!("Failed to read Claude usage body: {}", e))?;
        println!(
            "[cursor-juice] Claude web usage raw response: {}",
            &usage_body[..usage_body.len().min(500)]
        );
        let usage_value: Value = serde_json::from_str(&usage_body)
            .map_err(|e| format!("Invalid Claude usage JSON: {}", e))?;

        let session_percent_used =
            extract_window_percent(&usage_value, &["five_hour", "current_session"]).unwrap_or(0.0);
        let weekly_percent_used =
            extract_window_percent(&usage_value, &["seven_day", "current_week"]).unwrap_or(0.0);
        let session_reset = extract_window_reset(&usage_value, &["five_hour", "current_session"]);
        let weekly_reset = extract_window_reset(&usage_value, &["seven_day", "current_week"]);

        let overage_url = format!(
            "https://claude.ai/api/organizations/{}/overage_spend_limit",
            org_id
        );
        let mut extra_usage_spend = None;
        let mut extra_usage_limit = None;
        if let Ok(resp) = client
            .get(&overage_url)
            .header("Accept", "application/json")
            .header("Cookie", &cookie_header)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        extra_usage_spend = value_to_f64(&v, &["spend", "used", "monthly_spend"]);
                        extra_usage_limit = value_to_f64(&v, &["limit", "monthly_limit"]);
                    }
                }
            }
        }

        let account_url = "https://claude.ai/api/account";
        let mut plan_type = None;
        if let Ok(resp) = client
            .get(account_url)
            .header("Accept", "application/json")
            .header("Cookie", &cookie_header)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        plan_type = v
                            .get("plan")
                            .or_else(|| v.get("plan_type"))
                            .or_else(|| v.get("subscription_tier"))
                            .and_then(|x| x.as_str())
                            .map(|x| x.to_string());
                    }
                }
            }
        }

        println!(
            "[cursor-juice] Claude web: selected sessionKey candidate {}/{}.",
            idx + 1,
            session_keys.len()
        );
        return Ok(ClaudeUsageData {
            session_percent_used: clamp_percent(session_percent_used),
            weekly_percent_used: clamp_percent(weekly_percent_used),
            session_reset,
            weekly_reset,
            plan_type,
            extra_usage_spend,
            extra_usage_limit,
        });
    }

    Err(last_org_error.unwrap_or_else(|| {
        "Claude web fallback failed: all sessionKey candidates were rejected.".to_string()
    }))
}

fn create_claude_window(app_handle: &AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("claude") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(app_handle, "claude", WebviewUrl::App("index.html".into()))
        .title("Claude Juice")
        .inner_size(70.0, 400.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .shadow(false)
        .build()
        .map_err(|e| format!("Failed to create Claude window: {}", e))?;

    Ok(())
}

// --- Tauri Commands ---

#[tauri::command]
async fn fetch_cursor_usage() -> Result<UsageData, String> {
    println!("[cursor-juice] fetch_cursor_usage called");

    let cookie_header = find_cursor_cookie_header()?;
    println!("[cursor-juice] Got cookie header, making API request...");

    let client = reqwest::Client::new();
    let response = client
        .get("https://cursor.com/api/usage-summary")
        .header("Accept", "application/json")
        .header("Cookie", &cookie_header)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            let msg = format!("Network error: {}", e);
            eprintln!("[cursor-juice] {}", msg);
            msg
        })?;

    let status = response.status();
    println!("[cursor-juice] API response status: {}", status);

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let msg = "Not logged in. Please log into cursor.com in your browser and try again.";
        eprintln!("[cursor-juice] {}", msg);
        return Err(msg.to_string());
    }

    if !status.is_success() {
        let msg = format!("Cursor API returned HTTP {}", status);
        eprintln!("[cursor-juice] {}", msg);
        return Err(msg);
    }

    // Read raw body first for debugging
    let body = response.text().await.map_err(|e| {
        let msg = format!("Failed to read response body: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;
    println!("[cursor-juice] Raw API response: {}", &body[..body.len().min(500)]);

    let summary: CursorUsageSummary = serde_json::from_str(&body).map_err(|e| {
        let msg = format!("Failed to parse Cursor API response: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;

    // Convert cents to USD
    let plan = summary
        .individual_usage
        .as_ref()
        .and_then(|u| u.plan.as_ref());
    let on_demand = summary
        .individual_usage
        .as_ref()
        .and_then(|u| u.on_demand.as_ref());

    let used_cents = plan.and_then(|p| p.used).unwrap_or(0) as f64;
    let limit_cents = plan.and_then(|p| p.limit).unwrap_or(0) as f64;
    let remaining_cents = plan.and_then(|p| p.remaining).unwrap_or(0) as f64;

    // Use dollar-based calculation: used / limit * 100
    let percent_used = if limit_cents > 0.0 {
        (used_cents / limit_cents) * 100.0
    } else {
        0.0
    };

    println!(
        "[cursor-juice] Plan percent: {:.2}% (${:.2} / ${:.2})",
        percent_used, used_cents / 100.0, limit_cents / 100.0
    );

    let od_used_cents = on_demand.and_then(|o| o.used).unwrap_or(0) as f64;
    let od_limit_cents = on_demand.and_then(|o| o.limit);

    let on_demand_percent_used = match od_limit_cents {
        Some(limit) if limit > 0 => (od_used_cents / limit as f64) * 100.0,
        _ => 0.0,
    };

    let result = UsageData {
        percent_used,
        used_usd: used_cents / 100.0,
        limit_usd: limit_cents / 100.0,
        remaining_usd: remaining_cents / 100.0,
        on_demand_percent_used,
        on_demand_used_usd: od_used_cents / 100.0,
        on_demand_limit_usd: od_limit_cents.map(|c| c as f64 / 100.0),
        billing_cycle_end: summary.billing_cycle_end,
        membership_type: summary.membership_type,
    };

    println!(
        "[cursor-juice] Plan: {:.1}% (${:.2} / ${:.2}) | On-demand: {:.1}% (${:.2} / ${}) | membership: {:?}",
        result.percent_used, result.used_usd, result.limit_usd,
        result.on_demand_percent_used, result.on_demand_used_usd,
        result.on_demand_limit_usd.map(|v| format!("{:.2}", v)).unwrap_or("unlimited".to_string()),
        result.membership_type
    );

    Ok(result)
}

#[tauri::command]
async fn fetch_claude_usage() -> Result<ClaudeUsageData, String> {
    println!("[cursor-juice] fetch_claude_usage called");
    match fetch_claude_usage_oauth().await {
        Ok(data) => {
            println!("[cursor-juice] Claude auth: OAuth usage fetch succeeded.");
            Ok(data)
        }
        Err(oauth_err) => {
            if oauth_err.contains("No usable Claude OAuth credentials available") {
                println!(
                    "[cursor-juice] Claude OAuth credentials unavailable; using web fallback..."
                );
            } else {
                println!(
                    "[cursor-juice] Claude OAuth failed ({}), attempting web fallback...",
                    oauth_err
                );
            }
            fetch_claude_usage_web().await.map_err(|web_err| {
                format!(
                    "Claude OAuth failed: {}. Claude web fallback failed: {}",
                    oauth_err, web_err
                )
            })
        }
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Mutex<Settings>>) -> Settings {
    state.lock().unwrap().clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(load_settings()))
        .invoke_handler(tauri::generate_handler![
            fetch_cursor_usage,
            fetch_claude_usage,
            get_settings
        ])
        .setup(|app| {
            let settings = load_settings();

            let show_plan_item =
                CheckMenuItemBuilder::with_id("show_plan", "Show Plan Usage")
                    .checked(settings.show_plan)
                    .build(app)?;
            let show_od_item =
                CheckMenuItemBuilder::with_id("show_on_demand", "Show On-Demand Usage")
                    .checked(settings.show_on_demand)
                    .build(app)?;
            let show_claude_item =
                CheckMenuItemBuilder::with_id("show_claude", "Show Claude Usage")
                    .checked(settings.show_claude_window)
                    .build(app)?;

            let about_item = PredefinedMenuItem::about(app, Some("About Cursor Juice"), None)?;
            let quit_item = PredefinedMenuItem::quit(app, Some("Quit Cursor Juice"))?;

            let app_menu = SubmenuBuilder::new(app, "Cursor Juice")
                .item(&about_item)
                .separator()
                .item(&show_plan_item)
                .item(&show_od_item)
                .item(&show_claude_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_menu])
                .build()?;

            app.set_menu(menu)?;
            if settings.show_claude_window {
                let _ = create_claude_window(&app.handle());
            }

            app.on_menu_event(move |app_handle, event| {
                let id = event.id().0.as_str();
                match id {
                    "show_plan" | "show_on_demand" | "show_claude" => {
                        let state = app_handle.state::<Mutex<Settings>>();
                        let mut settings = state.lock().unwrap();
                        if id == "show_plan" {
                            settings.show_plan =
                                show_plan_item.is_checked().unwrap_or(true);
                        } else if id == "show_claude" {
                            settings.show_claude_window =
                                show_claude_item.is_checked().unwrap_or(false);
                            if settings.show_claude_window {
                                let _ = create_claude_window(app_handle);
                            } else if let Some(window) = app_handle.get_webview_window("claude") {
                                let _ = window.close();
                            }
                        } else {
                            settings.show_on_demand =
                                show_od_item.is_checked().unwrap_or(true);
                        }
                        save_settings(&settings);
                        let _ = app_handle.emit("settings-changed", settings.clone());
                        println!(
                            "[cursor-juice] Settings changed: show_plan={}, show_on_demand={}, show_claude_window={}",
                            settings.show_plan, settings.show_on_demand, settings.show_claude_window
                        );
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
