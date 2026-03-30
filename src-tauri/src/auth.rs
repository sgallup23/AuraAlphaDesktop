//! Auth token persistence — secure storage via OS keychain.
//!
//! Migrated from tauri-plugin-store (unencrypted JSON) to OS-native keychain
//! via the `keyring` crate. Falls back to encrypted file if keychain unavailable.

const SERVICE_NAME: &str = "cc.auraalpha.desktop";

// ── Keyring helpers ──────────────────────────────────────────────────

fn keyring_set(key: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.set_password(value)
        .map_err(|e| format!("Keyring set error: {}", e))
}

fn keyring_get(key: &str) -> Result<String, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.get_password()
        .map_err(|e| format!("Keyring get error: {}", e))
}

fn keyring_delete(key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Keyring delete error: {}", e)),
    }
}

// ── Encrypted file fallback (for headless Linux) ─────────────────────

mod auth_fallback {
    use sha2::{Sha256, Digest};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn derive_key() -> [u8; 32] {
        let mut hasher = Sha256::new();
        if let Ok(host) = hostname::get() {
            hasher.update(host.to_string_lossy().as_bytes());
        }
        hasher.update(
            std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "unknown".to_string())
                .as_bytes(),
        );
        if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
            hasher.update(mid.trim().as_bytes());
        }
        hasher.update(b"cc.auraalpha.desktop.auth-fallback.v1");
        hasher.finalize().into()
    }

    fn xor_crypt(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
        data.iter().enumerate().map(|(i, b)| b ^ key[i % 32]).collect()
    }

    fn fallback_path() -> PathBuf {
        let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("com.auraalpha.desktop");
        path.push("auth.enc");
        path
    }

    fn load_store() -> HashMap<String, String> {
        let path = fallback_path();
        if !path.exists() {
            return HashMap::new();
        }
        let encrypted = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        };
        let key = derive_key();
        let decrypted = xor_crypt(&encrypted, &key);
        String::from_utf8(decrypted)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_store(store: &HashMap<String, String>) -> Result<(), String> {
        let path = fallback_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create auth dir: {}", e))?;
        }
        let json = serde_json::to_string(store).map_err(|e| e.to_string())?;
        let key = derive_key();
        let encrypted = xor_crypt(json.as_bytes(), &key);
        let tmp = path.with_extension("enc.tmp");
        std::fs::write(&tmp, &encrypted)
            .map_err(|e| format!("Cannot write auth store: {}", e))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| format!("Cannot rename auth store: {}", e))?;
        Ok(())
    }

    pub fn set(key: &str, value: &str) -> Result<(), String> {
        let mut store = load_store();
        store.insert(key.to_string(), value.to_string());
        save_store(&store)
    }

    pub fn get(key: &str) -> Option<String> {
        load_store().get(key).cloned()
    }

    pub fn delete(key: &str) -> Result<(), String> {
        let mut store = load_store();
        store.remove(key);
        save_store(&store)
    }

    #[allow(dead_code)]
    pub fn clear_all() -> Result<(), String> {
        save_store(&HashMap::new())
    }
}

// ── Secure get/set with auto-fallback ────────────────────────────────

fn secure_set(key: &str, value: &str) -> Result<(), String> {
    match keyring_set(&format!("auth:{}", key), value) {
        Ok(()) => Ok(()),
        Err(e) => {
            log::warn!("Keyring unavailable for auth:{}: {}. Using fallback.", key, e);
            auth_fallback::set(key, value)
        }
    }
}

fn secure_get(key: &str) -> Option<String> {
    match keyring_get(&format!("auth:{}", key)) {
        Ok(val) => Some(val),
        Err(_) => auth_fallback::get(key),
    }
}

fn secure_delete(key: &str) -> Result<(), String> {
    let _ = keyring_delete(&format!("auth:{}", key));
    let _ = auth_fallback::delete(key);
    Ok(())
}

// ── IPC commands (same signatures as before) ─────────────────────────

/// IPC command: save auth tokens to secure storage (OS keychain or encrypted fallback)
#[tauri::command]
pub async fn save_auth_token(
    _app: tauri::AppHandle,
    access_token: String,
    refresh_token: String,
    user_json: String,
) -> Result<bool, String> {
    secure_set("access_token", &access_token)?;
    secure_set("refresh_token", &refresh_token)?;
    secure_set("user", &user_json)?;
    log::info!("Auth tokens saved to secure storage");
    Ok(true)
}

/// IPC command: load auth tokens from secure storage
#[tauri::command]
pub async fn load_auth_token(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let access = secure_get("access_token")
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null);
    let refresh = secure_get("refresh_token")
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null);
    let user = secure_get("user")
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null);

    Ok(serde_json::json!({
        "access_token": access,
        "refresh_token": refresh,
        "user": user
    }))
}

/// IPC command: clear auth tokens from secure storage
#[tauri::command]
pub async fn clear_auth_token(_app: tauri::AppHandle) -> Result<bool, String> {
    secure_delete("access_token")?;
    secure_delete("refresh_token")?;
    secure_delete("user")?;
    log::info!("Auth tokens cleared from secure storage");
    Ok(true)
}

/// Migrate any existing plaintext auth.json from tauri-plugin-store to secure storage.
/// Call once at startup.
pub fn migrate_plaintext_auth_if_exists() {
    // tauri-plugin-store saves to the app data directory
    let mut path = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("com.auraalpha.desktop");
    path.push("auth.json");

    if !path.exists() {
        return;
    }

    log::warn!("Found plaintext auth.json — migrating to secure storage");

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Cannot read plaintext auth.json for migration: {}", e);
            return;
        }
    };

    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Cannot parse plaintext auth.json: {}", e);
            return;
        }
    };

    let mut migrated = 0;
    for key in &["access_token", "refresh_token", "user"] {
        if let Some(val) = data.get(key).and_then(|v| v.as_str()) {
            if secure_set(key, val).is_ok() {
                migrated += 1;
            }
        }
    }

    if migrated > 0 {
        log::info!("Migrated {} auth tokens to secure storage", migrated);
        if let Err(e) = std::fs::remove_file(&path) {
            log::error!("Cannot delete plaintext auth.json: {}", e);
        } else {
            log::info!("Deleted plaintext auth.json");
        }
    }
}
