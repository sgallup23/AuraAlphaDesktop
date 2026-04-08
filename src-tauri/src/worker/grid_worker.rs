//! Core grid worker loop — token resolution, registration, heartbeat, parallel job processing.
//!
//! This module contains the long-running async function `run_worker` that is
//! spawned by the `start_grid_worker` IPC command in `mod.rs`.
//!
//! Parallel execution:
//! - Dequeues multiple jobs per cycle (up to `max_parallel` or available_cores/2)
//! - Uses `tokio::spawn` to execute jobs concurrently
//! - Reports results as each job completes (no waiting for entire batch)
//! - Tracks in-flight job count to avoid overloading the machine

use super::job_executor;
use super::WorkerStatus;
use log::{error, info, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Notify, RwLock, Semaphore};

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
///
/// When `telemetry_consented` is false, the auto-provision request sends
/// "anonymous" instead of the real hostname.
async fn resolve_token(coordinator_url: &str, telemetry_consented: bool) -> Result<(String, Option<String>), String> {
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
        .user_agent("Mozilla/5.0 (compatible; AuraAlpha-Desktop/5.0)")
        .build()
        .map_err(|e| format!("HTTP client build error: {e}"))?;

    // Always send real hostname for worker identification — these are the owner's machines.
    // Telemetry consent gates analytics/crash reporting, not worker registration.
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
        .user_agent("Mozilla/5.0 (compatible; AuraAlpha-Desktop/5.0)")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))
}

