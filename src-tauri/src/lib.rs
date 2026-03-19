mod bot_manager;
mod credential_store;

use serde::Serialize;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, RunEvent, WindowEvent,
};

const HEALTH_URL: &str = "https://auraalpha.cc/api/system/health";
const TELEMETRY_URL: &str = "https://auraalpha.cc/api/telemetry/latest";
const REMOTE_API_URL: &str = "https://auraalpha.cc/api/remote";

/// Managed state: holds the worker child process so we can kill it on exit
struct WorkerState {
    child: Mutex<Option<Child>>,
}

/// Managed state: holds the research worker sidecar child process
struct ResearchWorkerState {
    child: Mutex<Option<Child>>,
}

/// Research worker status info returned to the frontend via IPC
#[derive(Clone, Serialize)]
struct ResearchWorkerStatus {
    running: bool,
    pid: Option<u32>,
    coordinator_url: Option<String>,
}

/// Bot status info returned to the frontend via IPC
#[derive(Clone, Serialize)]
struct BotStatus {
    name: String,
    status: String, // "running", "stopped", "error"
    positions: u32,
    pnl_today: f64,
}

/// System health summary
#[derive(Clone, Serialize)]
struct HealthSummary {
    api_up: bool,
    bots_active: u32,
    total_positions: u32,
    total_pnl_today: f64,
}

/// Worker status info
#[derive(Clone, Serialize)]
struct WorkerStatus {
    running: bool,
    pid: Option<u32>,
    project_path: Option<String>,
}

// ── EC2 live data helpers ─────────────────────────────────────────────

/// IPC command: check API health — now parses real data from EC2
#[tauri::command]
async fn check_health() -> Result<HealthSummary, String> {
    let client = reqwest::Client::new();
    match client
        .get(HEALTH_URL)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Parse error: {}", e))?;

            // Parse real bot data from health response
            let bots = data.get("bots").and_then(|b| b.as_object());
            let mut bots_active: u32 = 0;
            let mut total_positions: u32 = 0;
            let mut total_pnl: f64 = 0.0;

            if let Some(bots_obj) = bots {
                for (_key, info) in bots_obj {
                    // Skip non-bot keys like "gateway_connected", "accounts", etc.
                    if !info.is_object() || info.get("bot").is_none() {
                        continue;
                    }
                    if info.get("status").and_then(|s| s.as_str()) == Some("OK") {
                        bots_active += 1;
                    }
                    // Count positions from payload
                    if let Some(payload) = info.get("payload") {
                        if let Some(positions) = payload.get("positions").and_then(|p| p.as_array())
                        {
                            total_positions += positions.len() as u32;
                        }
                        if let Some(equity) = payload.get("equity") {
                            total_pnl +=
                                equity.get("day_pnl").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        }
                    }
                }
            }

            Ok(HealthSummary {
                api_up: true,
                bots_active,
                total_positions,
                total_pnl_today: total_pnl,
            })
        }
        Ok(resp) => Err(format!("API returned status {}", resp.status())),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

/// IPC command: generic API proxy — bypasses browser CORS by using reqwest (Rust-side HTTP)
/// The frontend calls this instead of fetch() to avoid Cloudflare CORS issues.
#[tauri::command]
async fn api_proxy(
    method: String,
    path: String,
    body: Option<String>,
    auth_token: Option<String>,
) -> Result<String, String> {
    let url = if path.starts_with("http") {
        path
    } else {
        format!(
            "{}{}{}",
            REMOTE_API_URL.trim_end_matches("/api/remote"),
            if path.starts_with('/') { "" } else { "/" },
            path
        )
    };

    let client = reqwest::Client::new();
    let mut req = match method.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => client.get(&url),
    };

    req = req.timeout(std::time::Duration::from_secs(30));
    req = req.header("Content-Type", "application/json");

    if let Some(token) = auth_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    if let Some(b) = body {
        req = req.body(b);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if status >= 200 && status < 300 {
                Ok(text)
            } else {
                Err(format!("API {}: {}", status, text))
            }
        }
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

