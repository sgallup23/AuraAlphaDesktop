use tauri_plugin_store::StoreExt;

/// IPC command: save auth tokens to persistent store
#[tauri::command]
pub async fn save_auth_token(
    app: tauri::AppHandle,
    access_token: String,
    refresh_token: String,
    user_json: String,
) -> Result<bool, String> {
    let store = app.store("auth.json").map_err(|e| e.to_string())?;
    store.set("access_token", serde_json::Value::String(access_token));
    store.set("refresh_token", serde_json::Value::String(refresh_token));
    store.set("user", serde_json::Value::String(user_json));
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load auth tokens from persistent store
#[tauri::command]
pub async fn load_auth_token(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let store = app.store("auth.json").map_err(|e| e.to_string())?;
    let access = store.get("access_token").unwrap_or(serde_json::Value::Null);
    let refresh = store.get("refresh_token").unwrap_or(serde_json::Value::Null);
    let user = store.get("user").unwrap_or(serde_json::Value::Null);
    Ok(serde_json::json!({
        "access_token": access,
        "refresh_token": refresh,
        "user": user
    }))
}

/// IPC command: clear auth tokens from persistent store
#[tauri::command]
pub async fn clear_auth_token(app: tauri::AppHandle) -> Result<bool, String> {
    let store = app.store("auth.json").map_err(|e| e.to_string())?;
    store.delete("access_token");
    store.delete("refresh_token");
    store.delete("user");
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}
