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
