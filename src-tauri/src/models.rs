use serde::{Deserialize, Serialize};

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
    pub last_updated: Option<u64>,
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

pub(crate) fn default_display_mode() -> String {
    "remaining".to_string()
}

pub(crate) fn default_poll_interval() -> u64 {
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
pub(crate) struct ClaudeCredentials {
    #[serde(rename = "accessToken", alias = "access_token")]
    pub access_token: Option<String>,
    #[serde(rename = "rateLimitTier", alias = "rate_limit_tier")]
    pub rate_limit_tier: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeOAuthBlob {
    #[serde(rename = "accessToken", alias = "access_token")]
    pub access_token: Option<String>,
    #[serde(rename = "refreshToken", alias = "refresh_token")]
    pub refresh_token: Option<String>,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    pub expires_at: Option<i64>,
    pub scopes: Option<Vec<String>>,
    #[serde(rename = "subscriptionType", alias = "subscription_type")]
    pub subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier", alias = "rate_limit_tier")]
    pub rate_limit_tier: Option<String>,
}

// --- Usage API models ---

#[derive(Debug, Deserialize)]
pub(crate) struct ClaudeUsageWindow {
    pub utilization: Option<f64>,
    pub percent_used: Option<f64>,
    pub percent_left: Option<f64>,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub reset_at: Option<String>,
    pub resets_at: Option<String>,
    pub reset_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ClaudeExtraUsage {
    pub is_enabled: Option<bool>,
    pub used_credits: Option<f64>,
    pub monthly_limit: Option<f64>,
    pub utilization: Option<f64>,
    pub spend: Option<f64>,
    pub limit: Option<f64>,
    pub used: Option<f64>,
    pub monthly_spend: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaudeOAuthUsageResponse {
    pub five_hour: Option<ClaudeUsageWindow>,
    pub seven_day: Option<ClaudeUsageWindow>,
    pub extra_usage: Option<ClaudeExtraUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaudeProfileResponse {
    pub organization: Option<ClaudeProfileOrganization>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaudeProfileOrganization {
    pub organization_type: Option<String>,
    pub rate_limit_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthTokenResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveTokenInput {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}
