//! Job executor — dispatches grid jobs by type and returns results.
//!
//! Each `job_type` maps to a handler function. Currently all handlers are stubs
//! that return a "completed" status. Real implementations will be added as the
//! coordinator starts dispatching actual job types (backtest, scan, ML inference).

use log::{info, warn};

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
    let job_type = job["job_type"]
        .as_str()
        .unwrap_or("unknown");
    let job_id = job["job_id"]
        .as_str()
        .unwrap_or("unknown");

    info!("job_executor: dispatching job {job_id} (type={job_type})");

    match job_type {
        "backtest" => execute_backtest(job).await,
        "scan" => execute_scan(job).await,
        "ml_inference" => execute_ml_inference(job).await,
        "feature_extraction" => execute_feature_extraction(job).await,
        "health_check" => execute_health_check(job).await,
        "ping" => execute_ping(job).await,
        other => {
            warn!("job_executor: unknown job_type '{other}' — returning stub result");
            Ok(serde_json::json!({
                "status": "completed",
                "job_type": other,
                "message": "Unknown job type — stub handler returned success",
            }))
        }
    }
}

// ─── Stub handlers ──────────────────────────────────────────────────────────

/// Stub: run a strategy backtest.
async fn execute_backtest(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params = &job["params"];
    info!(
        "job_executor: backtest stub — strategy={}, symbol={}",
        params["strategy"].as_str().unwrap_or("?"),
        params["symbol"].as_str().unwrap_or("?"),
    );

    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "backtest",
        "message": "Backtest stub — real implementation pending",
        "metrics": {
            "total_return": 0.0,
            "sharpe_ratio": 0.0,
            "max_drawdown": 0.0,
            "trades": 0,
        }
    }))
}

/// Stub: run a market scan.
async fn execute_scan(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params = &job["params"];
    info!(
        "job_executor: scan stub — scan_type={}",
        params["scan_type"].as_str().unwrap_or("?"),
    );

    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "scan",
        "message": "Scan stub — real implementation pending",
        "results": [],
    }))
}

/// Stub: run ML model inference.
async fn execute_ml_inference(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params = &job["params"];
    info!(
        "job_executor: ml_inference stub — model={}",
        params["model"].as_str().unwrap_or("?"),
    );

    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "ml_inference",
        "message": "ML inference stub — real implementation pending",
        "predictions": [],
    }))
}

/// Stub: run feature extraction.
async fn execute_feature_extraction(job: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params = &job["params"];
    info!(
        "job_executor: feature_extraction stub — dataset={}",
        params["dataset"].as_str().unwrap_or("?"),
    );

    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "feature_extraction",
        "message": "Feature extraction stub — real implementation pending",
        "features_count": 0,
    }))
}

/// Health check — returns system info.
async fn execute_health_check(_job: &serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "status": "completed",
        "job_type": "health_check",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": env!("CARGO_PKG_VERSION"),
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
