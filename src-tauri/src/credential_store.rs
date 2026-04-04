//! Credential Store — secure broker credential management using OS keychain.
//!
//! Primary: OS-native keychain via `keyring` crate
//!   - Windows: Windows Credential Manager (DPAPI)
//!   - macOS: Keychain
//!   - Linux: Secret Service (GNOME Keyring / KWallet)
//!
//! Fallback: Encrypted file with machine-specific key (for headless Linux, etc.)
//! Credentials never leave the user's machine.

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

const SERVICE_NAME: &str = "cc.auraalpha.desktop";

/// Broker credential field definitions
#[derive(Clone, Debug, Serialize)]
pub struct CredentialField {
    pub name: String,
    pub label: String,
    pub field_type: String, // "text", "password", "number"
    pub required: bool,
    pub placeholder: String,
}

/// Available broker info for UI display
#[derive(Clone, Debug, Serialize)]
pub struct BrokerInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub commission: String,
    pub fractional: bool,
    pub paper_trading: bool,
    pub risk_tier: String,       // "official", "official_gateway", "unofficial"
    pub waiver_required: bool,
    pub waiver_text: Option<String>,
    pub credential_fields: Vec<CredentialField>,
}

const UNOFFICIAL_WAIVER: &str = "\
WARNING: This broker integration uses an UNOFFICIAL, community-maintained API that is NOT \
endorsed by the broker. Using it may violate the broker's Terms of Service and could result \
in account restrictions or closure. The API may break without notice when the broker updates \
their platform. Aura Alpha provides this integration as-is with NO WARRANTY. By proceeding, \
you acknowledge and accept ALL risks including but not limited to: account termination, order \
execution failures, data inaccuracies, and financial loss.";

const GATEWAY_WAIVER: &str = "\
This broker requires a local gateway application running on your machine. You are responsible \
for keeping the gateway running, updated, and properly configured. Connection interruptions \
may result in missed trades or unmanaged positions.";

