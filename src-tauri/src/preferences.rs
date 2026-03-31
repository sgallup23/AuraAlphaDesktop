/// Workspace and user-preference IPC commands.
///
/// All persistent data is stored through tauri-plugin-store:
///   - preferences.json  — key/value user preferences
///   - workspaces.json   — named dock-layout snapshots

/// IPC command: save a single user preference
#[tauri::command]
pub async fn save_preference(
    app: tauri::AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("preferences.json").map_err(|e| e.to_string())?;
    store.set(&key, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load all user preferences
#[tauri::command]
pub async fn load_preferences(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("preferences.json").map_err(|e| e.to_string())?;
    let keys = store.keys();
    let mut map = serde_json::Map::new();
    for key in keys {
        if let Some(val) = store.get(&key) {
            map.insert(key, val);
        }
    }
    Ok(serde_json::Value::Object(map))
}

/// IPC command: save a named workspace layout
#[tauri::command]
pub async fn save_workspace(
    app: tauri::AppHandle,
    name: String,
    layout_json: String,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    store.set(&name, serde_json::Value::String(layout_json));
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load a named workspace layout
#[tauri::command]
pub async fn load_workspace(app: tauri::AppHandle, name: String) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    match store.get(&name) {
        Some(serde_json::Value::String(s)) => Ok(s),
        _ => Err("Workspace not found".into()),
    }
}

/// IPC command: list all saved workspace names
#[tauri::command]
pub async fn list_workspaces(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    Ok(store.keys().into_iter().collect())
}

/// IPC command: delete a named workspace layout
#[tauri::command]
pub async fn delete_workspace(app: tauri::AppHandle, name: String) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    store.delete(&name);
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

// ── Multi-Monitor Panel State Persistence ─────────────────────────────

/// A single panel's persisted geometry.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PanelState {
    pub panel_id: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub monitor: Option<String>,
}

/// IPC command: save a single panel's position/size.
/// Called when a panel is moved or resized.
#[tauri::command]
pub async fn save_panel_state(
    app: tauri::AppHandle,
    panel: PanelState,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("panels.json").map_err(|e| e.to_string())?;

    // Load existing panels map or create new one
    let mut panels: std::collections::HashMap<String, PanelState> = store
        .get("panels")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    panels.insert(panel.panel_id.clone(), panel);

    store.set(
        "panels",
        serde_json::to_value(&panels).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load all saved panel states for restoration on launch.
/// Returns a list of panel geometries that the frontend can use to
/// recreate detached panel windows at their previous positions.
#[tauri::command]
pub async fn load_panel_state(
    app: tauri::AppHandle,
) -> Result<Vec<PanelState>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("panels.json").map_err(|e| e.to_string())?;

    let panels: std::collections::HashMap<String, PanelState> = store
        .get("panels")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(panels.into_values().collect())
}

/// IPC command: remove a single panel's saved state (e.g. when panel is closed).
#[tauri::command]
pub async fn remove_panel_state(
    app: tauri::AppHandle,
    panel_id: String,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("panels.json").map_err(|e| e.to_string())?;

    let mut panels: std::collections::HashMap<String, PanelState> = store
        .get("panels")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    panels.remove(&panel_id);

    store.set(
        "panels",
        serde_json::to_value(&panels).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}
