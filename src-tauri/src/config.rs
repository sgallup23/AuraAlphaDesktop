//! Runtime configuration module for Aura Alpha Desktop.
//!
//! This is the ONLY place in the application that contains hardcoded URLs.
//! All other modules receive URLs through `AppConfig` at runtime.
//!
//! Boot sequence:
//!   1. Load cached config from disk (or use compiled-in defaults).
//!   2. Attempt a single HTTP GET to `BOOTSTRAP_URL`.
//!   3. Merge server values over cached values (server always wins).
//!   4. Persist merged config to disk for next boot.
//!   5. Store in Tauri managed state; expose to frontend via `get_config` IPC.

use dirs;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

// ─── Single hardcoded URL ────────────────────────────────────────────────────

/// The only hardcoded URL in the entire application.
/// Everything else comes from the config returned by this endpoint.
const BOOTSTRAP_URL: &str = "https://auraalpha.cc/api/desktop/config";

// ─── Config struct ───────────────────────────────────────────────────────────

/// All runtime settings for the desktop application.
///
/// Fields are intentionally `pub` so consumers (tray, worker, updater) can
/// read them directly from Tauri managed state without boilerplate accessors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Base URL for all API calls (e.g. `https://auraalpha.cc/api`).
    pub api_base: String,

    /// WebSocket / SSE coordinator URL for real-time signals.
    pub coordinator_url: String,

    /// Endpoint polled by `tauri-plugin-updater` for app updates.
    pub update_endpoint: String,

    /// Whether the background grid worker sidecar should be launched.
    pub worker_enabled: bool,

    /// Maximum number of parallel tasks the grid worker may run.
    pub worker_max_parallel: u32,

    /// Semver string — if the running app version is below this, show the
    /// upgrade-required screen instead of the normal UI.
    pub minimum_version: String,

    /// Arbitrary boolean flags for progressive feature rollout.
    /// Keys are feature names; `true` means the feature is enabled.
    pub feature_flags: HashMap<String, bool>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_base: "https://auraalpha.cc/api".into(),
            coordinator_url: "https://auraalpha.cc".into(),
            update_endpoint: "https://auraalpha.cc/api/desktop/update".into(),
            worker_enabled: true,
            worker_max_parallel: 2,
            minimum_version: "0.0.0".into(),
            feature_flags: HashMap::new(),
        }
    }
}

// ─── Tauri managed-state wrapper ─────────────────────────────────────────────

/// Holds the live `AppConfig` inside Tauri's managed-state system.
///
/// Uses `RwLock` because reads (from every IPC command) vastly outnumber
/// writes (only during `initialize()` and future hot-reload calls).
pub struct AppConfigState(pub RwLock<AppConfig>);

impl AppConfigState {
    pub fn new(cfg: AppConfig) -> Self {
        Self(RwLock::new(cfg))
    }
}

// ─── Disk helpers ────────────────────────────────────────────────────────────

/// Returns the OS-appropriate path for the persisted config file.
///
/// | OS      | Example path                                                        |
/// |---------|---------------------------------------------------------------------|
/// | Windows | `%LOCALAPPDATA%\cc.auraalpha.desktop\config.json`                   |
/// | macOS   | `~/Library/Application Support/cc.auraalpha.desktop/config.json`    |
/// | Linux   | `~/.local/share/cc.auraalpha.desktop/config.json`                   |
pub fn config_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cc.auraalpha.desktop")
        .join("config.json")
}

/// Reads and deserialises the cached config from disk.
///
/// Returns `AppConfig::default()` if the file is missing, unreadable, or
/// contains invalid JSON — this covers the very first cold start.
pub fn load_cached() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<AppConfig>(&contents) {
            Ok(cfg) => {
                info!("config: loaded cached config from {}", path.display());
                cfg
            }
            Err(e) => {
                warn!(
                    "config: failed to parse cached config at {}: {e} — using defaults",
                    path.display()
                );
                AppConfig::default()
            }
        },
        Err(_) => {
            info!(
                "config: no cached config at {} — using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

/// Persists `cfg` to the cache file, creating parent directories as needed.
///
/// Errors are logged but not propagated — a write failure must never crash
/// the app; the in-memory config is still valid.
fn save_to_disk(cfg: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("config: could not create config dir {}: {e}", parent.display());
            return;
        }
    }
    match serde_json::to_string_pretty(cfg) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, &json) {
                warn!("config: could not write config to {}: {e}", path.display());
            } else {
                info!("config: saved config to {}", path.display());
            }
        }
        Err(e) => warn!("config: could not serialise config: {e}"),
    }
}