/// IPC command: get bot statuses from telemetry
#[tauri::command]
async fn get_bot_status() -> Result<Vec<BotStatus>, String> {
    let client = reqwest::Client::new();
    match client
        .get(TELEMETRY_URL)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Parse error: {}", e))?;

            let mut bots = Vec::new();
            if let Some(obj) = data.as_object() {
                for (name, info) in obj {
                    let status = if info.get("stale").and_then(|v| v.as_bool()).unwrap_or(true) {
                        "stopped"
                    } else {
                        "running"
                    };
                    let positions = info
                        .get("positions")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len() as u32)
                        .unwrap_or(0);
                    let pnl = info
                        .get("pnl_today")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    bots.push(BotStatus {
                        name: name.clone(),
                        status: status.to_string(),
                        positions,
                        pnl_today: pnl,
                    });
                }
            }
            Ok(bots)
        }
        Ok(resp) => Err(format!("API returned status {}", resp.status())),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

// ── Worker management ─────────────────────────────────────────────────

/// Find the prodesk project directory (checks common paths)
fn find_project_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("TRADING_DESK").join("prodesk"),
        home.join("prodesk"),
        home.join("AuraAlpha").join("prodesk"),
    ];
    candidates.into_iter().find(|p| {
        p.join("ops").join("remote_worker.py").exists()
    })
}

/// Read the REMOTE_WORKER_TOKEN from the project's .env file
fn read_worker_token(project_dir: &std::path::Path) -> Option<String> {
    let env_file = project_dir.join(".env");
    if !env_file.exists() {
        return None;
    }
    let content = std::fs::read_to_string(env_file).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("REMOTE_WORKER_TOKEN=") {
            let token = val.trim().trim_matches('"').trim_matches('\'');
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Find a Python interpreter
fn find_python(project_dir: &std::path::Path) -> String {
    // Prefer project venv
    let venv_python = project_dir.join(".venv").join("bin").join("python");
    if venv_python.exists() {
        return venv_python.to_string_lossy().to_string();
    }
    let venv_python3 = project_dir.join(".venv").join("bin").join("python3");
    if venv_python3.exists() {
        return venv_python3.to_string_lossy().to_string();
    }
    // Fallback to system
    "python3".to_string()
}

/// Spawn the remote worker process
fn spawn_worker(project_dir: &std::path::Path) -> Result<Child, String> {
    let worker_script = project_dir.join("ops").join("remote_worker.py");
    if !worker_script.exists() {
        return Err("remote_worker.py not found".to_string());
    }

    let token = read_worker_token(project_dir)
        .ok_or_else(|| "REMOTE_WORKER_TOKEN not found in .env".to_string())?;

    let python = find_python(project_dir);
    let log_dir = project_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::File::create(log_dir.join("remote_worker.log"))
        .map_err(|e| format!("Cannot create log file: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Cannot clone log file: {}", e))?;

    Command::new(&python)
        .arg(worker_script.to_string_lossy().as_ref())
        .current_dir(project_dir)
        .env("REMOTE_API_URL", REMOTE_API_URL)
        .env("REMOTE_WORKER_TOKEN", &token)
        .env("WORKER_ID", "desktop")
        .env("POLL_INTERVAL", "30")
        .stdout(log_file)
        .stderr(log_err)
        .spawn()
        .map_err(|e| format!("Failed to spawn worker: {}", e))
}

/// IPC command: start the remote worker
#[tauri::command]
async fn start_worker(state: tauri::State<'_, WorkerState>) -> Result<WorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    // Check if already running
    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(WorkerStatus {
                    running: true,
                    pid: Some(child.id()),
                    project_path: find_project_dir().map(|p| p.to_string_lossy().to_string()),
                });
            }
            _ => {
                // Process exited, clear it
                *guard = None;
            }
        }
    }

    let project_dir = find_project_dir().ok_or("Project directory not found")?;
    let child = spawn_worker(&project_dir)?;
    let pid = child.id();
    *guard = Some(child);

    Ok(WorkerStatus {
        running: true,
        pid: Some(pid),
        project_path: Some(project_dir.to_string_lossy().to_string()),
    })
}

