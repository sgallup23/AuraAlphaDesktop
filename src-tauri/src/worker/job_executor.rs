//! Job executor — dispatches grid jobs by type via Python sidecar subprocess.
//!
//! Each compute-intensive job (backtest, scan, ml_inference, feature_extraction)
//! is executed by spawning `python3 compute_worker.py --job-type <type> --params '<json>'`.
//! The Python sidecar does the actual computation (indicators, trade simulation,
//! metrics) and prints a single JSON line to stdout. This module captures that
//! output and returns it as the job result.
//!
//! Lightweight jobs (health_check, ping) remain pure Rust — no subprocess needed.

use log::{error, info, warn};
use std::path::PathBuf;
use std::time::Instant;

/// Maximum time (seconds) a Python subprocess is allowed to run before being killed.
const JOB_TIMEOUT_SECS: u64 = 600;

/// Locate the compute_worker.py sidecar script.
///
/// Search order:
/// 1. `AURA_COMPUTE_WORKER` environment variable (explicit override)
/// 2. Next to the running executable (Tauri bundle: `../grid_worker/compute_worker.py`)
/// 3. Adjacent to the project root (development: `../../grid_worker/compute_worker.py`)
/// 4. Home directory: `~/.aura-worker/compute_worker.py`
/// 5. Well-known dev paths
fn find_sidecar_script() -> Result<PathBuf, String> {
    // 1. Env override
    if let Ok(path) = std::env::var("AURA_COMPUTE_WORKER") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
        warn!("AURA_COMPUTE_WORKER={path} does not exist, falling back to auto-detect");
    }

    // 2 & 3. Relative to the current executable
    if let Ok(exe) = std::env::current_exe() {
        let candidates = [
            // Tauri bundle layout: exe is in Contents/MacOS/ or next to app
            exe.parent()
                .map(|p| p.join("grid_worker").join("compute_worker.py")),
            exe.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("grid_worker").join("compute_worker.py")),
            exe.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.join("grid_worker").join("compute_worker.py")),
            // Development: exe is in target/debug/ or target/release/
            exe.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.join("grid_worker").join("compute_worker.py")),
        ];

        for candidate in candidates.iter().flatten() {
            if candidate.exists() {
                info!(
                    "job_executor: found sidecar at {}",
                    candidate.display()
                );
                return Ok(candidate.clone());
            }
        }
    }

    // 4. Home directory
    if let Some(home) = dirs::home_dir() {
        let home_path = home
            .join(".aura-worker")
            .join("compute_worker.py");
        if home_path.exists() {
            return Ok(home_path);
        }
    }

    // 5. Well-known dev paths
    let dev_paths = [
        PathBuf::from("/home/shawn/AuraAlphaDesktop/grid_worker/compute_worker.py"),
        dirs::home_dir()
            .unwrap_or_default()
            .join("AuraAlphaDesktop")
            .join("grid_worker")
            .join("compute_worker.py"),
    ];

    for p in &dev_paths {
        if p.exists() {
            return Ok(p.clone());
        }
    }

    Err(
        "compute_worker.py not found. Set AURA_COMPUTE_WORKER env var or place it in \
         grid_worker/compute_worker.py next to the app."
            .to_string(),
    )
}

/// Find the Python 3 interpreter.
fn find_python() -> String {
    if let Ok(p) = std::env::var("AURA_PYTHON") {
        return p;
    }

    // Check for venv in grid_worker directory
    if let Ok(script) = find_sidecar_script() {
        if let Some(parent) = script.parent() {
            let venv_python = parent.join("venv").join("bin").join("python3");
            if venv_python.exists() {
                return venv_python.to_string_lossy().to_string();
            }
            let venv_python_win = parent.join("venv").join("Scripts").join("python.exe");
            if venv_python_win.exists() {
                return venv_python_win.to_string_lossy().to_string();
            }
        }
    }

    // Fall back to system Python
    for cmd in &["python3", "python"] {
        if let Ok(output) = std::process::Command::new(cmd)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return cmd.to_string();
            }
        }
    }

    "python3".to_string()
}

