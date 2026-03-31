/// Background tasks that run for the lifetime of the application.
///
/// - **Tray P&L tooltip**: polls the health endpoint every 60 s and updates
///   the system-tray tooltip with the current day P&L.
/// - **Signal notifications**: polls `/api/realtime/signals` every 60 s and
///   fires a native notification when new signals appear (only while the
///   main window is hidden / minimised to tray).

use std::sync::atomic::{AtomicUsize, Ordering};

use tauri::Manager;

use crate::startup;

/// Shared state: the last-known signal count so we can detect new arrivals.
static LAST_SIGNAL_COUNT: AtomicUsize = AtomicUsize::new(0);

// ── Tray P&L tooltip updater ─────────────────────────────────────────

/// Spawns an async loop that updates the tray tooltip with live P&L every
/// 60 seconds.  Call once from `tray::setup_tray`.
pub fn spawn_tray_pnl_updater(app: &tauri::App) {
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            update_tray_tooltip(&handle).await;
        }
    });
}

/// Fetch health summary and set the tray tooltip accordingly.
async fn update_tray_tooltip(app: &tauri::AppHandle) {
    let tooltip = match startup::fetch_health_summary().await {
        Ok(h) => {
            let sign = if h.total_pnl_today >= 0.0 { "+" } else { "" };
            format!(
                "Aura Alpha | P&L: {}${:.2} | {} bots | {} pos",
                sign, h.total_pnl_today, h.bots_active, h.total_positions
            )
        }
        Err(_) => "Aura Alpha | Offline".to_string(),
    };

    // TrayIcon is retrieved by the default tray id
    if let Some(tray) = app.tray_by_id("main") {
        if let Err(e) = tray.set_tooltip(Some(&tooltip)) {
            log::warn!("Failed to update tray tooltip: {}", e);
        }
    } else {
        // Fallback: try to find any tray icon
        log::debug!("No tray icon with id 'main' found for tooltip update");
    }
}

// ── Signal notification monitor ──────────────────────────────────────

/// Spawns an async loop that polls for new real-time signals every 60 s.
/// Fires a native notification only when the main window is hidden.
/// Call once during app setup.
pub fn spawn_signal_monitor(app: &tauri::App) {
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        // Wait 30 s before first poll to let the app finish booting
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        let client = reqwest::Client::new();
        loop {
            interval.tick().await;
            poll_signals(&handle, &client).await;
        }
    });
}

/// The signals URL (local-first, same pattern as health).
fn signals_url() -> String {
    format!("{}/api/realtime/signals", crate::config::DEFAULT_LOCAL_API)
}

/// Poll signals endpoint and notify on new arrivals.
async fn poll_signals(app: &tauri::AppHandle, client: &reqwest::Client) {
    // Only notify when the main window is hidden (user is in tray mode)
    let window_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(true);

    if window_visible {
        // Window is visible -- user can see signals in-app, skip notification.
        // But still update the count so we don't spam when they minimise.
        if let Some(count) = fetch_signal_count(client).await {
            LAST_SIGNAL_COUNT.store(count, Ordering::Relaxed);
        }
        return;
    }

    let current_count = match fetch_signal_count(client).await {
        Some(c) => c,
        None => return, // API unreachable -- skip silently
    };

    let previous = LAST_SIGNAL_COUNT.swap(current_count, Ordering::Relaxed);

    if current_count > previous && previous > 0 {
        let new_signals = current_count - previous;
        let body = if new_signals == 1 {
            "1 new trading signal detected".to_string()
        } else {
            format!("{} new trading signals detected", new_signals)
        };
        startup::notify(app, "Aura Alpha Signals", &body).await;
    }
}

/// Fetch the current signal count from `/api/realtime/signals`.
/// Returns `None` on any error (network, parse, etc).
async fn fetch_signal_count(client: &reqwest::Client) -> Option<usize> {
    let resp = client
        .get(&signals_url())
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;

    // The endpoint may return either:
    //   { "signals": [...] }       -- array of signal objects
    //   { "count": N, ... }        -- just a count
    //   [ ... ]                    -- bare array
    if let Some(arr) = data.get("signals").and_then(|v| v.as_array()) {
        Some(arr.len())
    } else if let Some(n) = data.get("count").and_then(|v| v.as_u64()) {
        Some(n as usize)
    } else if let Some(arr) = data.as_array() {
        Some(arr.len())
    } else {
        None
    }
}