/// IPC command: stop the remote worker
#[tauri::command]
async fn stop_worker(state: tauri::State<'_, WorkerState>) -> Result<WorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut child) = *guard {
        let _ = child.kill();
        let _ = child.wait();
    }
    *guard = None;

    Ok(WorkerStatus {
        running: false,
        pid: None,
        project_path: find_project_dir().map(|p| p.to_string_lossy().to_string()),
    })
}

/// IPC command: check worker status
#[tauri::command]
async fn get_worker_status(state: tauri::State<'_, WorkerState>) -> Result<WorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    let project_path = find_project_dir().map(|p| p.to_string_lossy().to_string());

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => Ok(WorkerStatus {
                running: true,
                pid: Some(child.id()),
                project_path,
            }),
            _ => {
                *guard = None;
                Ok(WorkerStatus {
                    running: false,
                    pid: None,
                    project_path,
                })
            }
        }
    } else {
        Ok(WorkerStatus {
            running: false,
            pid: None,
            project_path,
        })
    }
}

/// IPC command: send a native desktop notification
#[tauri::command]
async fn send_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| format!("Notification error: {}", e))
}

/// IPC command: navigate main window to a URL
#[tauri::command]
async fn navigate_to(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let parsed = url
            .parse::<tauri::Url>()
            .map_err(|e| e.to_string())?;
        window
            .navigate(parsed)
            .map_err(|e| format!("Navigation error: {}", e))
    } else {
        Err("Main window not found".to_string())
    }
}

// ── Auth Token & Persistence IPC Commands ────────────────────────────