/// Register this worker with the coordinator.
///
/// When `telemetry_consented` is false, hostname is sent as "anonymous"
/// and cpu_cores/memory_gb are sent as 0 to protect user privacy.
async fn register(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    max_parallel: u32,
    telemetry_consented: bool,
) -> Result<String, String> {
    // Detect GPU for registration
    let hw = crate::compute::hardware::detect_hardware();

    // Always report real hardware for worker identification and capacity planning.
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
        "max_parallel": max_parallel,
        "gpu_model": hw.gpu_name,
        "gpu_vram_gb": (hw.gpu_vram_mb as f64 / 1024.0),
        "cuda_available": hw.cuda_available,
        "supported_job_types": ["backtest", "research_backtest", "scan", "ml_inference", "feature_extraction"],
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

    info!("grid_worker: registered as {assigned_id} (max_parallel={max_parallel})");
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
    in_flight: u64,
    telemetry_consented: bool,
) -> Result<(), String> {
    // Always send hostname and hardware — these are the owner's own machines,
    // not anonymous third-party contributors. Telemetry consent is for analytics only.
    let machine_hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let hw = crate::compute::hardware::detect_hardware();
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let memory_gb = if true {
        (hw.ram_gb).round() as u32
    } else {
        0u32
    };

    let body = serde_json::json!({
        "worker_id": worker_id,
        "hostname": machine_hostname,
        "jobs_completed": jobs_completed,
        "jobs_failed": jobs_failed,
        "jobs_in_flight": in_flight,
        "cpu_cores": cpu_cores,
        "memory_gb": memory_gb,
        "gpu_model": hw.gpu_name,
        "gpu_vram_gb": (hw.gpu_vram_mb as f64 / 1024.0),
        "cuda_available": hw.cuda_available,
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

/// Dequeue multiple jobs from the coordinator. Returns empty vec if no jobs available.
async fn dequeue_jobs(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    count: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let body = serde_json::json!({
        "worker_id": worker_id,
        "count": count,
        "max_jobs": count,
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
        return Ok(vec![]);
    }

    if !resp.status().is_success() {
        return Err(format!("dequeue returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("dequeue parse error: {e}"))?;

    if let Some(jobs) = data["jobs"].as_array() {
        Ok(jobs.clone())
    } else {
        Ok(vec![])
    }
}

/// Report job completion to the coordinator.
async fn complete_job(
    client: &reqwest::Client,
    coordinator_url: &str,
    token: &str,
    worker_id: &str,
    job_id: &str,
    result: &serde_json::Value,
    compute_seconds: f64,
) -> Result<(), String> {
    let body = serde_json::json!({
        "job_id": job_id,
        "result": result,
        "metrics": result.get("metrics").unwrap_or(result),
        "compute_seconds": compute_seconds,
        "status": "completed",
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
    let body = serde_json::json!({
        "job_id": job_id,
        "error": error_msg,
        "status": "failed",
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

// ─── Parallel execution helpers ──────────────────────────────────────────────

/// Determine how many jobs to dequeue based on available CPU cores.
///
/// Strategy: each backtest/scan job uses rayon internally, so we want
/// fewer concurrent jobs than raw core count. The formula is:
///   - CPU-bound (backtest/scan): cores / 2  (each job uses rayon across ~half the cores)
///   - I/O-bound jobs would use cores * 2, but our jobs are CPU-bound
///   - Minimum 4 to keep the pipeline fed
///   - Leave headroom for OS, UI thread, and heartbeat
fn compute_max_parallel() -> u32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);

    // Memory check: each job can use ~100-200 MB for large symbol universes.
    // Cap parallel jobs so we don't exceed ~80% of available RAM.
    let ram_gb = {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        (sys.total_memory() as f64 / 1_073_741_824.0) as u32
    };
    // ~200MB per job estimate -> max jobs from RAM = ram_gb * 5
    let ram_limit = (ram_gb * 5).max(4);

    // CPU-bound: cores/2 since rayon expands each job internally
    let cpu_limit = (cores / 2).max(4);

    let parallel = cpu_limit.min(ram_limit);
    info!(
        "grid_worker: {cores} cores, {ram_gb} GB RAM -> max_parallel={parallel} \
         (cpu_limit={cpu_limit}, ram_limit={ram_limit})"
    );
    parallel
}

// ─── Main worker loop ───────────────────────────────────────────────────────

/// Entry point for the grid worker, spawned as a Tokio task.
///
/// Lifecycle:
/// 1. Resolve token (env -> file -> auto-provision)
/// 2. Register with coordinator (includes max_parallel capability)
/// 3. Spawn heartbeat task (30s interval)
/// 4. Loop: dequeue batch -> spawn parallel executors -> report results
/// 5. On `Notify` signal or unrecoverable error, exit cleanly
///
/// When `telemetry_consented` is false, hostname/cpu/ram fields are
/// anonymized to comply with GDPR/CCPA requirements.
pub async fn run_worker(
    coordinator_url: String,
    status: Arc<RwLock<WorkerStatus>>,
    shutdown: Arc<Notify>,
) {
    // Check telemetry consent at worker start
    let telemetry_consented = crate::telemetry_consent::is_telemetry_consented();
    let max_parallel = compute_max_parallel();
    info!(
        "grid_worker: starting (coordinator={coordinator_url}, max_parallel={max_parallel})"
    );
    let start_time = Instant::now();

    // Resolve token
    let (token, provisioned_id) = match resolve_token(&coordinator_url, telemetry_consented).await {
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
    let worker_id = match register(
        &client,
        &coordinator_url,
        &token,
        &initial_id,
        max_parallel,
        telemetry_consented,
    )
    .await
    {
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

    // Shared counters for parallel job tracking
    let jobs_completed = Arc::new(AtomicU64::new(0));
    let jobs_failed = Arc::new(AtomicU64::new(0));
    let in_flight = Arc::new(AtomicU64::new(0));

    // Semaphore to limit concurrent job execution
    let semaphore = Arc::new(Semaphore::new(max_parallel as usize));

    // Spawn heartbeat loop
    let hb_client = client.clone();
    let hb_url = coordinator_url.clone();
    let hb_token = token.clone();
    let hb_worker_id = worker_id.clone();
    let hb_status = Arc::clone(&status);
    let hb_shutdown = Arc::clone(&shutdown);
    let hb_completed = Arc::clone(&jobs_completed);
    let hb_failed = Arc::clone(&jobs_failed);
    let hb_in_flight = Arc::clone(&in_flight);
    let hb_telemetry = telemetry_consented;

    let _heartbeat_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let completed = hb_completed.load(Ordering::Relaxed);
                    let failed = hb_failed.load(Ordering::Relaxed);
                    let flying = hb_in_flight.load(Ordering::Relaxed);

                    let _ = heartbeat(
                        &hb_client,
                        &hb_url,
                        &hb_token,
                        &hb_worker_id,
                        completed,
                        failed,
                        flying,
                        hb_telemetry,
                    )
                    .await;

                    // Update status for IPC queries
                    let mut s = hb_status.write().await;
                    s.jobs_completed = completed;
                    s.jobs_failed = failed;
                }
                _ = hb_shutdown.notified() => {
                    info!("grid_worker: heartbeat loop shutting down");
                    break;
                }
            }
        }
    });

    // Main dequeue/execute loop
    //
    // Pipeline optimization: we dequeue the next batch while current jobs
    // are still running, so compute and network overlap. The inter-batch
    // delay is reduced from 500ms to 50ms when jobs were found (we're only
    // rate-limiting the API, not the compute).
    let mut idle_backoff_secs: u64 = 1;
    let max_backoff_secs: u64 = 30;
    let mut consecutive_empty: u32 = 0;

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

        // Update uptime (only every ~10 iterations to reduce lock contention)
        let completed_now = jobs_completed.load(Ordering::Relaxed);
        let failed_now = jobs_failed.load(Ordering::Relaxed);
        {
            let mut s = status.write().await;
            s.uptime_secs = start_time.elapsed().as_secs();
            s.jobs_completed = completed_now;
            s.jobs_failed = failed_now;
        }

        // Calculate how many jobs we can take: available semaphore permits.
        // Prefetch: request slightly more than current available slots so
        // jobs are queued and ready when a slot opens up.
        let current_in_flight = in_flight.load(Ordering::Relaxed) as u32;
        let available_slots = max_parallel.saturating_sub(current_in_flight);

        if available_slots == 0 {
            // All slots busy — short wait, then check again
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
                _ = shutdown.notified() => {
                    info!("grid_worker: shutdown while waiting for slots");
                    break;
                }
            }
            continue;
        }

        // Prefetch: request up to 2x available slots so the next batch is
        // ready in the semaphore queue when current jobs finish.
        let prefetch = (available_slots * 2).min(max_parallel * 2);
        let batch_size = prefetch.min(50);
        match dequeue_jobs(&client, &coordinator_url, &token, &worker_id, batch_size).await {
            Ok(jobs) if !jobs.is_empty() => {
                idle_backoff_secs = 1;
                consecutive_empty = 0;
                let job_count = jobs.len();

                info!(
                    "grid_worker: dequeued {job_count} jobs (in_flight={current_in_flight}, \
                     max_parallel={max_parallel})"
                );

                // Spawn each job as a separate tokio task
                for job in jobs {
                    let sem = Arc::clone(&semaphore);
                    let client = client.clone();
                    let url = coordinator_url.clone();
                    let tok = token.clone();
                    let wid = worker_id.clone();
                    let completed_counter = Arc::clone(&jobs_completed);
                    let failed_counter = Arc::clone(&jobs_failed);
                    let flight_counter = Arc::clone(&in_flight);

                    tokio::spawn(async move {
                        // Acquire semaphore permit (blocks if at max_parallel)
                        let _permit = match sem.acquire().await {
                            Ok(p) => p,
                            Err(_) => {
                                error!("grid_worker: semaphore closed unexpectedly");
                                return;
                            }
                        };

                        flight_counter.fetch_add(1, Ordering::Relaxed);

                        let job_id = job["job_id"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string();

                        let job_start = Instant::now();

                        match job_executor::execute_job(&job).await {
                            Ok(result) => {
                                let compute_secs = result["compute_seconds"]
                                    .as_f64()
                                    .unwrap_or(job_start.elapsed().as_secs_f64());

                                let _ = complete_job(
                                    &client,
                                    &url,
                                    &tok,
                                    &wid,
                                    &job_id,
                                    &result,
                                    compute_secs,
                                )
                                .await;
                                completed_counter.fetch_add(1, Ordering::Relaxed);
                                info!("grid_worker: job {job_id} completed in {compute_secs:.1}s");
                            }
                            Err(e) => {
                                let _ = fail_job(
                                    &client, &url, &tok, &wid, &job_id, &e,
                                )
                                .await;
                                failed_counter.fetch_add(1, Ordering::Relaxed);
                                warn!("grid_worker: job {job_id} failed: {e}");
                            }
                        }

                        flight_counter.fetch_sub(1, Ordering::Relaxed);
                        // _permit is dropped here, releasing the semaphore slot
                    });
                }

                // Minimal pause between dequeue cycles when jobs are flowing.
                // 50ms is enough to avoid hammering the API while keeping
                // the pipeline saturated.
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                    _ = shutdown.notified() => {
                        info!("grid_worker: shutdown between dequeue cycles");
                        break;
                    }
                }
            }
            Ok(_) => {
                // No jobs available — exponential backoff
                consecutive_empty += 1;
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

    // Wait for in-flight jobs to finish (up to 30 seconds)
    let remaining = in_flight.load(Ordering::Relaxed);
    if remaining > 0 {
        info!(
            "grid_worker: waiting for {remaining} in-flight jobs to complete (max 30s)..."
        );
        let drain_start = Instant::now();
        while in_flight.load(Ordering::Relaxed) > 0
            && drain_start.elapsed().as_secs() < 30
        {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        let still_remaining = in_flight.load(Ordering::Relaxed);
        if still_remaining > 0 {
            warn!(
                "grid_worker: {still_remaining} jobs still in flight at shutdown — \
                 they will be abandoned"
            );
        }
    }

    // Final status update
    {
        let mut s = status.write().await;
        s.running = false;
        s.uptime_secs = start_time.elapsed().as_secs();
        s.jobs_completed = jobs_completed.load(Ordering::Relaxed);
        s.jobs_failed = jobs_failed.load(Ordering::Relaxed);
    }

    info!(
        "grid_worker: stopped (completed={}, failed={}, uptime={}s)",
        jobs_completed.load(Ordering::Relaxed),
        jobs_failed.load(Ordering::Relaxed),
        start_time.elapsed().as_secs()
    );
}
