//! Core grid worker loop — token resolution, registration, heartbeat, job processing.
//!
//! This module contains the long-running async function `run_worker` that is
//! spawned by the `start_grid_worker` IPC command in `mod.rs`.

use super::job_executor;
use super::WorkerStatus;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Notify, RwLock};

// ─── Token resolution ───────────────────────────────────────────────────────

/// Token file stored at `~/.aura-worker/grid_token.json`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct TokenFile {
    token: String,
    worker_id: Option<String>,
}

/// Resolve a worker token using the following priority:
/// 1. `AURA_WORKER_TOKEN` environment variable
/// 2. `~/.aura-worker/grid_token.json` file
/// 3. Auto-provision via POST to `{coordinator}/api/cluster/contributor/auto-provision`
async fn resolve_token(coordinator_url: &str) -> Result<(String, Option<String>), String> {
    // 1. Environment variable
    if let Ok(token) = std::env::var("AURA_WORKER_TOKEN") {
        if !token.is_empty() {
            info!("grid_worker: using token from AURA_WORKER_TOKEN env var");
            return Ok((token, None));
        }
    }

    // 2. Token file on disk
    let token_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".aura-worker")
        .join("grid_token.json");

    if token_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&token_path) {
            if let Ok(tf) = serde_json::from_str::<TokenFile>(&contents) {
                info!(
                    "grid_worker: loaded token from {}",
                    token_path.display()
                );
                return Ok((tf.token, tf.worker_id));
            }
        }
    }

    // 3. Auto-provision
    info!("grid_worker: no token found — auto-provisioning with coordinator");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client build error: {e}"))?;

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let body = serde_json::json!({
        "hostname": hostname,
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/auto-provision"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("provision request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "provision returned status {}",
            resp.status()
        ));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("provision parse error: {e}"))?;

    let token = data["token"]
        .as_str()
        .ok_or("provision response missing 'token' field")?
        .to_string();
    let worker_id = data["worker_id"].as_str().map(|s| s.to_string());

    // Save to disk for next launch
    let tf = TokenFile {
        token: token.clone(),
        worker_id: worker_id.clone(),
    };
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&tf) {
        if let Err(e) = std::fs::write(&token_path, json) {
            warn!("grid_worker: could not save token to {}: {e}", token_path.display());
        } else {
            info!("grid_worker: saved token to {}", token_path.display());
        }
    }

    Ok((token, worker_id))
}

// ─── HTTP helpers ───────────────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("AuraAlpha-Desktop/5.0")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))
}