/// IPC command: save auth tokens to persistent store
#[tauri::command]
async fn save_auth_token(
    app: tauri::AppHandle,
    access_token: String,
    refresh_token: String,
    user_json: String,
) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("auth.json").map_err(|e| e.to_string())?;
    store.set("access_token", serde_json::Value::String(access_token));
    store.set("refresh_token", serde_json::Value::String(refresh_token));
    store.set("user", serde_json::Value::String(user_json));
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load auth tokens from persistent store
#[tauri::command]
async fn load_auth_token(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri_plugin_store::StoreExt;
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
async fn clear_auth_token(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("auth.json").map_err(|e| e.to_string())?;
    store.delete("access_token");
    store.delete("refresh_token");
    store.delete("user");
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: create a detached panel window
#[tauri::command]
async fn create_panel_window(
    app: tauri::AppHandle,
    panel_id: String,
    panel_title: String,
    width: f64,
    height: f64,
) -> Result<bool, String> {
    use tauri::WebviewWindowBuilder;
    use tauri::WebviewUrl;
    let label = format!("panel-{}", panel_id.replace(|c: char| !c.is_alphanumeric(), "-"));
    let url_str = format!("index.html?panel={}", panel_id);
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url_str.into()))
        .title(&panel_title)
        .inner_size(width, height)
        .min_inner_size(640.0, 480.0)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: save a single user preference
#[tauri::command]
async fn save_preference(app: tauri::AppHandle, key: String, value: serde_json::Value) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("preferences.json").map_err(|e| e.to_string())?;
    store.set(&key, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load all user preferences
#[tauri::command]
async fn load_preferences(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
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
async fn save_workspace(app: tauri::AppHandle, name: String, layout_json: String) -> Result<bool, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    store.set(&name, serde_json::Value::String(layout_json));
    store.save().map_err(|e| e.to_string())?;
    Ok(true)
}

/// IPC command: load a named workspace layout
#[tauri::command]
async fn load_workspace(app: tauri::AppHandle, name: String) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    match store.get(&name) {
        Some(serde_json::Value::String(s)) => Ok(s),
        _ => Err("Workspace not found".into()),
    }
}

/// IPC command: list all saved workspace names
#[tauri::command]
async fn list_workspaces(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("workspaces.json").map_err(|e| e.to_string())?;
    Ok(store.keys().into_iter().collect())
}

// ── Bot Management IPC Commands ──────────────────────────────────────

/// IPC command: get list of all supported brokers with credential field definitions
#[tauri::command]
async fn get_available_brokers() -> Result<Vec<credential_store::BrokerInfo>, String> {
    Ok(credential_store::get_broker_definitions())
}

/// IPC command: save broker credentials to encrypted store
#[tauri::command]
async fn configure_broker(
    broker: String,
    credentials: HashMap<String, String>,
) -> Result<bool, String> {
    credential_store::save_credentials(&broker, &credentials)?;
    log::info!("Saved credentials for broker: {}", broker);
    Ok(true)
}

/// IPC command: delete stored credentials for a broker
#[tauri::command]
async fn delete_broker_credentials(broker: String) -> Result<bool, String> {
    credential_store::delete_credentials(&broker)?;
    log::info!("Deleted credentials for broker: {}", broker);
    Ok(true)
}

/// IPC command: list brokers that have stored credentials
#[tauri::command]
async fn list_configured_brokers() -> Result<Vec<String>, String> {
    Ok(credential_store::list_configured_brokers())
}

/// IPC command: start a trading bot with the given configuration
#[tauri::command]
async fn start_bot(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    config: bot_manager::BotConfig,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    // Check if bot with this name is already running
    if let Some(existing) = guard.get_mut(&config.bot_name) {
        if bot_manager::check_bot_alive(&mut existing.child) {
            return Ok(bot_manager::BotInfo {
                bot_name: config.bot_name.clone(),
                broker: existing.config.broker.clone(),
                pid: Some(existing.child.id()),
                running: true,
                config_path: Some(existing.config_path.to_string_lossy().to_string()),
                log_path: Some(existing.log_path.to_string_lossy().to_string()),
                started_at: Some(existing.started_at),
            });
        }
        // Process died, remove it
        guard.remove(&config.bot_name);
    }

    let project_dir =
        bot_manager::find_project_dir().ok_or("Project directory not found")?;

    // Load broker credentials from store
    let creds = credential_store::load_credentials(&config.broker)
        .unwrap_or_default();

    // Write bot config to file
    let creds_json = serde_json::to_value(&creds).unwrap_or_default();
    let config_path =
        bot_manager::write_bot_config(&project_dir, &config, &creds_json)?;

    // Build env vars for broker
    let broker_env: HashMap<String, String> = creds;

    // Spawn the bot process
    let (child, log_path) =
        bot_manager::spawn_bot(&project_dir, &config, &config_path, &broker_env)?;

    let pid = child.id();
    let started_at = bot_manager::now_epoch();

    let info = bot_manager::BotInfo {
        bot_name: config.bot_name.clone(),
        broker: config.broker.clone(),
        pid: Some(pid),
        running: true,
        config_path: Some(config_path.to_string_lossy().to_string()),
        log_path: Some(log_path.to_string_lossy().to_string()),
        started_at: Some(started_at),
    };

    guard.insert(
        config.bot_name.clone(),
        bot_manager::BotProcess {
            child,
            config,
            config_path,
            log_path,
            started_at,
        },
    );

    log::info!("Started bot '{}' (PID {})", info.bot_name, pid);
    Ok(info)
}

/// IPC command: stop a running bot by name
#[tauri::command]
async fn stop_bot(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(mut process) = guard.remove(&bot_name) {
        bot_manager::stop_bot_process(&mut process.child)?;
        log::info!("Stopped bot '{}'", bot_name);
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: process.config.broker,
            pid: None,
            running: false,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        })
    } else {
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: String::new(),
            pid: None,
            running: false,
            config_path: None,
            log_path: None,
            started_at: None,
        })
    }
}