/// Get credential field definitions for all supported brokers
pub fn get_broker_definitions() -> Vec<BrokerInfo> {
    vec![
        // ── Official APIs ────────────────────────────────────────
        BrokerInfo {
            id: "ibkr".to_string(),
            name: "Interactive Brokers".to_string(),
            description: "Full-featured broker via IB Gateway (ib_async)".to_string(),
            commission: "tiered".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official_gateway".to_string(),
            waiver_required: true,
            waiver_text: Some(GATEWAY_WAIVER.to_string()),
            credential_fields: vec![
                CredentialField { name: "IBKR_HOST".into(), label: "Gateway Host".into(), field_type: "text".into(), required: false, placeholder: "127.0.0.1".into() },
                CredentialField { name: "IBKR_PORT".into(), label: "Gateway Port".into(), field_type: "number".into(), required: false, placeholder: "4001".into() },
                CredentialField { name: "IBKR_ACCOUNT".into(), label: "Account ID".into(), field_type: "text".into(), required: false, placeholder: "U1234567".into() },
                CredentialField { name: "IBKR_CLIENT_ID_TRADING".into(), label: "Client ID".into(), field_type: "number".into(), required: false, placeholder: "22".into() },
            ],
        },
        BrokerInfo {
            id: "alpaca".to_string(),
            name: "Alpaca Markets".to_string(),
            description: "Commission-free broker with fractional shares (alpaca-py SDK)".to_string(),
            commission: "free".to_string(),
            fractional: true,
            paper_trading: true,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "ALPACA_API_KEY".into(), label: "API Key".into(), field_type: "text".into(), required: true, placeholder: "PK...".into() },
                CredentialField { name: "ALPACA_SECRET_KEY".into(), label: "Secret Key".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ALPACA_PAPER".into(), label: "Paper Trading".into(), field_type: "text".into(), required: false, placeholder: "true".into() },
            ],
        },
        BrokerInfo {
            id: "tradier".to_string(),
            name: "Tradier".to_string(),
            description: "Commission-free stocks, ETFs, and options via REST API".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "TRADIER_ACCESS_TOKEN".into(), label: "Access Token".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TRADIER_ACCOUNT_ID".into(), label: "Account ID".into(), field_type: "text".into(), required: false, placeholder: "VA12345678".into() },
                CredentialField { name: "TRADIER_SANDBOX".into(), label: "Sandbox Mode".into(), field_type: "text".into(), required: false, placeholder: "true".into() },
            ],
        },
        BrokerInfo {
            id: "tastytrade".to_string(),
            name: "Tastytrade".to_string(),
            description: "Stocks, ETFs, options, futures, and crypto via REST API".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "TASTYTRADE_USERNAME".into(), label: "Username".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TASTYTRADE_PASSWORD".into(), label: "Password".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TASTYTRADE_ACCOUNT".into(), label: "Account Number".into(), field_type: "text".into(), required: false, placeholder: "5YZ12345".into() },
                CredentialField { name: "TASTYTRADE_SANDBOX".into(), label: "Sandbox Mode".into(), field_type: "text".into(), required: false, placeholder: "true".into() },
            ],
        },
        BrokerInfo {
            id: "public".to_string(),
            name: "Public.com".to_string(),
            description: "Commission-free with fractional shares, stocks/ETFs/options/crypto".to_string(),
            commission: "free".to_string(),
            fractional: true,
            paper_trading: false,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "PUBLIC_API_KEY".into(), label: "API Key".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "PUBLIC_ACCOUNT_ID".into(), label: "Account ID".into(), field_type: "text".into(), required: false, placeholder: "".into() },
            ],
        },
        BrokerInfo {
            id: "schwab".to_string(),
            name: "Charles Schwab".to_string(),
            description: "Stocks, ETFs, options via official Schwab API (OAuth 2.0)".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: false,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "SCHWAB_CLIENT_ID".into(), label: "Client ID (App Key)".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "SCHWAB_CLIENT_SECRET".into(), label: "Client Secret".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "SCHWAB_REFRESH_TOKEN".into(), label: "Refresh Token".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "SCHWAB_ACCOUNT_HASH".into(), label: "Account Hash".into(), field_type: "text".into(), required: false, placeholder: "".into() },
            ],
        },
        BrokerInfo {
            id: "etrade".to_string(),
            name: "E*TRADE (Morgan Stanley)".to_string(),
            description: "Stocks, ETFs, options via official E*TRADE API (OAuth 1.0a)".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "ETRADE_CONSUMER_KEY".into(), label: "Consumer Key".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ETRADE_CONSUMER_SECRET".into(), label: "Consumer Secret".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ETRADE_ACCESS_TOKEN".into(), label: "Access Token".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ETRADE_ACCESS_SECRET".into(), label: "Access Secret".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ETRADE_ACCOUNT_ID_KEY".into(), label: "Account ID Key".into(), field_type: "text".into(), required: false, placeholder: "".into() },
                CredentialField { name: "ETRADE_SANDBOX".into(), label: "Sandbox Mode".into(), field_type: "text".into(), required: false, placeholder: "true".into() },
            ],
        },
        BrokerInfo {
            id: "tradestation".to_string(),
            name: "TradeStation".to_string(),
            description: "Stocks, ETFs, options, futures via official TradeStation API (OAuth 2.0)".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official".to_string(),
            waiver_required: false,
            waiver_text: None,
            credential_fields: vec![
                CredentialField { name: "TRADESTATION_CLIENT_ID".into(), label: "Client ID".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TRADESTATION_CLIENT_SECRET".into(), label: "Client Secret".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TRADESTATION_REFRESH_TOKEN".into(), label: "Refresh Token".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "TRADESTATION_ACCOUNT_ID".into(), label: "Account ID".into(), field_type: "text".into(), required: false, placeholder: "".into() },
                CredentialField { name: "TRADESTATION_SIM".into(), label: "Simulation Mode".into(), field_type: "text".into(), required: false, placeholder: "true".into() },
            ],
        },

        // ── Unofficial / Gateway-Dependent (Waiver Required) ─────
        BrokerInfo {
            id: "webull".to_string(),
            name: "Webull".to_string(),
            description: "UNOFFICIAL API — community-maintained, may violate broker ToS".to_string(),
            commission: "free".to_string(),
            fractional: true,
            paper_trading: false,
            risk_tier: "unofficial".to_string(),
            waiver_required: true,
            waiver_text: Some(UNOFFICIAL_WAIVER.to_string()),
            credential_fields: vec![
                CredentialField { name: "WEBULL_EMAIL".into(), label: "Email".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "WEBULL_PASSWORD".into(), label: "Password".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "WEBULL_TRADING_PIN".into(), label: "Trading PIN".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "WEBULL_DEVICE_ID".into(), label: "Device ID".into(), field_type: "text".into(), required: false, placeholder: "(auto-generated)".into() },
            ],
        },
        BrokerInfo {
            id: "robinhood".to_string(),
            name: "Robinhood".to_string(),
            description: "UNOFFICIAL API — community-maintained, may violate broker ToS, no sandbox".to_string(),
            commission: "free".to_string(),
            fractional: true,
            paper_trading: false,
            risk_tier: "unofficial".to_string(),
            waiver_required: true,
            waiver_text: Some(UNOFFICIAL_WAIVER.to_string()),
            credential_fields: vec![
                CredentialField { name: "ROBINHOOD_USERNAME".into(), label: "Username / Email".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ROBINHOOD_PASSWORD".into(), label: "Password".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "ROBINHOOD_MFA_CODE".into(), label: "MFA Code (if 2FA)".into(), field_type: "text".into(), required: false, placeholder: "".into() },
            ],
        },
        BrokerInfo {
            id: "moomoo".to_string(),
            name: "Moomoo (Futu)".to_string(),
            description: "Official OpenAPI SDK — requires OpenD gateway daemon running locally".to_string(),
            commission: "per_order".to_string(),
            fractional: false,
            paper_trading: true,
            risk_tier: "official_gateway".to_string(),
            waiver_required: true,
            waiver_text: Some(GATEWAY_WAIVER.to_string()),
            credential_fields: vec![
                CredentialField { name: "MOOMOO_HOST".into(), label: "OpenD Host".into(), field_type: "text".into(), required: false, placeholder: "127.0.0.1".into() },
                CredentialField { name: "MOOMOO_PORT".into(), label: "OpenD Port".into(), field_type: "number".into(), required: false, placeholder: "11111".into() },
                CredentialField { name: "MOOMOO_TRADING_PWD".into(), label: "Trading Password".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "MOOMOO_TRADING_ENV".into(), label: "Environment".into(), field_type: "text".into(), required: false, placeholder: "SIMULATE".into() },
                CredentialField { name: "MOOMOO_ACC_ID".into(), label: "Account ID".into(), field_type: "text".into(), required: false, placeholder: "".into() },
            ],
        },
        BrokerInfo {
            id: "firstrade".to_string(),
            name: "Firstrade".to_string(),
            description: "UNOFFICIAL API — limited functionality, community-maintained, may violate ToS".to_string(),
            commission: "free".to_string(),
            fractional: false,
            paper_trading: false,
            risk_tier: "unofficial".to_string(),
            waiver_required: true,
            waiver_text: Some(UNOFFICIAL_WAIVER.to_string()),
            credential_fields: vec![
                CredentialField { name: "FIRSTRADE_USERNAME".into(), label: "Username".into(), field_type: "text".into(), required: true, placeholder: "".into() },
                CredentialField { name: "FIRSTRADE_PASSWORD".into(), label: "Password".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "FIRSTRADE_PIN".into(), label: "PIN".into(), field_type: "password".into(), required: true, placeholder: "".into() },
                CredentialField { name: "FIRSTRADE_ACCOUNT".into(), label: "Account Number".into(), field_type: "text".into(), required: false, placeholder: "".into() },
            ],
        },
    ]
}