/// Register this worker with the coordinator.
async fn register(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
) -> Result<String, String> {
    let machine_hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let memory_gb = {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        (sys.total_memory() as f64 / 1_073_741_824.0).round() as u32
    };

    let body = serde_json::json!({
        "worker_id": worker_id,
        "hostname": machine_hostname,
        "os": format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": env!("CARGO_PKG_VERSION"),
        "cpu_cores": cpu_cores,
        "memory_gb": memory_gb,
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/register"))
        .header("X-Worker-Token", token)
        .header("X-Worker-Id", worker_id)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("register failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("register returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("register parse error: {e}"))?;

    // The server may assign a new worker_id
    let assigned_id = data["worker_id"]
        .as_str()
        .unwrap_or(worker_id)
        .to_string();

    info!("grid_worker: registered as {assigned_id}");
    Ok(assigned_id)
}

/// Send a heartbeat to the coordinator.
async fn heartbeat(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    jobs_completed: u64,
    jobs_failed: u64,
) -> Result<(), String> {
    let machine_hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let body = serde_json::json!({
        "worker_id": worker_id,
        "hostname": machine_hostname,
        "jobs_completed": jobs_completed,
        "jobs_failed": jobs_failed,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/heartbeat"))
        .header("X-Worker-Token", token)
        .header("X-Worker-Id", worker_id)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("heartbeat failed: {e}"))?;

    if !resp.status().is_success() {
        warn!("grid_worker: heartbeat returned {}", resp.status());
    }

    Ok(())
}

/// Dequeue a job from the coordinator. Returns `None` if no jobs available.
async fn dequeue_job(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
) -> Result<Option<serde_json::Value>, String> {
    // Server expects DequeueRequest body: {worker_id, count, job_types}
    let body = serde_json::json!({
        "worker_id": worker_id,
        "count": 1,
        "job_types": [],
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/dequeue"))
        .header("X-Worker-Token", token)
        .header("X-Worker-Id", worker_id)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("dequeue failed: {e}"))?;

    if resp.status().as_u16() == 204 {
        // No jobs available
        return Ok(None);
    }

    if !resp.status().is_success() {
        return Err(format!("dequeue returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("dequeue parse error: {e}"))?;

    // Server returns {"jobs": [...]}, extract the first job if present
    if let Some(jobs) = data["jobs"].as_array() {
        if let Some(first) = jobs.first() {
            return Ok(Some(first.clone()));
        }
    }

    // Empty jobs array or unexpected shape — no work available
    Ok(None)
}

/// Report job completion to the coordinator.
async fn complete_job(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    job_id: &str,
    result: &serde_json::Value,
) -> Result<(), String> {
    // Server expects CompleteRequest: {job_id, metrics, result, compute_seconds}
    let body = serde_json::json!({
        "job_id": job_id,
        "result": result,
        "metrics": result,
        "compute_seconds": 0,
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/complete"))
        .header("X-Worker-Token", token)
        .header("X-Worker-Id", worker_id)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("complete failed: {e}"))?;

    if !resp.status().is_success() {
        warn!(
            "grid_worker: complete for job {job_id} returned {}",
            resp.status()
        );
    }

    Ok(())
}

/// Report job failure to the coordinator.
async fn fail_job(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    job_id: &str,
    error_msg: &str,
) -> Result<(), String> {
    // Server expects FailRequest: {job_id, error} at the /fail endpoint
    let body = serde_json::json!({
        "job_id": job_id,
        "error": error_msg,
    });

    let resp = client
        .post(format!("{coordinator_url}/api/cluster/contributor/fail"))
        .header("X-Worker-Token", token)
        .header("X-Worker-Id", worker_id)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("fail report failed: {e}"))?;

    if !resp.status().is_success() {
        warn!(
            "grid_worker: fail report for job {job_id} returned {}",
            resp.status()
        );
    }

    Ok(())
}

// ─── Main worker loop ───────────────────────────────────────────────────────

/// Entry point for the grid worker, spawned as a Tokio task.
///
/// Lifecycle:
/// 1. Resolve token (env -> file -> auto-provision)
/// 2. Register with coordinator
/// 3. Spawn heartbeat task (30s interval)
/// 4. Loop: dequeue job -> execute -> report result
/// 5. On `Notify` signal or unrecoverable error, exit cleanly
pub async fn run_worker(
    coordinator_url: String,
    status: Arc<RwLock<WorkerStatus>>,
    shutdown: Arc<Notify>,
) {
    info!("grid_worker: starting (coordinator={coordinator_url})");
    let start_time = Instant::now();

    // Resolve token
    let (token, provisioned_id) = match resolve_token(&coordinator_url).await {
        Ok(t) => t,
        Err(e) => {
            error!("grid_worker: token resolution failed: {e}");
            let mut s = status.write().await;
            s.running = false;
            return;
        }
    };

    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            error!("grid_worker: {e}");
            let mut s = status.write().await;
            s.running = false;
            return;
        }
    };

    // Register
    let initial_id = provisioned_id.unwrap_or_else(|| "desktop-worker".to_string());
    let worker_id = match register(&client, &coordinator_url, &token, &initial_id).await {
        Ok(id) => id,
        Err(e) => {
            warn!("grid_worker: registration failed ({e}), using local ID");
            initial_id
        }
    };

    // Update status with worker_id
    {
        let mut s = status.write().await;
        s.worker_id = Some(worker_id.clone());
    }

    // Spawn heartbeat loop
    let hb_client = client.clone();
    let hb_url = coordinator_url.clone();
    let hb_token = token.clone();
    let hb_worker_id = worker_id.clone();
    let hb_status = Arc::clone(&status);
    let hb_shutdown = Arc::clone(&shutdown);

    let _heartbeat_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let s = hb_status.read().await;
                    let _ = heartbeat(
                        &hb_client,
                        &hb_url,
                        &hb_token,
                        &hb_worker_id,
                        s.jobs_completed,
                        s.jobs_failed,
                    )
                    .await;
                }
                _ = hb_shutdown.notified() => {
                    info!("grid_worker: heartbeat loop shutting down");
                    break;
                }
            }
        }
    });

    // Main dequeue/execute loop
    let mut idle_backoff_secs: u64 = 2;
    let max_backoff_secs: u64 = 60;

    loop {
        // Check for shutdown signal (non-blocking)
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                info!("grid_worker: shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(0)) => {}
        }

        // Update uptime
        {
            let mut s = status.write().await;
            s.uptime_secs = start_time.elapsed().as_secs();
        }

        // Dequeue
        match dequeue_job(&client, &coordinator_url, &token, &worker_id).await {
            Ok(Some(job)) => {
                idle_backoff_secs = 2; // Reset backoff on successful dequeue

                let job_id = job["job_id"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                info!("grid_worker: executing job {job_id}");

                match job_executor::execute_job(&job).await {
                    Ok(result) => {
                        let _ = complete_job(
                            &client,
                            &coordinator_url,
                            &token,
                            &worker_id,
                            &job_id,
                            &result,
                        )
                        .await;
                        let mut s = status.write().await;
                        s.jobs_completed += 1;
                        info!("grid_worker: job {job_id} completed");
                    }
                    Err(e) => {
                        let _ = fail_job(
                            &client,
                            &coordinator_url,
                            &token,
                            &worker_id,
                            &job_id,
                            &e,
                        )
                        .await;
                        let mut s = status.write().await;
                        s.jobs_failed += 1;
                        warn!("grid_worker: job {job_id} failed: {e}");
                    }
                }
            }
            Ok(None) => {
                // No jobs available — back off
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(idle_backoff_secs)) => {}
                    _ = shutdown.notified() => {
                        info!("grid_worker: shutdown during idle backoff");
                        break;
                    }
                }
                idle_backoff_secs = (idle_backoff_secs * 2).min(max_backoff_secs);
            }
            Err(e) => {
                warn!("grid_worker: dequeue error: {e}");
                // Back off on errors too
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(idle_backoff_secs)) => {}
                    _ = shutdown.notified() => {
                        info!("grid_worker: shutdown during error backoff");
                        break;
                    }
                }
                idle_backoff_secs = (idle_backoff_secs * 2).min(max_backoff_secs);
            }
        }
    }

    // Final status update
    {
        let mut s = status.write().await;
        s.running = false;
        s.uptime_secs = start_time.elapsed().as_secs();
    }

    info!("grid_worker: stopped");
}