/// IPC command: get status of a specific bot
#[tauri::command]
async fn get_local_bot_status(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(process) = guard.get_mut(&bot_name) {
        let running = bot_manager::check_bot_alive(&mut process.child);
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: process.config.broker.clone(),
            pid: if running { Some(process.child.id()) } else { None },
            running,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        })
    } else {
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: String::new(),
            pid: None,
            running: false,
            config_path: None,
            log_path: None,
            started_at: None,
        })
    }
}

/// IPC command: list all local bots and their statuses
#[tauri::command]
async fn list_local_bots(
    state: tauri::State<'_, bot_manager::BotManagerState>,
) -> Result<Vec<bot_manager::BotInfo>, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    for (name, process) in guard.iter_mut() {
        let running = bot_manager::check_bot_alive(&mut process.child);
        result.push(bot_manager::BotInfo {
            bot_name: name.clone(),
            broker: process.config.broker.clone(),
            pid: if running { Some(process.child.id()) } else { None },
            running,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        });
    }

    Ok(result)
}

/// IPC command: read recent lines from a bot's log file
#[tauri::command]
async fn get_bot_log(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
    tail_lines: Option<usize>,
) -> Result<String, String> {
    let guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(process) = guard.get(&bot_name) {
        let content = std::fs::read_to_string(&process.log_path)
            .map_err(|e| format!("Cannot read log: {}", e))?;

        let lines: Vec<&str> = content.lines().collect();
        let n = tail_lines.unwrap_or(100);
        let start = if lines.len() > n { lines.len() - n } else { 0 };
        Ok(lines[start..].join("\n"))
    } else {
        Err(format!("Bot '{}' not found", bot_name))
    }
}

// ── Research Worker Sidecar ───────────────────────────────────────────