// ── Keyring-based secure storage ──────────────────────────────────────

/// Try to store data in OS keychain. Returns Ok(()) on success, Err on failure.
fn keyring_set(username: &str, data: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, username)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.set_password(data)
        .map_err(|e| format!("Keyring set error: {}", e))
}

/// Try to load data from OS keychain. Returns Ok(data) or Err.
fn keyring_get(username: &str) -> Result<String, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, username)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.get_password()
        .map_err(|e| format!("Keyring get error: {}", e))
}

/// Try to delete data from OS keychain.
fn keyring_delete(username: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, username)
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        // Not found is fine — already deleted
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Keyring delete error: {}", e)),
    }
}

// ── Encrypted file fallback ───────────────────────────────────────────
//
// For headless Linux or environments without a Secret Service daemon,
// we fall back to an encrypted JSON file. The encryption key is derived
// from a machine-specific fingerprint (hostname + username + machine-id).

mod encrypted_fallback {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use aes_gcm::aead::Aead;
    use rand::RngCore;
    use sha2::{Sha256, Digest};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// AES-256-GCM nonce size: 12 bytes
    const NONCE_SIZE: usize = 12;

    /// Magic prefix to identify AES-GCM encrypted data (vs legacy XOR)
    const AES_GCM_MAGIC: &[u8; 4] = b"AG01";

