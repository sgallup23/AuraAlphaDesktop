//! Headless grid worker — pure Rust compute, no Tauri/GTK required.
//!
//! Supports two modes:
//!   --redis-url redis://...  → Redis-backed feeder/worker/reporter (fast path)
//!   (no --redis-url)         → HTTP API dequeue (legacy fallback)

use aura_alpha_desktop_lib::worker::grid_worker;
use aura_alpha_desktop_lib::worker::redis_worker;
use aura_alpha_desktop_lib::worker::WorkerStatus;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

#[tokio::main]
async fn main() {
    // Init logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // Configure rayon global thread pool to use all available CPU cores.
    let num_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .num_threads(num_cores)
        .thread_name(|idx| format!("grid-rayon-{}", idx))
        .build_global()
    {
        log::debug!("Rayon global pool already initialized: {}", e);
    } else {
        log::info!("Rayon global thread pool: {} threads", num_cores);
    }

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    let redis_url = get_arg(&args, "--redis-url")
        .or_else(|| std::env::var("REDIS_URL").ok());
    let api_url = get_arg(&args, "--api-url")
        .or_else(|| std::env::var("COORDINATOR_URL").ok())
        .or_else(|| std::env::var("API_URL").ok())
        .unwrap_or_else(|| {
            // Try config file
            let config_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("cc.auraalpha.desktop")
                .join("config.json");
            if let Ok(data) = std::fs::read_to_string(&config_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(url) = json["coordinator_url"].as_str() {
                        return url.to_string();
                    }
                }
            }
            "https://auraalpha.cc".to_string()
        });
    let batch_size: u32 = get_arg(&args, "--batch-size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(250);
    let refill_threshold: u32 = get_arg(&args, "--refill-threshold")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let max_parallel: u32 = get_arg(&args, "--workers")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            let cores = std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(4);
            cores.saturating_sub(4).max(4)
        });

    let mode = if redis_url.is_some() { "Redis" } else { "HTTP API" };

    println!("═══════════════════════════════════════════════════════════");
    println!("  Aura Alpha Grid Worker (Rust headless)");
    println!("  Mode: {mode}");
    if let Some(ref url) = redis_url {
        println!("  Redis: {url}");
    }
    println!("  API: {api_url}");
    println!("  Workers: {max_parallel} | Batch: {batch_size} | Refill: {refill_threshold}");
    println!("  Press Ctrl+C to stop");
    println!("═══════════════════════════════════════════════════════════");

    let status = Arc::new(RwLock::new(WorkerStatus::default()));
    let shutdown = Arc::new(Notify::new());

    // Handle Ctrl+C
    let shutdown_signal = Arc::clone(&shutdown);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\nShutting down gracefully...");
        shutdown_signal.notify_waiters();
    });

    if let Some(redis_url) = redis_url {
        // Resolve token
        let token = resolve_token(&api_url).await;
        let worker_id = resolve_worker_id();

        // Redis mode — feeder/worker/reporter
        redis_worker::run_redis_worker(
            redis_url,
            api_url,
            token,
            worker_id,
            max_parallel,
            batch_size,
            refill_threshold,
            status,
            shutdown,
        ).await;
    } else {
        // Legacy HTTP API mode
        grid_worker::run_worker(api_url, status, shutdown).await;
    }

    println!("Worker stopped.");
}

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

async fn resolve_token(api_url: &str) -> String {
    // Try env var
    if let Ok(token) = std::env::var("AURA_WORKER_TOKEN") {
        return token;
    }
    // Try stored token file
    let token_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".aura-worker")
        .join("grid_token.json");
    if let Ok(data) = std::fs::read_to_string(&token_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(token) = json["token"].as_str() {
                return token.to_string();
            }
        }
    }
    // Auto-provision
    let client = reqwest::Client::new();
    let hostname = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();
    let resp = client.post(format!("{api_url}/api/cluster/contributor/auto-provision"))
        .json(&serde_json::json!({
            "hostname": hostname,
            "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            "ram_gb": 0,
        }))
        .send().await;

    if let Ok(r) = resp {
        if let Ok(data) = r.json::<serde_json::Value>().await {
            let token = data["token"].as_str().unwrap_or("").to_string();
            let worker_id = data["worker_id"].as_str().unwrap_or("").to_string();
            // Save for next time
            let save_data = serde_json::json!({
                "token": token,
                "worker_id": worker_id,
                "coordinator_url": api_url,
            });
            if let Some(parent) = token_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&token_path, serde_json::to_string_pretty(&save_data).unwrap_or_default());
            return token;
        }
    }
    String::new()
}

fn resolve_worker_id() -> String {
    let token_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".aura-worker")
        .join("grid_token.json");
    if let Ok(data) = std::fs::read_to_string(&token_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(id) = json["worker_id"].as_str() {
                return id.to_string();
            }
        }
    }
    format!("desktop-{}", &uuid::Uuid::new_v4().to_string()[..8])
}