// ─── Server fetch ────────────────────────────────────────────────────────────

/// Performs a single HTTP GET to `BOOTSTRAP_URL` and deserialises the response.
///
/// Returns `Err(String)` on any network error, non-200 status, or JSON parse
/// failure.  The caller (`initialize`) treats all errors the same way: fall
/// back to the cached/default config.
pub async fn fetch_from_server() -> Result<AppConfig, String> {
    info!("config: fetching remote config from {BOOTSTRAP_URL}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("config: could not build HTTP client: {e}"))?;

    let response = client
        .get(BOOTSTRAP_URL)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("config: GET {BOOTSTRAP_URL} failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "config: server returned {} for {BOOTSTRAP_URL}",
            response.status()
        ));
    }

    response
        .json::<AppConfig>()
        .await
        .map_err(|e| format!("config: could not deserialise server response: {e}"))
}

// ─── Merge helper ────────────────────────────────────────────────────────────

/// Merges `server` on top of `base`.
///
/// Server values overwrite every field in `base`.  For `feature_flags`,
/// the server map is merged key-by-key so that flags not mentioned by the
/// server retain their cached value (useful for gradual rollout).
fn merge(base: AppConfig, server: AppConfig) -> AppConfig {
    let mut merged_flags = base.feature_flags.clone();
    for (k, v) in server.feature_flags {
        merged_flags.insert(k, v);
    }

    AppConfig {
        api_base: server.api_base,
        coordinator_url: server.coordinator_url,
        update_endpoint: server.update_endpoint,
        worker_enabled: server.worker_enabled,
        worker_max_parallel: server.worker_max_parallel,
        minimum_version: server.minimum_version,
        feature_flags: merged_flags,
    }
}

// ─── Public initializer ──────────────────────────────────────────────────────

/// Full boot-time config initialisation.
///
/// Call this **once** during Tauri's `setup` hook, then store the result in
/// `AppConfigState` managed state.
///
/// Sequence:
/// 1. Load disk cache (or defaults on first run).
/// 2. Attempt to fetch from `BOOTSTRAP_URL` (5 s timeout).
/// 3. On success: merge server values over cache, persist to disk.
/// 4. On failure: log a warning, keep the cached/default config.
/// 5. Return the final `AppConfig`.
pub async fn initialize() -> AppConfig {
    let cached = load_cached();

    match fetch_from_server().await {
        Ok(server_cfg) => {
            info!("config: remote config fetched successfully");
            let merged = merge(cached, server_cfg);
            save_to_disk(&merged);
            merged
        }
        Err(e) => {
            warn!("config: using cached/default config — reason: {e}");
            cached
        }
    }
}

// ─── Tauri IPC command ───────────────────────────────────────────────────────

/// IPC command — returns a snapshot of the current `AppConfig` to the frontend.
///
/// Register this with `tauri::Builder::invoke_handler` in `lib.rs`:
/// ```rust
/// tauri::generate_handler![config::get_config, /* … */]
/// ```
///
/// Frontend usage (Tauri v2 JS API):
/// ```js
/// import { invoke } from "@tauri-apps/api/core";
/// const cfg = await invoke("get_config");
/// ```
#[tauri::command]
pub async fn get_config(
    state: tauri::State<'_, AppConfigState>,
) -> Result<AppConfig, String> {
    state
        .0
        .read()
        .map(|guard| guard.clone())
        .map_err(|e| format!("config: state lock poisoned: {e}"))
}