/// Find the sidecar script path (checks bundled resource dirs per-platform, then fallbacks)
fn find_research_worker_script() -> Option<std::path::PathBuf> {
    let target = std::path::Path::new("sidecar").join("research_worker.py");

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Windows / Linux AppImage: resources are next to the binary
            let candidate = exe_dir.join(&target);
            if candidate.exists() {
                return Some(candidate);
            }

            // macOS .app bundle: exe is at Contents/MacOS/, resources at Contents/Resources/
            let macos_resources = exe_dir.join("../Resources").join(&target);
            if macos_resources.exists() {
                return Some(macos_resources);
            }

            // Linux .deb: exe at /usr/bin/, resources at /usr/share/<identifier>/
            let deb_resources = std::path::Path::new("/usr/share/cc.auraalpha.desktop").join(&target);
            if deb_resources.exists() {
                return Some(deb_resources);
            }

            // Development: walk up to project root
            let dev_candidate = exe_dir
                .ancestors()
                .find(|p| p.join(&target).exists())
                .map(|p| p.join(&target));
            if let Some(path) = dev_candidate {
                return Some(path);
            }
        }
    }
    // Fallback: check home dir (dev clones)
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join("AuraAlphaDesktop").join(&target);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Spawn the research worker sidecar process
fn spawn_research_worker(coordinator_url: &str, max_parallel: u32) -> Result<Child, String> {
    let script = find_research_worker_script()
        .ok_or_else(|| "research_worker.py not found".to_string())?;

    // Find python — prefer bundled portable Python, then system
    let python = {
        let mut found = String::from("python3");
        // Check for bundled Python next to the sidecar script
        if let Some(script_dir) = script.parent() {
            // Unix: sidecar/python/bin/python3
            let bundled_unix = script_dir.join("python").join("bin").join("python3");
            // Windows: sidecar/python/python.exe
            let bundled_win = script_dir.join("python").join("python.exe");
            // Also check one level up (macOS Resources/sidecar/python/...)
            let bundled_unix_res = script_dir.parent()
                .map(|p| p.join("sidecar").join("python").join("bin").join("python3"))
                .unwrap_or_default();

            if bundled_unix.exists() {
                found = bundled_unix.to_string_lossy().to_string();
                log::info!("Using bundled Python: {}", found);
            } else if bundled_win.exists() {
                found = bundled_win.to_string_lossy().to_string();
                log::info!("Using bundled Python (Windows): {}", found);
            } else if bundled_unix_res.exists() {
                found = bundled_unix_res.to_string_lossy().to_string();
                log::info!("Using bundled Python (Resources): {}", found);
            }
        }
        // Fallback: prodesk venv or system python
        if found == "python3" {
            if let Some(home) = dirs::home_dir() {
                let prodesk_venv = home.join("TRADING_DESK").join("prodesk")
                    .join(".venv").join("bin").join("python");
                if prodesk_venv.exists() {
                    found = prodesk_venv.to_string_lossy().to_string();
                }
            }
        }
        found
    };

    // Create log directory in user-writable location (not inside .app bundle or AppImage)
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local").join("share"))
        .join("cc.auraalpha.desktop")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::File::create(log_dir.join("research_worker.log"))
        .map_err(|e| format!("Cannot create log file: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Cannot clone log file: {}", e))?;

    Command::new(&python)
        .arg(script.to_string_lossy().as_ref())
        .arg("--coordinator-url")
        .arg(coordinator_url)
        .arg("--max-parallel")
        .arg(max_parallel.to_string())
        .stdout(log_file)
        .stderr(log_err)
        .spawn()
        .map_err(|e| format!("Failed to spawn research worker: {}", e))
}

/// IPC command: start the research worker sidecar
#[tauri::command]
async fn start_research_worker(
    state: tauri::State<'_, ResearchWorkerState>,
    coordinator_url: Option<String>,
    max_parallel: Option<u32>,
) -> Result<ResearchWorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    // Check if already running
    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(ResearchWorkerStatus {
                    running: true,
                    pid: Some(child.id()),
                    coordinator_url: Some(coordinator_url.unwrap_or_default()),
                });
            }
            _ => {
                *guard = None;
            }
        }
    }

    let url = coordinator_url.unwrap_or_else(|| "https://auraalpha.cc".to_string());
    let par = max_parallel.unwrap_or(2);

    let child = spawn_research_worker(&url, par)?;
    let pid = child.id();
    *guard = Some(child);

    log::info!("Research worker started (PID {})", pid);
    Ok(ResearchWorkerStatus {
        running: true,
        pid: Some(pid),
        coordinator_url: Some(url),
    })
}

/// IPC command: stop the research worker sidecar
#[tauri::command]
async fn stop_research_worker(
    state: tauri::State<'_, ResearchWorkerState>,
) -> Result<ResearchWorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut child) = *guard {
        let _ = child.kill();
        let _ = child.wait();
    }
    *guard = None;

    log::info!("Research worker stopped");
    Ok(ResearchWorkerStatus {
        running: false,
        pid: None,
        coordinator_url: None,
    })
}

