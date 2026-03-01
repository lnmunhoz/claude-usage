use serde_json::Value;

use crate::keychain::read_keychain_oauth_blob;
use crate::models::{
    ClaudeOAuthUsageResponse, ClaudeProfileOrganization, ClaudeProfileResponse, ClaudeUsageData,
    ClaudeUsageWindow,
};
use crate::oauth::{load_claude_credentials, refresh_claude_token};

pub(crate) fn plan_display_from_profile(org: &ClaudeProfileOrganization) -> Option<String> {
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

pub(crate) async fn fetch_claude_usage_impl() -> Result<ClaudeUsageData, String> {
    let credentials = load_claude_credentials().await?;
    let access_token = credentials
        .access_token
        .as_deref()
        .ok_or_else(|| "Claude credentials are missing accessToken.".to_string())?;

    println!(
        "[claude-usage] Fetching usage with token starting with: {}...",
        &access_token[..access_token.len().min(20)]
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

    let initial_status = response.status();

    // On 401, attempt token refresh and retry once before reading the body
    let (status, body, access_token) = if initial_status == reqwest::StatusCode::UNAUTHORIZED {
        println!("[claude-usage] Usage API returned 401, attempting token refresh and retry...");

        // Discard the 401 response body
        let _ = response.text().await;

        let oauth_blob =
            read_keychain_oauth_blob().map_err(|e| format!("Re-auth failed: {}", e))?;
        let refresh_tok = oauth_blob
            .refresh_token
            .as_deref()
            .ok_or_else(|| "No refresh token available for retry.".to_string())?;
        let refreshed = refresh_claude_token(refresh_tok, &oauth_blob).await?;
        let new_token = refreshed
            .access_token
            .as_deref()
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
        let retry_body = retry_response
            .text()
            .await
            .map_err(|e| format!("Failed to read retry response: {}", e))?;
        (retry_status, retry_body, new_token)
    } else {
        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read Claude OAuth response body: {}", e))?;
        (initial_status, body, access_token.to_string())
    };

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

#[cfg(test)]
mod tests {
    use super::*;

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
