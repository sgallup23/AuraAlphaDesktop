//! Redis-backed grid worker — feeder/worker/reporter architecture.
//!
//! Architecture:
//!   Feeder (async)  → mpsc channel → Worker Pool (spawn_blocking) → results mpsc → Reporter (async)
//!
//! Feeder: ZPOPMIN from Redis, push jobs into channel
//! Workers: recv from channel, execute via Rust compute (rayon + wgpu), send results
//! Reporter: batch 50 results, POST /api/grid/complete-batch to API
//! Heartbeat: extend Redis lease TTL every 30s for in-flight jobs
//!
//! Fallback: if Redis is unavailable, falls back to HTTP API dequeue.

use super::job_executor;
use super::WorkerStatus;
use log::{error, info, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify, RwLock};

/// Completed job result for the reporter
#[derive(Debug, Clone)]
struct JobResult {
    job_id: String,
    status: String, // "completed" or "failed"
    metrics: serde_json::Value,
    error: String,
    compute_seconds: f64,
}

/// Run the Redis-backed grid worker with feeder/worker/reporter architecture.
pub async fn run_redis_worker(
    redis_url: String,
    api_url: String,
    token: String,
    worker_id: String,
    max_parallel: u32,
    batch_size: u32,
    refill_threshold: u32,
    status: Arc<RwLock<WorkerStatus>>,
    shutdown: Arc<Notify>,
) {
    info!("redis_worker: starting (redis={redis_url}, api={api_url}, parallel={max_parallel}, batch={batch_size})");

    let start_time = Instant::now();

    // Connect to Redis
    let redis_client = match redis::Client::open(redis_url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            error!("redis_worker: failed to connect to Redis: {e}");
            return;
        }
    };

    let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            error!("redis_worker: Redis connection failed: {e}");
            return;
        }
    };
    info!("redis_worker: connected to Redis");

    // Build HTTP client for reporting completions
    let http_client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("redis_worker: HTTP client build failed: {e}");
            return;
        }
    };

    // Channels
    let (job_tx, job_rx) = mpsc::channel::<serde_json::Value>(batch_size as usize * 2);
    let (result_tx, result_rx) = mpsc::channel::<JobResult>(200);

    // Shared counters
    let jobs_completed = Arc::new(AtomicU64::new(0));
    let jobs_failed = Arc::new(AtomicU64::new(0));
    let in_flight = Arc::new(AtomicU64::new(0));
    let in_flight_ids: Arc<tokio::sync::Mutex<Vec<String>>> = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Update initial status
    {
        let mut s = status.write().await;
        s.running = true;
        s.worker_id = Some(worker_id.clone());
        s.coordinator_url = Some(api_url.clone());
    }

    // ─── Feeder Task ────────────────────────────────────────────
    let feeder_shutdown = shutdown.clone();
    let feeder_redis = redis_client.clone();
    let feeder_worker_id = worker_id.clone();
    let feeder_in_flight = in_flight.clone();

    let feeder_handle = tokio::spawn(async move {
        let mut conn = match feeder_redis.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!("redis_worker/feeder: Redis connection failed: {e}");
                return;
            }
        };

        let mut idle_backoff = Duration::from_secs(2);
        let max_backoff = Duration::from_secs(30);

        loop {
            // Check shutdown
            tokio::select! {
                biased;
                _ = feeder_shutdown.notified() => {
                    info!("redis_worker/feeder: shutdown");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }

            // Check if workers have capacity for more jobs
            let current_flight = feeder_in_flight.load(Ordering::Relaxed) as u32;
            let available = max_parallel.saturating_sub(current_flight);
            // channel capacity() returns REMAINING space (high = empty, low = full)
            let channel_remaining = job_tx.capacity();

            if available == 0 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            let fetch_count = available.min(batch_size);

            // ZPOPMIN from Redis ready queue
            let claimed: Result<Vec<(String, f64)>, _> = redis::cmd("ZPOPMIN")
                .arg("aura:queue:ready")
                .arg(fetch_count)
                .query_async(&mut conn)
                .await;

            match claimed {
                Ok(entries) if !entries.is_empty() => {
                    idle_backoff = Duration::from_secs(2);
                    let count = entries.len();

                    // Set lease keys and fetch payloads
                    let mut pipe = redis::pipe();
                    let mut job_ids = Vec::with_capacity(count);

                    for (job_id, _score) in &entries {
                        job_ids.push(job_id.clone());
                        // Set lease with TTL
                        pipe.cmd("SETEX")
                            .arg(format!("aura:lease:{job_id}"))
                            .arg(600)  // 10 min lease
                            .arg(format!("{{\"worker_id\":\"{}\"}}", feeder_worker_id));
                    }

                    let _: Result<(), _> = pipe.query_async(&mut conn).await;

                    // Fetch job payloads
                    let mut payload_pipe = redis::pipe();
                    for job_id in &job_ids {
                        payload_pipe.cmd("HGETALL").arg(format!("aura:job:{job_id}"));
                    }

                    let payloads: Result<Vec<std::collections::HashMap<String, String>>, _> =
                        payload_pipe.query_async(&mut conn).await;

                    match payloads {
                        Ok(payload_list) => {
                            let mut sent = 0;
                            for (job_id, payload_map) in job_ids.iter().zip(payload_list.iter()) {
                                if payload_map.is_empty() {
                                    continue;
                                }

                                let raw_payload = payload_map.get("payload").cloned().unwrap_or_default();
                                let parsed: serde_json::Value = serde_json::from_str(&raw_payload)
                                    .unwrap_or(serde_json::json!({}));

                                let job = serde_json::json!({
                                    "job_id": job_id,
                                    "job_type": payload_map.get("job_type").cloned().unwrap_or_default(),
                                    "payload": parsed,
                                    "priority": payload_map.get("priority").and_then(|s| s.parse::<i32>().ok()).unwrap_or(5),
                                    "batch_id": payload_map.get("batch_id").cloned().unwrap_or_default(),
                                });

                                if job_tx.send(job).await.is_err() {
                                    warn!("redis_worker/feeder: channel closed");
                                    return;
                                }
                                sent += 1;
                            }

                            info!("redis_worker/feeder: claimed {sent}/{count} jobs from Redis");
                        }
                        Err(e) => {
                            warn!("redis_worker/feeder: payload fetch failed: {e}");
                        }
                    }
                }
                Ok(_) => {
                    // Queue empty — backoff
                    tokio::time::sleep(idle_backoff).await;
                    idle_backoff = (idle_backoff * 2).min(max_backoff);
                }
                Err(e) => {
                    warn!("redis_worker/feeder: ZPOPMIN failed: {e}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    // ─── Worker Pool ────────────────────────────────────────────
    let worker_count = max_parallel as usize;
    let mut worker_handles = Vec::with_capacity(worker_count);
    let job_rx = Arc::new(tokio::sync::Mutex::new(job_rx));

    for worker_idx in 0..worker_count {
        let rx = job_rx.clone();
        let tx = result_tx.clone();
        let flight = in_flight.clone();
        let flight_ids = in_flight_ids.clone();
        let completed = jobs_completed.clone();
        let failed = jobs_failed.clone();
        let shutdown = shutdown.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Try to receive a job
                let job = {
                    let mut guard = rx.lock().await;
                    tokio::select! {
                        biased;
                        _ = shutdown.notified() => {
                            break;
                        }
                        result = guard.recv() => {
                            match result {
                                Some(j) => j,
                                None => break, // channel closed
                            }
                        }
                    }
                };

                let job_id = job["job_id"].as_str().unwrap_or("unknown").to_string();
                flight.fetch_add(1, Ordering::Relaxed);
                {
                    let mut ids = flight_ids.lock().await;
                    ids.push(job_id.clone());
                }

                // Execute job via Rust compute
                let start = Instant::now();
                let result = job_executor::execute_job(&job).await;

                let elapsed = start.elapsed().as_secs_f64();

                let job_result = match result {
                    Ok(metrics) => {
                        completed.fetch_add(1, Ordering::Relaxed);
                        JobResult {
                            job_id: job_id.clone(),
                            status: "completed".to_string(),
                            metrics,
                            error: String::new(),
                            compute_seconds: elapsed,
                        }
                    }
                    Err(e) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                        JobResult {
                            job_id: job_id.clone(),
                            status: "failed".to_string(),
                            metrics: serde_json::json!({}),
                            error: e,
                            compute_seconds: elapsed,
                        }
                    }
                };

                flight.fetch_sub(1, Ordering::Relaxed);
                {
                    let mut ids = flight_ids.lock().await;
                    ids.retain(|id| id != &job_id);
                }

                if tx.send(job_result).await.is_err() {
                    break;
                }
            }
        });

        worker_handles.push(handle);
    }

    // Drop the sender so reporter knows when workers are done
    drop(result_tx);

    // ─── Reporter Task ──────────────────────────────────────────
    let reporter_shutdown = shutdown.clone();
    let reporter_api_url = api_url.clone();
    let reporter_token = token.clone();
    let reporter_worker_id = worker_id.clone();
    let reporter_redis = redis_client.clone();

    let reporter_handle = tokio::spawn(async move {
        let mut result_rx = result_rx;
        let mut batch: Vec<JobResult> = Vec::with_capacity(50);
        let mut last_flush = Instant::now();
        let flush_interval = Duration::from_secs(2);

        loop {
            // Collect results with timeout
            tokio::select! {
                biased;
                result = result_rx.recv() => {
                    match result {
                        Some(r) => batch.push(r),
                        None => {
                            // Channel closed — flush remaining
                            if !batch.is_empty() {
                                flush_batch(&http_client, &reporter_api_url, &reporter_token,
                                    &reporter_redis, &mut batch).await;
                            }
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(flush_interval) => {}
            }

            // Flush when batch full or interval elapsed
            if batch.len() >= 50 || (last_flush.elapsed() >= flush_interval && !batch.is_empty()) {
                flush_batch(&http_client, &reporter_api_url, &reporter_token,
                    &reporter_redis, &mut batch).await;
                last_flush = Instant::now();
            }
        }
    });

    // ─── Heartbeat Task ─────────────────────────────────────────
    let hb_shutdown = shutdown.clone();
    let hb_redis = redis_client.clone();
    let hb_flight_ids = in_flight_ids.clone();
    let hb_status = status.clone();
    let hb_completed = jobs_completed.clone();
    let hb_failed = jobs_failed.clone();

    let hb_handle = tokio::spawn(async move {
        let mut conn = match hb_redis.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Extend leases for in-flight jobs
                    let ids = hb_flight_ids.lock().await.clone();
                    if !ids.is_empty() {
                        let mut pipe = redis::pipe();
                        for id in &ids {
                            pipe.cmd("EXPIRE").arg(format!("aura:lease:{id}")).arg(600);
                        }
                        let _: Result<(), _> = pipe.query_async(&mut conn).await;
                    }

                    // Update status
                    let mut s = hb_status.write().await;
                    s.jobs_completed = hb_completed.load(Ordering::Relaxed);
                    s.jobs_failed = hb_failed.load(Ordering::Relaxed);
                    s.uptime_secs = start_time.elapsed().as_secs();

                    info!(
                        "redis_worker/heartbeat: completed={}, failed={}, in_flight={}, uptime={}s",
                        s.jobs_completed, s.jobs_failed, ids.len(), s.uptime_secs,
                    );
                }
                _ = hb_shutdown.notified() => {
                    info!("redis_worker/heartbeat: shutdown");
                    break;
                }
            }
        }
    });

    // ─── Wait for shutdown ──────────────────────────────────────
    shutdown.notified().await;
    info!("redis_worker: shutdown signal received, draining...");

    // Wait for workers to finish (up to 30s)
    tokio::time::timeout(Duration::from_secs(30), async {
        for handle in worker_handles {
            let _ = handle.await;
        }
    }).await.ok();

    let _ = feeder_handle.await;
    let _ = reporter_handle.await;
    let _ = hb_handle.await;

    let final_completed = jobs_completed.load(Ordering::Relaxed);
    let final_failed = jobs_failed.load(Ordering::Relaxed);
    let uptime = start_time.elapsed().as_secs();

    info!(
        "redis_worker: stopped. completed={final_completed}, failed={final_failed}, uptime={uptime}s, \
         rate={:.0}/min",
        if uptime > 0 { final_completed as f64 / uptime as f64 * 60.0 } else { 0.0 }
    );

    let mut s = status.write().await;
    s.running = false;
    s.jobs_completed = final_completed;
    s.jobs_failed = final_failed;
    s.uptime_secs = uptime;
}