/// IPC command: check research worker status
#[tauri::command]
async fn research_worker_status(
    state: tauri::State<'_, ResearchWorkerState>,
) -> Result<ResearchWorkerStatus, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => Ok(ResearchWorkerStatus {
                running: true,
                pid: Some(child.id()),
                coordinator_url: None,
            }),
            _ => {
                *guard = None;
                Ok(ResearchWorkerStatus {
                    running: false,
                    pid: None,
                    coordinator_url: None,
                })
            }
        }
    } else {
        Ok(ResearchWorkerStatus {
            running: false,
            pid: None,
            coordinator_url: None,
        })
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // window-state removed — was restoring corrupt position causing black screen
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .manage(WorkerState {
            child: Mutex::new(None),
        })
        .manage(ResearchWorkerState {
            child: Mutex::new(None),
        })
        .manage(bot_manager::BotManagerState::new())
        .invoke_handler(tauri::generate_handler![
            // EC2 monitoring
            check_health,
            api_proxy,
            get_bot_status,
            send_notification,
            navigate_to,
            // Auth token persistence
            save_auth_token,
            load_auth_token,
            clear_auth_token,
            // Multi-window panels
            create_panel_window,
            // Preferences & workspaces
            save_preference,
            load_preferences,
            save_workspace,
            load_workspace,
            list_workspaces,
            // Remote worker
            start_worker,
            stop_worker,
            get_worker_status,
            // Broker management
            get_available_brokers,
            configure_broker,
            delete_broker_credentials,
            list_configured_brokers,
            // Local bot management
            start_bot,
            stop_bot,
            get_local_bot_status,
            list_local_bots,
            get_bot_log,
            // Research worker sidecar
            start_research_worker,
            stop_research_worker,
            research_worker_status,
        ])
        .setup(|app| {
            // ── System tray ──────────────────────────────────────
            let show = MenuItem::with_id(app, "show", "Show Aura Alpha", true, None::<&str>)?;
            let health = MenuItem::with_id(app, "health", "Check Health", true, None::<&str>)?;
            let worker_item =
                MenuItem::with_id(app, "worker", "Start Worker", true, None::<&str>)?;
            let research_item =
                MenuItem::with_id(app, "research", "Research: Start", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &health, &worker_item, &research_item, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Aura Alpha — Trading Desk")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "health" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            match check_health().await {
                                Ok(h) => {
                                    let status = if h.api_up { "Healthy" } else { "Down" };
                                    let _ = send_notification(
                                        app,
                                        "Aura Alpha Health".to_string(),
                                        format!(
                                            "API: {} | Bots: {} | Positions: {} | Day P&L: ${:.2}",
                                            status, h.bots_active, h.total_positions, h.total_pnl_today
                                        ),
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    let _ = send_notification(
                                        app,
                                        "Aura Alpha Health".to_string(),
                                        format!("Health check failed: {}", e),
                                    )
                                    .await;
                                }
                            }
                        });
                    }
                    "worker" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<WorkerState>();
                            match start_worker(state).await {
                                Ok(ws) if ws.running => {
                                    let _ = send_notification(
                                        app,
                                        "Compute Worker".to_string(),
                                        format!("Worker started (PID {})", ws.pid.unwrap_or(0)),
                                    )
                                    .await;
                                }
                                Ok(_) => {
                                    let _ = send_notification(
                                        app,
                                        "Compute Worker".to_string(),
                                        "Worker failed to start".to_string(),
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    let _ = send_notification(
                                        app,
                                        "Compute Worker".to_string(),
                                        format!("Error: {}", e),
                                    )
                                    .await;
                                }
                            }
                        });
                    }
                    "research" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<ResearchWorkerState>();
                            // Toggle: check if running via status command
                            let is_running = match research_worker_status(state.clone()).await {
                                Ok(s) => s.running,
                                Err(_) => false,
                            };
                            if is_running {
                                // Stop it
                                match stop_research_worker(state).await {
                                    Ok(_) => {
                                        let _ = send_notification(
                                            app,
                                            "Research Worker".to_string(),
                                            "Research worker stopped".to_string(),
                                        ).await;
                                    }
                                    Err(e) => {
                                        let _ = send_notification(
                                            app,
                                            "Research Worker".to_string(),
                                            format!("Stop error: {}", e),
                                        ).await;
                                    }
                                }
                            } else {
                                // Start it
                                match start_research_worker(state, None, None).await {
                                    Ok(rs) if rs.running => {
                                        let _ = send_notification(
                                            app,
                                            "Research Worker".to_string(),
                                            format!("Research worker started (PID {})", rs.pid.unwrap_or(0)),
                                        ).await;
                                    }
                                    Ok(_) => {
                                        let _ = send_notification(
                                            app,
                                            "Research Worker".to_string(),
                                            "Research worker failed to start".to_string(),
                                        ).await;
                                    }
                                    Err(e) => {
                                        let _ = send_notification(
                                            app,
                                            "Research Worker".to_string(),
                                            format!("Error: {}", e),
                                        ).await;
                                    }
                                }
                            }
                        });
                    }
                    "quit" => {
                        // Kill worker on quit
                        let state = app.state::<WorkerState>();
                        if let Ok(mut guard) = state.child.lock() {
                            if let Some(ref mut child) = *guard {
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                            *guard = None;
                        }
                        // Kill research worker on quit
                        let research_state = app.state::<ResearchWorkerState>();
                        if let Ok(mut guard) = research_state.child.lock() {
                            if let Some(ref mut child) = *guard {
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                            *guard = None;
                        }
                        // Kill all local bots on quit
                        let bot_state = app.state::<bot_manager::BotManagerState>();
                        if let Ok(mut guard) = bot_state.bots.lock() {
                            for (name, process) in guard.iter_mut() {
                                log::info!("Stopping bot '{}' on quit", name);
                                let _ = bot_manager::stop_bot_process(&mut process.child);
                            }
                            guard.clear();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── Auto-start remote worker if project found ─────────
            let worker_state = app.state::<WorkerState>();
            if let Some(project_dir) = find_project_dir() {
                match spawn_worker(&project_dir) {
                    Ok(child) => {
                        log::info!(
                            "Auto-started remote worker (PID {}) from {}",
                            child.id(),
                            project_dir.display()
                        );
                        if let Ok(mut guard) = worker_state.child.lock() {
                            *guard = Some(child);
                        }
                    }
                    Err(e) => {
                        log::warn!("Could not auto-start worker: {}", e);
                    }
                }
            } else {
                log::info!("No project directory found — worker not auto-started. \
                    Optimization and backtest jobs will remain queued until a worker connects.");
            }

            // ── Auto-start research worker sidecar ─────────────────────
            let research_state = app.state::<ResearchWorkerState>();
            if find_research_worker_script().is_some() {
                let coordinator_url = "https://auraalpha.cc";
                match spawn_research_worker(coordinator_url, 2) {
                    Ok(child) => {
                        log::info!(
                            "Auto-started research worker sidecar (PID {})",
                            child.id()
                        );
                        if let Ok(mut guard) = research_state.child.lock() {
                            *guard = Some(child);
                        }
                    }
                    Err(e) => {
                        log::warn!("Could not auto-start research worker: {}", e);
                    }
                }
            } else {
                log::info!("Research worker script not found — sidecar not auto-started.");
            }

            // ── Safety net: show window after 3s even if JS fails ────
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                    }
                });
            }

            // ── Navigate to auraalpha.cc with retry loop ────
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                let client = reqwest::Client::new();
                let url: tauri::Url = "https://auraalpha.cc".parse().unwrap();

                // Try up to 30 times (covers ~2.5 minutes of downtime)
                for attempt in 0..30 {
                    let reachable = client
                        .get(HEALTH_URL)
                        .timeout(std::time::Duration::from_secs(5))
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false);

                    if reachable {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.navigate(url);
                        }
                        log::info!("Navigated to auraalpha.cc (attempt {})", attempt + 1);
                        return;
                    }

                    // Wait 5s before retry
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                log::warn!("Could not reach auraalpha.cc after 30 attempts — staying on local fallback");
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Minimize to tray instead of closing
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            RunEvent::ExitRequested { api, .. } => {
                // Keep running in tray
                api.prevent_exit();
            }
            _ => {}
        });
}