    /// Derive a 32-byte encryption key from machine-specific data.
    /// This is NOT a substitute for a proper keychain — it protects against
    /// casual file browsing but not a determined attacker with local access.
    fn derive_machine_key() -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Machine hostname
        if let Ok(host) = hostname::get() {
            hasher.update(host.to_string_lossy().as_bytes());
        }

        // Current OS username
        hasher.update(whoami().as_bytes());

        // Linux machine-id (stable across reboots)
        if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
            hasher.update(mid.trim().as_bytes());
        } else if let Ok(mid) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            hasher.update(mid.trim().as_bytes());
        }

        // Static salt to differentiate from other apps using same approach
        hasher.update(b"cc.auraalpha.desktop.credential-fallback.v1");

        hasher.finalize().into()
    }

    fn whoami() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// AES-256-GCM encryption. Generates a random 12-byte nonce and prepends
    /// a 4-byte magic header + the nonce to the ciphertext.
    /// Output format: [AG01 (4 bytes)][nonce (12 bytes)][ciphertext+tag]
    fn aes_gcm_encrypt(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("AES-GCM key init error: {}", e))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| format!("AES-GCM encrypt error: {}", e))?;

        let mut output = Vec::with_capacity(AES_GCM_MAGIC.len() + NONCE_SIZE + ciphertext.len());
        output.extend_from_slice(AES_GCM_MAGIC);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    /// AES-256-GCM decryption. Expects the format produced by aes_gcm_encrypt.
    fn aes_gcm_decrypt(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let min_len = AES_GCM_MAGIC.len() + NONCE_SIZE + 16; // 16 = GCM tag
        if data.len() < min_len {
            return Err("Data too short for AES-GCM".to_string());
        }
        if &data[..4] != AES_GCM_MAGIC {
            return Err("Missing AES-GCM magic header".to_string());
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("AES-GCM key init error: {}", e))?;

        let nonce = Nonce::from_slice(&data[4..4 + NONCE_SIZE]);
        let ciphertext = &data[4 + NONCE_SIZE..];

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("AES-GCM decrypt error: {}", e))
    }

    /// Legacy XOR decryption for backward compatibility with existing stored
    /// credentials. Will be attempted if AES-GCM decryption fails.
    fn xor_decrypt_legacy(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
        let mut result = Vec::with_capacity(data.len());
        for (i, byte) in data.iter().enumerate() {
            result.push(byte ^ key[i % 32]);
        }
        result
    }

    fn fallback_path() -> PathBuf {
        let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("com.auraalpha.desktop");
        path.push("credentials.enc");
        path
    }

    pub fn load_store() -> HashMap<String, HashMap<String, String>> {
        let path = fallback_path();
        if !path.exists() {
            return HashMap::new();
        }
        let encrypted = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        };
        let key = derive_machine_key();

        // Try AES-GCM first (new format)
        if encrypted.len() >= 4 && &encrypted[..4] == AES_GCM_MAGIC {
            if let Ok(decrypted) = aes_gcm_decrypt(&encrypted, &key) {
                if let Ok(json_str) = String::from_utf8(decrypted) {
                    if let Ok(store) = serde_json::from_str::<HashMap<String, HashMap<String, String>>>(&json_str) {
                        return store;
                    }
                }
            }
        }

        // Fall back to legacy XOR decryption for existing stored credentials
        let decrypted = xor_decrypt_legacy(&encrypted, &key);
        let json_str = match String::from_utf8(decrypted) {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };
        match serde_json::from_str::<HashMap<String, HashMap<String, String>>>(&json_str) {
            Ok(store) => {
                // Re-encrypt with AES-GCM for future reads
                log::info!("Migrating credential store from XOR to AES-256-GCM");
                let _ = save_store(&store);
                store
            }
            Err(_) => HashMap::new(),
        }
    }

    pub fn save_store(store: &HashMap<String, HashMap<String, String>>) -> Result<(), String> {
        let path = fallback_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create store dir: {}", e))?;
        }
        let json = serde_json::to_string(store).map_err(|e| e.to_string())?;
        let key = derive_machine_key();
        let encrypted = aes_gcm_encrypt(json.as_bytes(), &key)?;

        // Atomic write: write to temp file then rename
        let tmp_path = path.with_extension("enc.tmp");
        std::fs::write(&tmp_path, &encrypted)
            .map_err(|e| format!("Cannot write encrypted store: {}", e))?;
        std::fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Cannot rename temp store: {}", e))?;
        Ok(())
    }

    pub fn save_credentials(broker: &str, fields: &HashMap<String, String>) -> Result<(), String> {
        let mut store = load_store();
        store.insert(broker.to_string(), fields.clone());
        save_store(&store)
    }

    pub fn load_credentials(broker: &str) -> Result<HashMap<String, String>, String> {
        let store = load_store();
        store.get(broker).cloned()
            .ok_or_else(|| format!("No credentials for broker: {}", broker))
    }

    pub fn delete_credentials(broker: &str) -> Result<(), String> {
        let mut store = load_store();
        store.remove(broker);
        save_store(&store)
    }

    pub fn list_configured() -> Vec<String> {
        let store = load_store();
        store.keys().cloned().collect()
    }
}

