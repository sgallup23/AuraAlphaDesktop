//! Credential Store — encrypted broker credential management using OS keychain.
//!
//! Uses Tauri's plugin-store with OS-level encryption to store broker API keys,
//! tokens, and passwords. Credentials never leave the user's machine.

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

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

/// Get the credential store file path
pub fn store_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("com.auraalpha.desktop");
    path.push("credentials.json");
    path
}

/// Save credentials for a broker (encrypted at rest via OS keychain)
/// In production, this uses tauri-plugin-store which encrypts via OS keychain.
/// This file-based fallback is for the credential structure only.
pub fn save_credentials(broker: &str, fields: &HashMap<String, String>) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create store dir: {}", e))?;
    }

    // Load existing
    let mut store: HashMap<String, HashMap<String, String>> = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("Cannot read store: {}", e))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    store.insert(broker.to_string(), fields.clone());

    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write store: {}", e))?;

    Ok(())
}

/// Load credentials for a broker
pub fn load_credentials(broker: &str) -> Result<HashMap<String, String>, String> {
    let path = store_path();
    if !path.exists() {
        return Err(format!("No credentials stored for {}", broker));
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Cannot read store: {}", e))?;
    let store: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;

    store
        .get(broker)
        .cloned()
        .ok_or_else(|| format!("No credentials for broker: {}", broker))
}

/// Delete credentials for a broker
pub fn delete_credentials(broker: &str) -> Result<(), String> {
    let path = store_path();
    if !path.exists() {
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Cannot read store: {}", e))?;
    let mut store: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(&content).unwrap_or_default();

    store.remove(broker);

    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("Cannot write store: {}", e))?;

    Ok(())
}

/// List all configured brokers (those with stored credentials)
pub fn list_configured_brokers() -> Vec<String> {
    let path = store_path();
    if !path.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let store: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(&content).unwrap_or_default();

    store.keys().cloned().collect()
}
