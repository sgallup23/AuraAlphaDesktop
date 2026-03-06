use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

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

/// IPC command: check API health
#[tauri::command]
async fn check_health() -> Result<HealthSummary, String> {
    let client = reqwest::Client::new();
    match client
        .get("https://auraalpha.cc/api/system/health")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => Ok(HealthSummary {
            api_up: true,
            bots_active: 3,
            total_positions: 0,
            total_pnl_today: 0.0,
        }),
        Ok(resp) => Err(format!("API returned status {}", resp.status())),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

/// IPC command: get bot statuses from telemetry
#[tauri::command]
async fn get_bot_status() -> Result<Vec<BotStatus>, String> {
    let client = reqwest::Client::new();
    match client
        .get("https://auraalpha.cc/api/telemetry/latest")
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_health,
            get_bot_status,
            send_notification,
            navigate_to,
        ])
        .setup(|app| {
            // ── System tray ──────────────────────────────────────
            let show = MenuItem::with_id(app, "show", "Show Aura Alpha", true, None::<&str>)?;
            let health = MenuItem::with_id(app, "health", "Check Health", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &health, &quit])?;

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
                                        format!("API: {} | Bots: {}", status, h.bots_active),
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
                    "quit" => {
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

            // ── Navigate to auraalpha.cc on startup ──────────────
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Brief delay to let the splash/loading screen render
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                let client = reqwest::Client::new();
                let reachable = client
                    .get("https://auraalpha.cc/api/system/health")
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);

                if reachable {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let url: tauri::Url = "https://auraalpha.cc".parse().unwrap();
                        let _ = window.navigate(url);
                    }
                }
                // If not reachable, the local index.html stays visible with retry button
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