// ── Public API (keyring with fallback) ────────────────────────────────

/// Save credentials for a broker. Tries OS keychain first, falls back to encrypted file.
pub fn save_credentials(broker: &str, fields: &HashMap<String, String>) -> Result<(), String> {
    let json = serde_json::to_string(fields).map_err(|e| e.to_string())?;

    // Try OS keychain first
    match keyring_set(&format!("broker:{}", broker), &json) {
        Ok(()) => {
            log::info!("Saved credentials for '{}' in OS keychain", broker);
            // Also update the broker index in keychain
            let _ = update_broker_index(broker, true);
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "OS keychain unavailable for '{}': {}. Using encrypted file fallback.",
                broker, e
            );
            encrypted_fallback::save_credentials(broker, fields)
        }
    }
}

/// Load credentials for a broker. Tries OS keychain first, falls back to encrypted file.
pub fn load_credentials(broker: &str) -> Result<HashMap<String, String>, String> {
    // Try OS keychain first
    match keyring_get(&format!("broker:{}", broker)) {
        Ok(json) => {
            serde_json::from_str(&json)
                .map_err(|e| format!("Corrupt keychain data for '{}': {}", broker, e))
        }
        Err(_) => {
            // Fallback to encrypted file
            encrypted_fallback::load_credentials(broker)
        }
    }
}

/// Delete credentials for a broker. Removes from both keychain and fallback.
pub fn delete_credentials(broker: &str) -> Result<(), String> {
    // Delete from keychain (ignore errors — might not exist there)
    let _ = keyring_delete(&format!("broker:{}", broker));
    let _ = update_broker_index(broker, false);

    // Also delete from fallback file if it exists
    let _ = encrypted_fallback::delete_credentials(broker);

    Ok(())
}

/// List all configured brokers (those with stored credentials).
pub fn list_configured_brokers() -> Vec<String> {
    // Try keychain broker index first
    if let Ok(json) = keyring_get("broker:_index") {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&json) {
            if !list.is_empty() {
                return list;
            }
        }
    }

    // Fallback to encrypted file
    encrypted_fallback::list_configured()
}

/// Maintain a broker index in the keychain so we can enumerate without filesystem.
fn update_broker_index(broker: &str, add: bool) -> Result<(), String> {
    let mut list: Vec<String> = keyring_get("broker:_index")
        .ok()
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();

    if add {
        if !list.contains(&broker.to_string()) {
            list.push(broker.to_string());
        }
    } else {
        list.retain(|b| b != broker);
    }

    let json = serde_json::to_string(&list).map_err(|e| e.to_string())?;
    keyring_set("broker:_index", &json)
}

/// Migrate any existing plaintext credentials.json to secure storage.
/// Call this once at startup. After migration, the plaintext file is deleted.
pub fn migrate_plaintext_if_exists() {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("com.auraalpha.desktop");
    path.push("credentials.json");

    if !path.exists() {
        return;
    }

    log::warn!("Found plaintext credentials.json — migrating to secure storage");

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Cannot read plaintext credentials for migration: {}", e);
            return;
        }
    };

    let store: HashMap<String, HashMap<String, String>> = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Cannot parse plaintext credentials for migration: {}", e);
            return;
        }
    };

    let mut migrated = 0;
    for (broker, fields) in &store {
        if save_credentials(broker, fields).is_ok() {
            migrated += 1;
        }
    }

    log::info!("Migrated {} broker credential sets to secure storage", migrated);

    // Delete the plaintext file
    if let Err(e) = std::fs::remove_file(&path) {
        log::error!("Cannot delete plaintext credentials.json: {}", e);
    } else {
        log::info!("Deleted plaintext credentials.json");
    }
}
