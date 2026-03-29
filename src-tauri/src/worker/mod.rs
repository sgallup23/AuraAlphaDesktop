//! Grid worker module — pure-Rust background compute worker.
//!
//! Replaces the Python sidecar with a native Tokio-based worker that:
//! - Authenticates with the coordinator via token
//! - Dequeues jobs, executes them, reports results
//! - Sends heartbeats every 30 seconds
//! - Shuts down gracefully via `tokio::sync::Notify`
//!
//! IPC commands:
//! - `start_grid_worker`  — spawn the worker loop
//! - `stop_grid_worker`   — signal graceful shutdown
//! - `grid_worker_status` — return current status snapshot

pub mod grid_worker;
pub mod job_executor;

use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;

use crate::config::AppConfigState;

// ─── Managed state ──────────────────────────────────────────────────────────

/// Tauri managed state for the Rust grid worker.
pub struct GridWorkerState {
    /// Handle to the spawned worker task (None when stopped).
    handle: Mutex<Option<JoinHandle<()>>>,
    /// Shared status readable by IPC and the worker loop.
    status: Arc<RwLock<WorkerStatus>>,
    /// Notify channel used to signal graceful shutdown.
    shutdown: Arc<Notify>,
}

impl GridWorkerState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            status: Arc::new(RwLock::new(WorkerStatus::default())),
            shutdown: Arc::new(Notify::new()),
        }
    }
}

// ─── Status struct ──────────────────────────────────────────────────────────

/// Snapshot of the worker's current state, returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    pub running: bool,
    pub worker_id: Option<String>,
    pub coordinator_url: Option<String>,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub uptime_secs: u64,
}

impl Default for WorkerStatus {
    fn default() -> Self {
        Self {
            running: false,
            worker_id: None,
            coordinator_url: None,
            jobs_completed: 0,
            jobs_failed: 0,
            uptime_secs: 0,
        }
    }
}

// ─── IPC commands ───────────────────────────────────────────────────────────

/// Start the grid worker background task.
///
/// Reads `coordinator_url` and `worker_max_parallel` from `AppConfigState`.
/// If the worker is already running, returns its current status without
/// spawning a second instance.
#[tauri::command]
pub async fn start_grid_worker(
    state: tauri::State<'_, GridWorkerState>,
    config_state: tauri::State<'_, AppConfigState>,
) -> Result<WorkerStatus, String> {
    let mut handle_guard = state.handle.lock().await;

    // Already running? Return current status.
    if let Some(ref h) = *handle_guard {
        if !h.is_finished() {
            let s = state.status.read().await.clone();
            return Ok(s);
        }
        // Task finished (crashed or completed) — allow restart.
        *handle_guard = None;
    }

    // Read config
    let cfg = config_state
        .0
        .read()
        .map_err(|e| format!("config lock poisoned: {e}"))?
        .clone();

    if !cfg.worker_enabled {
        return Err("Grid worker is disabled in config".into());
    }

    let coordinator_url = cfg.coordinator_url.clone();

    // Reset status
    {
        let mut s = state.status.write().await;
        *s = WorkerStatus {
            running: true,
            worker_id: None,
            coordinator_url: Some(coordinator_url.clone()),
            jobs_completed: 0,
            jobs_failed: 0,
            uptime_secs: 0,
        };
    }

    // Clone Arcs for the spawned task
    let status = Arc::clone(&state.status);
    let shutdown = Arc::clone(&state.shutdown);

    let handle = tokio::spawn(async move {
        grid_worker::run_worker(coordinator_url, status, shutdown).await;
    });

    *handle_guard = Some(handle);

    let s = state.status.read().await.clone();
    Ok(s)
}

/// Signal the grid worker to shut down gracefully.
///
/// The worker loop checks `Notify` on every iteration and exits cleanly.
/// The JoinHandle is awaited (with a 5s timeout) to confirm shutdown.
#[tauri::command]
pub async fn stop_grid_worker(
    state: tauri::State<'_, GridWorkerState>,
) -> Result<WorkerStatus, String> {
    // Signal shutdown
    state.shutdown.notify_one();

    // Wait for the task to finish (up to 5 seconds)
    let mut handle_guard = state.handle.lock().await;
    if let Some(h) = handle_guard.take() {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), h).await;
    }

    // Update status
    {
        let mut s = state.status.write().await;
        s.running = false;
    }

    let s = state.status.read().await.clone();
    Ok(s)
}

/// Return a snapshot of the grid worker's current status.
#[tauri::command]
pub async fn grid_worker_status(
    state: tauri::State<'_, GridWorkerState>,
) -> Result<WorkerStatus, String> {
    // Check if the task handle is still alive
    let handle_guard = state.handle.lock().await;
    let task_alive = match &*handle_guard {
        Some(h) => !h.is_finished(),
        None => false,
    };
    drop(handle_guard);

    // If the task died unexpectedly, update status
    if !task_alive {
        let mut s = state.status.write().await;
        s.running = false;
    }

    let s = state.status.read().await.clone();
    Ok(s)
}
