use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::models::ClaudeOAuthBlob;

const KEYCHAIN_SERVICE: &str = "app.claudeusage.desktop";
const KEYCHAIN_ACCOUNT: &str = "claude-oauth";

// --- Token storage abstraction ---

trait TokenStore {
    fn read(&self) -> Result<ClaudeOAuthBlob, String>;
    fn write(&self, blob: &ClaudeOAuthBlob) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

// --- macOS Keychain backend (release builds) ---

struct KeychainStore;

impl TokenStore for KeychainStore {
    fn read(&self) -> Result<ClaudeOAuthBlob, String> {
        let data = get_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .map_err(|e| format!("Keychain lookup failed: {}", e))?;
        let json = String::from_utf8(data.to_vec())
            .map_err(|_| "Keychain data was not valid UTF-8.".to_string())?;
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse keychain JSON: {}", e))
    }

    fn write(&self, blob: &ClaudeOAuthBlob) -> Result<(), String> {
        let json =
            serde_json::to_string(blob).map_err(|e| format!("Failed to serialize: {}", e))?;
        set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, json.as_bytes())
            .map_err(|e| format!("Failed to write keychain: {}", e))
    }

    fn delete(&self) -> Result<(), String> {
        delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .map_err(|e| format!("Failed to clear token: {}", e))
    }
}

// --- File backend (dev builds — avoids keychain prompts on every rebuild) ---

struct FileStore;

impl FileStore {
    fn token_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("claude-usage")
            .join("oauth-token.json")
    }
}

impl TokenStore for FileStore {
    fn read(&self) -> Result<ClaudeOAuthBlob, String> {
        let path = Self::token_path();
        let contents =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read token file: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse token file: {}", e))
    }

    fn write(&self, blob: &ClaudeOAuthBlob) -> Result<(), String> {
        let path = Self::token_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(blob)
            .map_err(|e| format!("Failed to serialize token: {}", e))?;
        fs::write(&path, &json).map_err(|e| format!("Failed to write token file: {}", e))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to set file permissions: {}", e))?;
        Ok(())
    }

    fn delete(&self) -> Result<(), String> {
        let path = Self::token_path();
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete token file: {}", e))?;
        }
        Ok(())
    }
}

// --- Single dispatch point: file in debug, keychain in release ---
// Uses cfg!() (not #[cfg()]) so both backends are always type-checked.
// The compiler dead-code-eliminates the unused branch.

fn store() -> &'static dyn TokenStore {
    static FILE: FileStore = FileStore;
    static KEYCHAIN: KeychainStore = KeychainStore;

    if cfg!(debug_assertions) {
        &FILE
    } else {
        &KEYCHAIN
    }
}

// --- Public API (unchanged signatures) ---

pub(crate) fn read_keychain_oauth_blob() -> Result<ClaudeOAuthBlob, String> {
    store().read()
}

pub(crate) fn write_keychain_oauth_blob(blob: &ClaudeOAuthBlob) -> Result<(), String> {
    store().write(blob)
}

pub(crate) fn delete_keychain_oauth_blob() -> Result<(), String> {
    store().delete()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::now_ms;

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
}
