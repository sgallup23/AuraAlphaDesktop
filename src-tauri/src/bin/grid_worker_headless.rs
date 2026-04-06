//! Headless grid worker — pure Rust compute, no Tauri/GTK required.
//! Runs the grid worker loop directly with coordinator HTTP integration.

use aura_alpha_desktop_lib::worker::grid_worker;
use aura_alpha_desktop_lib::worker::WorkerStatus;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

#[tokio::main]
async fn main() {
    // Init logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // Read coordinator URL from env or config
    let coordinator_url = std::env::var("COORDINATOR_URL")
        .unwrap_or_else(|_| {
            // Try to read from config file
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

    println!("═══════════════════════════════════════════════════════════");
    println!("  Aura Alpha Grid Worker (Rust headless)");
    println!("  Coordinator: {coordinator_url}");
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

    // Run the worker
    grid_worker::run_worker(coordinator_url, status, shutdown).await;

    println!("Worker stopped.");
}
