/// System tray setup and menu event handling.
///
/// Call `setup_tray(app)` from inside the Tauri `.setup()` closure.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::{
    bot_manager,
    startup::{self, find_research_worker_script, find_sidecar_binary, spawn_worker},
    ResearchWorkerState, WorkerState,
};

// tray.rs intentionally does NOT call crate::#[tauri::command] fns to avoid
// macro namespace collisions. All state management goes through startup:: helpers.

pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show Aura Alpha", true, None::<&str>)?;
    let health = MenuItem::with_id(app, "health", "Check Health", true, None::<&str>)?;
    let worker_item = MenuItem::with_id(app, "worker", "Start Worker", true, None::<&str>)?;
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
                    match startup::fetch_health_summary().await {
                        Ok(h) => {
                            let status = if h.api_up { "Healthy" } else { "Down" };
                            startup::notify(
                                &app,
                                "Aura Alpha Health",
                                &format!(
                                    "API: {} | Bots: {} | Positions: {} | Day P&L: ${:.2}",
                                    status,
                                    h.bots_active,
                                    h.total_positions,
                                    h.total_pnl_today
                                ),
                            )
                            .await;
                        }
                        Err(e) => {
                            startup::notify(
                                &app,
                                "Aura Alpha Health",
                                &format!("Health check failed: {}", e),
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
                    match startup::tray_start_worker(state).await {
                        Ok(ws) if ws.running => {
                            startup::notify(
                                &app,
                                "Compute Worker",
                                &format!("Worker started (PID {})", ws.pid.unwrap_or(0)),
                            )
                            .await;
                        }
                        Ok(_) => {
                            startup::notify(
                                &app,
                                "Compute Worker",
                                "Worker failed to start",
                            )
                            .await;
                        }
                        Err(e) => {
                            startup::notify(&app, "Compute Worker", &format!("Error: {}", e))
                                .await;
                        }
                    }
                });
            }
            "research" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<ResearchWorkerState>();
                    let is_running =
                        startup::tray_research_worker_status(state.clone()).await;
                    if is_running {
                        match startup::tray_stop_research_worker(state).await {
                            Ok(_) => {
                                startup::notify(
                                    &app,
                                    "Research Worker",
                                    "Research worker stopped",
                                )
                                .await;
                            }
                            Err(e) => {
                                startup::notify(
                                    &app,
                                    "Research Worker",
                                    &format!("Stop error: {}", e),
                                )
                                .await;
                            }
                        }
                    } else {
                        match startup::tray_start_research_worker(state).await {
                            Ok(rs) if rs.running => {
                                startup::notify(
                                    &app,
                                    "Research Worker",
                                    &format!(
                                        "Research worker started (PID {})",
                                        rs.pid.unwrap_or(0)
                                    ),
                                )
                                .await;
                            }
                            Ok(_) => {
                                startup::notify(
                                    &app,
                                    "Research Worker",
                                    "Research worker failed to start",
                                )
                                .await;
                            }
                            Err(e) => {
                                startup::notify(
                                    &app,
                                    "Research Worker",
                                    &format!("Error: {}", e),
                                )
                                .await;
                            }
                        }
                    }
                });
            }
            "quit" => {
                // Kill remote worker
                let state = app.state::<WorkerState>();
                if let Ok(mut guard) = state.child.lock() {
                    if let Some(ref mut child) = *guard {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    *guard = None;
                }
                // Kill research worker
                let research_state = app.state::<ResearchWorkerState>();
                if let Ok(mut guard) = research_state.child.lock() {
                    if let Some(ref mut child) = *guard {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    *guard = None;
                }
                // Kill all local bots
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

    // ── Auto-start remote worker if project found ──────────────────
    let worker_state = app.state::<WorkerState>();
    if let Some(project_dir) = startup::find_project_dir() {
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
        log::info!(
            "No project directory found — worker not auto-started. \
             Optimization and backtest jobs will remain queued until a worker connects."
        );
    }

    // ── Auto-start research worker sidecar ─────────────────────────
    let research_state = app.state::<ResearchWorkerState>();
    let has_worker = find_sidecar_binary(if cfg!(target_os = "windows") {
        "aura-grid-worker.exe"
    } else {
        "aura-grid-worker"
    })
    .is_some()
        || find_research_worker_script().is_some();

    if has_worker {
        let coordinator_url = "https://auraalpha.cc";
        match startup::spawn_research_worker(coordinator_url, 2) {
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
                log::error!("Could not auto-start research worker: {}", e);
                log::error!(
                    "=== GRID WORKER TROUBLESHOOTING ===\n\
                     If you are on macOS and downloaded this .app from the internet:\n\
                     1. Open System Settings > Privacy & Security\n\
                     2. Look for a blocked app message and click 'Open Anyway'\n\
                     3. Or run in Terminal: xattr -cr /Applications/Aura\\ Alpha.app\n\
                     4. Then restart the application.\n\
                     The bundled Python binary may have been quarantined by Gatekeeper."
                );
            }
        }
    } else {
        log::info!(
            "Grid worker binary and script both not found — sidecar not auto-started."
        );
    }

    Ok(())
}
