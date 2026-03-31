/// Telemetry consent management — GDPR/CCPA compliance.
///
/// Stores the user's telemetry consent preference via tauri-plugin-store.
/// The grid worker checks this before sending machine identifiers (hostname,
/// CPU cores, RAM) to the coordinator.  When consent is not granted, those
/// fields are replaced with anonymous placeholders.
///
/// Store file: `telemetry.json`
///   - key `"consented"` — bool (true = opted in, false = opted out)
///   - key `"decided"`   — bool (true = user has seen the prompt)

use log::info;

const STORE_FILE: &str = "telemetry.json";
const KEY_CONSENTED: &str = "consented";
const KEY_DECIDED: &str = "decided";

/// Return the current telemetry consent state.
///
/// Returns a JSON object:
/// ```json
/// { "consented": false, "decided": false }
/// ```
/// `decided` is false when the user has never been shown the consent dialog.
#[tauri::command]
pub async fn get_telemetry_consent(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;

    let consented = store
        .get(KEY_CONSENTED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let decided = store
        .get(KEY_DECIDED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "consented": consented,
        "decided": decided,
    }))
}

/// Set the telemetry consent preference.
///
/// `consented`: true = user opted in, false = user opted out.
/// Also sets `decided = true` so the dialog is not shown again.
#[tauri::command]
pub async fn set_telemetry_consent(
    app: tauri::AppHandle,
    consented: bool,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;

    store.set(KEY_CONSENTED, serde_json::Value::Bool(consented));
    store.set(KEY_DECIDED, serde_json::Value::Bool(true));
    store.save().map_err(|e| e.to_string())?;

    info!(
        "telemetry_consent: user {} telemetry data sharing",
        if consented { "accepted" } else { "declined" }
    );

    Ok(true)
}

/// Check whether telemetry is consented by reading the store file directly.
///
/// This is called from Rust code (e.g. the grid worker auto-start in lib.rs)
/// where there is no IPC context.  Returns `false` when the store file does
/// not exist or the key is missing (consent is opt-in).
pub fn is_telemetry_consented() -> bool {
    let store_dir = dirs::data_dir()
        .unwrap_or_default()
        .join("AuraAlpha");
    let store_path = store_dir.join(STORE_FILE);

    if !store_path.exists() {
        return false;
    }

    match std::fs::read_to_string(&store_path) {
        Ok(contents) => {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&contents) {
                data.get(KEY_CONSENTED)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