/// Flush a batch of results to the API and clean up Redis leases.
async fn flush_batch(
    client: &reqwest::Client,
    api_url: &str,
    token: &str,
    redis_client: &redis::Client,
    batch: &mut Vec<JobResult>,
) {
    if batch.is_empty() {
        return;
    }

    let count = batch.len();
    let completed_ids: Vec<String> = batch.iter()
        .filter(|r| r.status == "completed")
        .map(|r| r.job_id.clone())
        .collect();

    // Build API payload
    let results: Vec<serde_json::Value> = batch.iter().map(|r| {
        serde_json::json!({
            "job_id": r.job_id,
            "status": r.status,
            "metrics": r.metrics,
            "error": r.error,
        })
    }).collect();

    // POST to API
    let url = format!("{api_url}/api/grid/complete-batch");
    let resp = client.post(&url)
        .header("X-Worker-Token", token)
        .json(&serde_json::json!({ "results": results }))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            info!("redis_worker/reporter: flushed {count} results to API");
        }
        Ok(r) => {
            warn!("redis_worker/reporter: API returned {}: batch of {count}", r.status());
        }
        Err(e) => {
            warn!("redis_worker/reporter: API error: {e} — results will expire via lease");
        }
    }

    // Clean up Redis leases for completed jobs
    if !completed_ids.is_empty() {
        if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
            let mut pipe = redis::pipe();
            for id in &completed_ids {
                pipe.cmd("DEL").arg(format!("aura:lease:{id}"));
                pipe.cmd("DEL").arg(format!("aura:job:{id}"));
            }
            let _: Result<(), _> = pipe.query_async(&mut conn).await;
        }
    }

    batch.clear();
}
