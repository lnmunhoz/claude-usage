use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::models::ClaudeOAuthBlob;

pub(crate) const KEYCHAIN_SERVICE: &str = "app.claudeusage.desktop";
pub(crate) const KEYCHAIN_ACCOUNT: &str = "claude-oauth";

pub(crate) fn read_keychain_oauth_blob() -> Result<ClaudeOAuthBlob, String> {
    let data = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain lookup failed: {}", e))?;
    let json = String::from_utf8(data.to_vec())
        .map_err(|_| "Keychain data was not valid UTF-8.".to_string())?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to parse keychain JSON: {}", e))
}

pub(crate) fn write_keychain_oauth_blob(blob: &ClaudeOAuthBlob) -> Result<(), String> {
    let json = serde_json::to_string(blob)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, json.as_bytes())
        .map_err(|e| format!("Failed to write keychain: {}", e))
}

pub(crate) fn delete_keychain_oauth_blob() -> Result<(), String> {
    delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Failed to clear token: {}", e))
}

pub(crate) fn validate_claude_oauth_access_token(
    access_token: &str,
    source: &str,
) -> Result<(), String> {
    if access_token.starts_with("sk-ant-oat") {
        return Ok(());
    }
    Err(format!(
        "Claude OAuth token from {} is not an OAuth access token.",
        source
    ))
}