/// Execute a grid job and return the result payload.
///
/// The `job` value is the raw JSON received from the coordinator's dequeue
/// endpoint. Expected shape:
/// ```json
/// {
///   "job_id": "abc-123",
///   "job_type": "backtest",
///   "params": { ... }
/// }
/// ```
///
/// Returns `Ok(result_json)` on success or `Err(error_message)` on failure.
pub async fn execute_job(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let job_type = job["job_type"].as_str().unwrap_or("unknown");
    let job_id = job["job_id"].as_str().unwrap_or("unknown");

    info!("job_executor: dispatching job {job_id} (type={job_type})");

    match job_type {
        // Compute-intensive jobs — pure Rust via compute module (zero Python)
        "backtest" | "research_backtest" | "walk_forward" => execute_rust_backtest(job).await,
        "scan" | "signal_gen" => execute_rust_scan(job).await,
        "ml_inference" | "ml_predict" => execute_rust_ml(job).await,
        "ml_train" | "optimization" => execute_rust_backtest(job).await, // TODO: ml_train needs memory fix
        "feature_extraction" => execute_rust_features(job).await,

        // Lightweight jobs — pure Rust
        "health_check" => execute_health_check(job).await,
        "ping" => execute_ping(job).await,

        other => {
            warn!("job_executor: unknown job_type '{other}' — running as backtest");
            execute_rust_backtest(job).await
        }
    }
}

// ── Rust-native compute dispatch (zero Python) ──────────────────────────────

