pub mod api_proxy;
pub mod auth;
pub mod config;
pub mod local_bots;
pub mod preferences;
pub mod startup;
pub mod updater;
pub mod worker;
mod bot_manager;
mod credential_store;
mod tray;

use serde::Serialize;
use std::process::Child;
use std::sync::Mutex;
use tauri::{Manager, RunEvent, WindowEvent};

use startup::TELEMETRY_URL;

// ── Managed state ─────────────────────────────────────────────────────

/// Holds the remote-worker child process so we can kill it on exit.
pub struct WorkerState {
    pub child: Mutex<Option<Child>>,
}

/// Holds the research/grid worker sidecar child process.
pub struct ResearchWorkerState {
    pub child: Mutex<Option<Child>>,
}

/// Holds the local API sidecar process (standalone mode).
pub struct LocalApiState {
    pub child: Mutex<Option<Child>>,
}

// ── Private types ─────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct BotStatus {
    name: String,
    status: String,
    positions: u32,
    pnl_today: f64,
}

// ── EC2 live data ─────────────────────────────────────────────────────

/// IPC command: check API health — delegates to startup::fetch_health_summary.
#[tauri::command]
async fn check_health() -> Result<startup::HealthSummary, String> {
    startup::fetch_health_summary().await
}

/// IPC command: get bot statuses from telemetry.
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
                    let status =
                        if info.get("stale").and_then(|v| v.as_bool()).unwrap_or(true) {
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

// ── Utility IPC ───────────────────────────────────────────────────────

/// IPC command: send a native desktop notification.
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

/// IPC command: navigate main window to a URL.
#[tauri::command]
async fn navigate_to(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let parsed = url.parse::<tauri::Url>().map_err(|e| e.to_string())?;
        window
            .navigate(parsed)
            .map_err(|e| format!("Navigation error: {}", e))
    } else {
        Err("Main window not found".to_string())
    }
}

/// IPC command: create a detached panel window.
#[tauri::command]
async fn create_panel_window(
    app: tauri::AppHandle,
    panel_id: String,
    panel_title: String,
    width: f64,
    height: f64,
) -> Result<bool, String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    let label =
        format!("panel-{}", panel_id.replace(|c: char| !c.is_alphanumeric(), "-"));
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

// ── Remote worker IPC → see startup.rs ───────────────────────────────
// startup::start_worker, startup::stop_worker, startup::get_worker_status

// ── Broker / bot management IPC → see local_bots.rs ──────────────────

// ── Research worker sidecar IPC → see startup.rs ─────────────────────
// startup::start_research_worker, startup::stop_research_worker,
// startup::research_worker_status

// ── App entry point ───────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // window-state removed — was restoring corrupt position causing black screen
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        // ── Managed state ──
        .manage(WorkerState {
            child: Mutex::new(None),
        })
        .manage(ResearchWorkerState {
            child: Mutex::new(None),
        })
        .manage(bot_manager::BotManagerState::new())
        .manage(LocalApiState {
            child: Mutex::new(None),
        })
        .manage(config::AppConfigState::new(config::AppConfig::default()))
        .manage(worker::GridWorkerState::new())
        // ── IPC handler ──
        .invoke_handler(tauri::generate_handler![
            // EC2 monitoring
            check_health,
            api_proxy::api_proxy,
            get_bot_status,
            send_notification,
            navigate_to,
            // Auth token persistence
            auth::save_auth_token,
            auth::load_auth_token,
            auth::clear_auth_token,
            // Updater
            updater::check_for_update,
            // Multi-window panels
            create_panel_window,
            // Preferences & workspaces
            preferences::save_preference,
            preferences::load_preferences,
            preferences::save_workspace,
            preferences::load_workspace,
            preferences::list_workspaces,
            preferences::delete_workspace,
            // Remote worker
            startup::start_worker,
            startup::stop_worker,
            startup::get_worker_status,
            // Broker management
            local_bots::get_available_brokers,
            local_bots::configure_broker,
            local_bots::delete_broker_credentials,
            local_bots::list_configured_brokers,
            // Local bot management
            local_bots::start_bot,
            local_bots::stop_bot,
            local_bots::get_local_bot_status,
            local_bots::list_local_bots,
            local_bots::get_bot_log,
            // Research worker sidecar
            startup::start_research_worker,
            startup::stop_research_worker,
            startup::research_worker_status,
            // Startup state machine
            startup::startup_check,
            startup::clean_shutdown,
            // Config IPC
            config::get_config,
            // Rust grid worker
            worker::start_grid_worker,
            worker::stop_grid_worker,
            worker::grid_worker_status,
        ])
        .setup(|app| {
            // ── System tray + auto-start workers ──────────────────
            tray::setup_tray(app)?;

            // ── Safety net: show window after 3s even if JS fails ──
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                    }
                });
            }

            // ── Auto-start grid worker (Rust-native, no subprocess) ──
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    // Check config to see if worker is enabled
                    let config_state = app_handle.state::<config::AppConfigState>();
                    let enabled = config_state.0.read().map(|c| c.worker_enabled).unwrap_or(true);
                    if enabled {
                        let state = app_handle.state::<worker::GridWorkerState>();
                        let config = app_handle.state::<config::AppConfigState>();
                        let coordinator = config.0.read().map(|c| c.coordinator_url.clone()).unwrap_or_default();
                        if !coordinator.is_empty() {
                            let status = state.status.clone();
                            let shutdown = state.shutdown.clone();
                            let jh = tokio::spawn(worker::grid_worker::run_worker(
                                coordinator, status, shutdown,
                            ));
                            let mut guard = state.handle.lock().await;
                            *guard = Some(jh);
                            log::info!("Grid worker auto-started from Rust setup");
                        }
                    }
                });
            }

            // ── Navigate to auraalpha.cc ───────────────────────────
            // WebView handles TLS/Cloudflare natively; reqwest is blocked by
            // Cloudflare Bot Fight Mode, so we navigate directly instead.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                let url: tauri::Url = "https://auraalpha.cc".parse().unwrap();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.navigate(url);
                    log::info!("Navigated to auraalpha.cc");
                }
            });

            // ── Auto-start Local API sidecar (standalone mode) ─────
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<LocalApiState>();
                    startup::try_start_local_api(app_handle.clone(), state).await;
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Minimize to tray on window close
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
