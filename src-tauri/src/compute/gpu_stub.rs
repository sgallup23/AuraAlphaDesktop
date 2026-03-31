//! GPU stub — provides the same API as gpu.rs but always returns "no GPU".
//!
//! Used when the `gpu` feature is not enabled (default build without wgpu).
//! This keeps binary size small (~24MB vs ~55MB with wgpu).

/// Benchmark result (same shape as the real one).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuBenchmarkResult {
    pub operation: String,
    pub data_points: usize,
    pub period: usize,
    pub cpu_time_us: u64,
    pub gpu_time_us: u64,
    pub speedup: f64,
    pub gpu_adapter: String,
    pub results_match: bool,
    pub max_abs_error: f64,
}

/// GPU status returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuStatus {
    pub available: bool,
    pub adapter_name: String,
    pub backend: String,
    pub threshold_bars: usize,
}

/// Always false — GPU feature not compiled in.
pub fn is_gpu_available() -> bool {
    false
}

/// Always None — GPU feature not compiled in.
pub fn gpu_adapter_name() -> Option<String> {
    None
}

/// Always None — GPU feature not compiled in.
pub async fn gpu_compute_sma(_data: &[f64], _period: usize) -> Option<Vec<f64>> {
    None
}

/// Always None — GPU feature not compiled in.
pub async fn gpu_compute_ema(_data: &[f64], _period: usize) -> Option<Vec<f64>> {
    None
}

/// Always None — GPU feature not compiled in.
pub async fn gpu_batch_sma(_datasets: &[&[f64]], _period: usize) -> Option<Vec<Vec<f64>>> {
    None
}

/// IPC command: check GPU compute availability (stub).
#[tauri::command]
pub async fn gpu_compute_status() -> GpuStatus {
    GpuStatus {
        available: false,
        adapter_name: "Not compiled (enable 'gpu' feature)".to_string(),
        backend: "None".to_string(),
        threshold_bars: super::indicators::GPU_THRESHOLD,
    }
}

/// IPC command: run a GPU vs CPU benchmark (stub — CPU only).
#[tauri::command]
pub async fn gpu_benchmark(data_points: Option<usize>, period: Option<usize>) -> GpuBenchmarkResult {
    use std::time::Instant;

    let n = data_points.unwrap_or(10_000);
    let p = period.unwrap_or(20);

    let data: Vec<f64> = (0..n).map(|i| 100.0 + (i as f64) * 0.01).collect();

    let cpu_start = Instant::now();
    let _cpu_result = super::indicators::compute_sma(&data, p);
    let cpu_time = cpu_start.elapsed();

    GpuBenchmarkResult {
        operation: "SMA".to_string(),
        data_points: n,
        period: p,
        cpu_time_us: cpu_time.as_micros() as u64,
        gpu_time_us: 0,
        speedup: 0.0,
        gpu_adapter: "Not compiled (enable 'gpu' feature)".to_string(),
        results_match: false,
        max_abs_error: f64::NAN,
    }
}