async fn execute_rust_backtest(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let params = job.get("params").cloned().unwrap_or(serde_json::json!({}));
    let result = tokio::task::spawn_blocking(move || {
        crate::compute::backtest::execute_backtest_job(&params)
    }).await.map_err(|e| format!("backtest task failed: {e}"))??;
    let mut r = result;
    r["compute_seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
    Ok(r)
}

async fn execute_rust_scan(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let params = job.get("params").cloned().unwrap_or(serde_json::json!({}));
    let result = tokio::task::spawn_blocking(move || {
        crate::compute::scanner::execute_scan_job(&params)
    }).await.map_err(|e| format!("scan task failed: {e}"))??;
    let mut r = result;
    r["compute_seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
    Ok(r)
}

async fn execute_rust_ml(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let params = job.get("params").cloned().unwrap_or(serde_json::json!({}));
    let result = tokio::task::spawn_blocking(move || {
        crate::compute::ml::execute_ml_job(&params)
    }).await.map_err(|e| format!("ml task failed: {e}"))??;
    let mut r = result;
    r["compute_seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
    Ok(r)
}

async fn execute_rust_ml_train(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    // ML training is memory-heavy (loads all symbols). Use large stack thread.
    let start = Instant::now();
    let payload = job.get("payload").cloned().unwrap_or(serde_json::json!({}));
    let result = tokio::task::spawn_blocking(move || {
        // Run in a thread with 16MB stack to handle large DataFrames
        let handle = std::thread::Builder::new()
            .name("ml_train".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let params: crate::compute::ml_train::MlTrainParams =
                    serde_json::from_value(payload).unwrap_or_default();
                crate::compute::ml_train::train_model(&params)
            })
            .map_err(|e| format!("thread spawn failed: {e}"))?;
        handle.join().map_err(|_| "ml_train thread panicked".to_string())?
    }).await.map_err(|e| format!("ml_train task failed: {e}"))??;

    let mut r = serde_json::json!({
        "status": result.status,
        "accuracy": result.accuracy,
        "precision": result.precision,
        "recall": result.recall,
        "f1_score": result.f1_score,
        "train_samples": result.train_samples,
        "test_samples": result.test_samples,
        "model_path": result.model_path,
    });
    r["compute_seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
    r["job_type"] = serde_json::json!("ml_train");
    Ok(r)
}

async fn execute_rust_features(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let params = job.get("params").cloned().unwrap_or(serde_json::json!({}));
    let result = tokio::task::spawn_blocking(move || {
        crate::compute::features::execute_features_job(&params)
    }).await.map_err(|e| format!("feature extraction task failed: {e}"))??;
    let mut r = result;
    r["compute_seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
    Ok(r)
}

// ── Legacy Python sidecar (kept as dead code for reference) ─────────────────
#[allow(dead_code)]
/// Spawn the Python sidecar subprocess and capture its JSON output.
async fn execute_via_sidecar(
    job_type: &str,
    job: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let job_id = job["job_id"].as_str().unwrap_or("unknown");

    // Locate sidecar script
    let script_path = find_sidecar_script()?;
    let python = find_python();

    // Serialize the entire job as the params argument
    let params_json = serde_json::to_string(job)
        .map_err(|e| format!("Failed to serialize job params: {e}"))?;

    info!(
        "job_executor: spawning {python} {} --job-type {job_type} for job {job_id}",
        script_path.display()
    );

    // Spawn the subprocess using tokio::process for async
    let output = tokio::process::Command::new(&python)
        .arg(script_path.to_string_lossy().as_ref())
        .arg("--job-type")
        .arg(job_type)
        .arg("--params")
        .arg(&params_json)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to spawn Python sidecar ({python}): {e}. \
                 Ensure Python 3 with numpy/polars is installed."
            )
        })?;

    // Wait with timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(JOB_TIMEOUT_SECS),
        output.wait_with_output(),
    )
    .await;

    let elapsed = start.elapsed().as_secs_f64();

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stderr.is_empty() {
                // Log stderr but don't fail — Python may emit warnings
                for line in stderr.lines().take(10) {
                    warn!("job_executor: [stderr] {line}");
                }
            }

            if output.status.success() || !stdout.trim().is_empty() {
                // Parse the last non-empty line of stdout as JSON
                // (in case Python printed warnings before the result)
                let json_line = stdout
                    .lines()
                    .rev()
                    .find(|l| l.trim().starts_with('{'))
                    .unwrap_or(stdout.trim());

                match serde_json::from_str::<serde_json::Value>(json_line) {
                    Ok(mut result) => {
                        // Inject compute_seconds if not already present
                        if result.get("compute_seconds").is_none() {
                            result["compute_seconds"] =
                                serde_json::json!(format!("{:.3}", elapsed).parse::<f64>().unwrap_or(elapsed));
                        }
                        info!(
                            "job_executor: job {job_id} completed via sidecar in {elapsed:.2}s"
                        );
                        Ok(result)
                    }
                    Err(parse_err) => {
                        let truncated: String = stdout.chars().take(500).collect();
                        error!(
                            "job_executor: failed to parse sidecar output for job {job_id}: \
                             {parse_err}. stdout={truncated}"
                        );
                        Err(format!(
                            "Sidecar returned invalid JSON: {parse_err}. \
                             Output: {truncated}"
                        ))
                    }
                }
            } else {
                let stderr_truncated: String = stderr.chars().take(500).collect();
                let exit_code = output.status.code().unwrap_or(-1);
                error!(
                    "job_executor: sidecar exited with code {exit_code} for job {job_id}"
                );
                Err(format!(
                    "Python sidecar exited with code {exit_code}: {stderr_truncated}"
                ))
            }
        }
        Ok(Err(e)) => {
            error!("job_executor: sidecar I/O error for job {job_id}: {e}");
            Err(format!("Sidecar I/O error: {e}"))
        }
        Err(_) => {
            error!(
                "job_executor: sidecar timed out after {JOB_TIMEOUT_SECS}s for job {job_id}"
            );
            Err(format!(
                "Job {job_id} timed out after {JOB_TIMEOUT_SECS}s"
            ))
        }
    }
}

// ─── Lightweight handlers (pure Rust, no subprocess) ─────────────────────────

/// Health check — returns system info.
async fn execute_health_check(_job: &serde_json::Value) -> Result<serde_json::Value, String> {
    // Also check if Python sidecar is available
    let sidecar_ok = find_sidecar_script().is_ok();
    let python_cmd = find_python();

    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "health_check",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": env!("CARGO_PKG_VERSION"),
        "sidecar_available": sidecar_ok,
        "python_command": python_cmd,
        "cpu_cores": std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
    }))
}

/// Ping — immediate acknowledgement.
async fn execute_ping(_job: &serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "ping",
        "pong": true,
    }))
}
