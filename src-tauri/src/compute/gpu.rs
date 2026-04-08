//! GPU-accelerated computation via wgpu (Vulkan/Metal/DX12).
//!
//! Works on ANY GPU — NVIDIA, AMD, Intel integrated. No CUDA required.
//! Falls back gracefully to CPU if no GPU adapter is available.
//!
//! Key design decisions:
//! - Uses f32 on the GPU (WGSL does not support f64 without extensions).
//!   For financial indicators this is acceptable: f32 has ~7 decimal digits,
//!   and price data rarely exceeds 6 significant figures.
//! - GPU path only activates for large datasets (>1000 bars) where the
//!   overhead of buffer upload/download is amortized.
//! - GpuContext is initialized once (lazy) and cached for the process lifetime.

use std::sync::OnceLock;
use tokio::sync::OnceCell;
use bytemuck::{Pod, Zeroable};

// ── Shader source (WGSL) ──────────────────────────────────────────────

/// WGSL compute shader for SMA (Simple Moving Average).
///
/// Each workgroup thread computes one output element.
/// For index i, it sums data[i-period+1 .. i] and divides by period.
/// Indices before (period-1) are set to NaN.
const SHADER_SMA: &str = r#"
struct Params {
    len: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.len {
        return;
    }

    let period = params.period;
    if i < period - 1u {
        // Not enough data — output NaN.
        output[i] = bitcast<f32>(0x7FC00000u);
        return;
    }

    var sum: f32 = 0.0;
    let start = i + 1u - period;
    for (var j = start; j <= i; j = j + 1u) {
        sum = sum + input[j];
    }
    output[i] = sum / f32(period);
}
"#;

/// WGSL compute shader for EMA (Exponential Moving Average).
///
/// EMA is inherently sequential (each value depends on the previous).
/// This shader runs a single workgroup that processes the entire array
/// serially on the GPU. The benefit is avoiding CPU-GPU context switches
/// when EMA is part of a larger GPU pipeline (e.g. batch indicators for
/// hundreds of symbols dispatched together).
///
/// For a single EMA call, the CPU path is often faster. The GPU path
/// is valuable when computing EMA across many symbols in batch.
const SHADER_EMA: &str = r#"
struct Params {
    len: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = params.len;
    let period = params.period;
    let nan_val = bitcast<f32>(0x7FC00000u);
    let alpha = 2.0 / (f32(period) + 1.0);

    // Fill leading NaNs.
    for (var i = 0u; i < period; i = i + 1u) {
        if i >= n {
            return;
        }
        output[i] = nan_val;
    }

    if n <= period {
        return;
    }

    // Seed: SMA of first `period` values.
    var seed: f32 = 0.0;
    for (var i = 0u; i < period; i = i + 1u) {
        seed = seed + input[i];
    }
    seed = seed / f32(period);
    output[period - 1u] = seed;

    // EMA forward pass.
    var prev = seed;
    for (var i = period; i < n; i = i + 1u) {
        let val = input[i] * alpha + prev * (1.0 - alpha);
        output[i] = val;
        prev = val;
    }
}
"#;

/// WGSL compute shader for RSI (Relative Strength Index).
///
/// RSI is inherently sequential: avg_gain/avg_loss use Wilder smoothing
/// where each value depends on the previous. Like EMA, runs with
/// workgroup_size(1) — the GPU benefit comes from batching many symbols.
///
/// Params: len = data length, period = RSI period (typically 14).
/// Input: close prices. Output: RSI values [0, 100]. Pre-period slots = 50.0.
const SHADER_RSI: &str = r#"
struct Params {
    len: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = params.len;
    let period = params.period;

    // Fill with default RSI = 50.0 (neutral).
    for (var i = 0u; i < n; i = i + 1u) {
        output[i] = 50.0;
    }

    // Need at least period+2 data points (period+1 deltas, period for initial avg).
    if n <= period + 1u {
        return;
    }

    // Compute initial avg_gain and avg_loss from first `period` deltas.
    var sum_gain: f32 = 0.0;
    var sum_loss: f32 = 0.0;
    for (var i = 1u; i <= period; i = i + 1u) {
        let delta = input[i] - input[i - 1u];
        if delta > 0.0 {
            sum_gain = sum_gain + delta;
        } else {
            sum_loss = sum_loss - delta;
        }
    }
    var avg_gain = sum_gain / f32(period);
    var avg_loss = sum_loss / f32(period);

    // Wilder smoothing from index `period` onward.
    let period_f = f32(period);
    for (var i = period; i < n - 1u; i = i + 1u) {
        let delta = input[i + 1u] - input[i];
        var gain: f32 = 0.0;
        var loss: f32 = 0.0;
        if delta > 0.0 {
            gain = delta;
        } else {
            loss = -delta;
        }
        avg_gain = (avg_gain * (period_f - 1.0) + gain) / period_f;
        avg_loss = (avg_loss * (period_f - 1.0) + loss) / period_f;
        let rs = avg_gain / (avg_loss + 1e-10);
        output[i + 1u] = 100.0 - (100.0 / (1.0 + rs));
    }
}
"#;

/// WGSL compute shader for ATR (Average True Range).
///
/// Two-phase approach in a single shader:
/// Phase 1: Compute True Range in parallel (workgroup_size 256).
///          TR[i] = max(high[i]-low[i], |high[i]-close[i-1]|, |low[i]-close[i-1]|)
/// Phase 2: Sequential Wilder smoothing (thread 0 only after barrier).
///
/// Input layout: high[0..n], low[n..2n], close[2n..3n] packed contiguously.
/// Output: ATR values with leading NaNs.
const SHADER_ATR: &str = r#"
struct Params {
    len: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

// Scratch buffer for True Range values.
@group(0) @binding(3) var<storage, read_write> scratch: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = params.len;
    let period = params.period;
    let nan_val = bitcast<f32>(0x7FC00000u);

    // Input layout: highs at [0..n], lows at [n..2n], closes at [2n..3n].
    let h_off = 0u;
    let l_off = n;
    let c_off = 2u * n;

    // Fill output with NaN.
    for (var i = 0u; i < n; i = i + 1u) {
        output[i] = nan_val;
    }

    if n < period + 1u {
        return;
    }

    // Phase 1: Compute True Range (n-1 values, starting at index 1).
    for (var i = 1u; i < n; i = i + 1u) {
        let hl = input[h_off + i] - input[l_off + i];
        let hc = abs(input[h_off + i] - input[c_off + i - 1u]);
        let lc = abs(input[l_off + i] - input[c_off + i - 1u]);
        scratch[i - 1u] = max(hl, max(hc, lc));
    }

    let tr_len = n - 1u;
    if tr_len < period {
        return;
    }

    // Phase 2: Sequential Wilder smoothing.
    // Initial ATR = simple mean of first `period` TR values.
    var atr_val: f32 = 0.0;
    for (var j = 0u; j < period; j = j + 1u) {
        atr_val = atr_val + scratch[j];
    }
    atr_val = atr_val / f32(period);
    output[period] = atr_val;

    // Wilder smoothing: ATR[i] = (prev * (period-1) + TR[i-1]) / period
    let period_f = f32(period);
    for (var i = period + 1u; i <= tr_len; i = i + 1u) {
        atr_val = (atr_val * (period_f - 1.0) + scratch[i - 1u]) / period_f;
        output[i] = atr_val;
    }
}
"#;

/// WGSL compute shader for batch RSI — process multiple symbols at once.
///
/// Each workgroup handles one symbol sequentially (workgroup_size(1)).
/// Symbols are packed contiguously with an offset/length table.
const SHADER_BATCH_RSI: &str = r#"
struct SymbolInfo {
    offset: u32,
    len: u32,
}

struct Params {
    num_symbols: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> symbols: array<SymbolInfo>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let sym_idx = gid.x;
    if sym_idx >= params.num_symbols {
        return;
    }

    let info = symbols[sym_idx];
    let base = info.offset;
    let n = info.len;
    let period = params.period;

    // Fill with default RSI = 50.0.
    for (var i = 0u; i < n; i = i + 1u) {
        output[base + i] = 50.0;
    }

    if n <= period + 1u {
        return;
    }

    // Initial avg_gain / avg_loss from first `period` deltas.
    var sum_gain: f32 = 0.0;
    var sum_loss: f32 = 0.0;
    for (var i = 1u; i <= period; i = i + 1u) {
        let delta = input[base + i] - input[base + i - 1u];
        if delta > 0.0 {
            sum_gain = sum_gain + delta;
        } else {
            sum_loss = sum_loss - delta;
        }
    }
    var avg_gain = sum_gain / f32(period);
    var avg_loss = sum_loss / f32(period);

    // Wilder smoothing.
    let period_f = f32(period);
    for (var i = period; i < n - 1u; i = i + 1u) {
        let delta = input[base + i + 1u] - input[base + i];
        var gain: f32 = 0.0;
        var loss: f32 = 0.0;
        if delta > 0.0 {
            gain = delta;
        } else {
            loss = -delta;
        }
        avg_gain = (avg_gain * (period_f - 1.0) + gain) / period_f;
        avg_loss = (avg_loss * (period_f - 1.0) + loss) / period_f;
        let rs = avg_gain / (avg_loss + 1e-10);
        output[base + i + 1u] = 100.0 - (100.0 / (1.0 + rs));
    }
}
"#;

/// WGSL compute shader for batch ATR — process multiple symbols at once.
///
/// Each symbol's data is packed as: high[0..n], low[n..2n], close[2n..3n].
/// The symbol info stores the offset into this packed layout and the bar count.
/// Total floats per symbol = 3 * n.
const SHADER_BATCH_ATR: &str = r#"
struct SymbolInfo {
    offset: u32,
    len: u32,
}

struct Params {
    num_symbols: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> symbols: array<SymbolInfo>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let sym_idx = gid.x;
    if sym_idx >= params.num_symbols {
        return;
    }

    let info = symbols[sym_idx];
    let n = info.len;
    let period = params.period;
    let nan_val = bitcast<f32>(0x7FC00000u);

    // Input layout per symbol: highs at [offset..offset+n],
    // lows at [offset+n..offset+2n], closes at [offset+2n..offset+3n].
    let h_off = info.offset;
    let l_off = info.offset + n;
    let c_off = info.offset + 2u * n;

    // Output is indexed by symbol output offset (same as the SymbolInfo
    // but for the output buffer which has `n` floats per symbol).
    // We need a separate output offset. Use sym_idx to find it.
    // Actually the output buffer mirrors the input layout but only needs n per symbol.
    // We reuse info.offset / 3 since input has 3*n per symbol.
    let out_base = info.offset / 3u;

    // Fill output with NaN.
    for (var i = 0u; i < n; i = i + 1u) {
        output[out_base + i] = nan_val;
    }

    if n < period + 1u {
        return;
    }

    // Compute True Range and Wilder-smooth sequentially.
    // TR[i] for i in 1..n
    // Initial ATR = mean(TR[0..period])
    var atr_val: f32 = 0.0;
    for (var i = 1u; i <= period; i = i + 1u) {
        let hl = input[h_off + i] - input[l_off + i];
        let hc = abs(input[h_off + i] - input[c_off + i - 1u]);
        let lc = abs(input[l_off + i] - input[c_off + i - 1u]);
        atr_val = atr_val + max(hl, max(hc, lc));
    }
    atr_val = atr_val / f32(period);
    output[out_base + period] = atr_val;

    let period_f = f32(period);
    for (var i = period + 1u; i < n; i = i + 1u) {
        let hl = input[h_off + i] - input[l_off + i];
        let hc = abs(input[h_off + i] - input[c_off + i - 1u]);
        let lc = abs(input[l_off + i] - input[c_off + i - 1u]);
        let tr = max(hl, max(hc, lc));
        atr_val = (atr_val * (period_f - 1.0) + tr) / period_f;
        output[out_base + i] = atr_val;
    }
}
"#;

/// WGSL compute shader for batch SMA — process multiple symbols at once.
///
/// Each workgroup handles one symbol. Symbols are packed contiguously in the
/// input buffer with an offset/length table.
const SHADER_BATCH_SMA: &str = r#"
struct SymbolInfo {
    offset: u32,
    len: u32,
}

struct Params {
    num_symbols: u32,
    period: u32,
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> symbols: array<SymbolInfo>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let sym_idx = gid.y;
    let local_i = gid.x;

    if sym_idx >= params.num_symbols {
        return;
    }

    let info = symbols[sym_idx];
    if local_i >= info.len {
        return;
    }

    let global_i = info.offset + local_i;
    let period = params.period;

    if local_i < period - 1u {
        output[global_i] = bitcast<f32>(0x7FC00000u);
        return;
    }

    var sum: f32 = 0.0;
    let start = info.offset + local_i + 1u - period;
    let end = info.offset + local_i;
    for (var j = start; j <= end; j = j + 1u) {
        sum = sum + input[j];
    }
    output[global_i] = sum / f32(period);
}
"#;

// ── GPU uniform struct ─────────────────────────────────────────────────

/// Parameters passed to the GPU shader as a uniform buffer.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuParams {
    len: u32,
    period: u32,
}

/// Symbol offset/length for batch processing.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SymbolInfo {
    offset: u32,
    len: u32,
}

/// Batch params for multi-symbol shaders.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BatchParams {
    num_symbols: u32,
    period: u32,
}

// ── GpuContext ─────────────────────────────────────────────────────────

/// Holds the wgpu device and queue, plus pre-compiled pipelines.
/// Initialized once and cached for the process lifetime.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sma_pipeline: wgpu::ComputePipeline,
    ema_pipeline: wgpu::ComputePipeline,
    rsi_pipeline: wgpu::ComputePipeline,
    atr_pipeline: wgpu::ComputePipeline,
    batch_sma_pipeline: wgpu::ComputePipeline,
    batch_rsi_pipeline: wgpu::ComputePipeline,
    batch_atr_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    adapter_name: String,
}

/// Global GPU context — initialized lazily, cached forever.
static GPU_CONTEXT: OnceCell<Option<GpuContext>> = OnceCell::const_new();

/// Check if a GPU adapter is available without fully initializing.
/// This is a fast check using a synchronous OnceLock.
static GPU_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Cached adapter name from the GPU probe.
static GPU_ADAPTER_NAME: OnceLock<Option<String>> = OnceLock::new();

/// Helper: create a wgpu Instance for headless compute.
fn create_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL,
        flags: wgpu::InstanceFlags::default(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

impl GpuContext {
    /// Create a new GpuContext by requesting a wgpu adapter and device.
    async fn init() -> Option<Self> {
        let instance = create_instance();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let adapter_name = adapter.get_info().name.clone();
        log::info!("wgpu adapter found: {}", adapter_name);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("AuraAlpha Compute Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .ok()?;

        // Compile shader modules.
        let sma_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SMA Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SMA.into()),
        });

        let ema_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("EMA Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_EMA.into()),
        });

        let rsi_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RSI Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_RSI.into()),
        });

        let atr_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ATR Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_ATR.into()),
        });

        let batch_sma_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Batch SMA Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_BATCH_SMA.into()),
        });

        let batch_rsi_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Batch RSI Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_BATCH_RSI.into()),
        });

        let batch_atr_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Batch ATR Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_BATCH_ATR.into()),
        });

        // Create compute pipelines.
        let sma_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SMA Pipeline"),
            layout: None, // Auto layout
            module: &sma_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let ema_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("EMA Pipeline"),
            layout: None,
            module: &ema_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let rsi_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RSI Pipeline"),
            layout: None,
            module: &rsi_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let atr_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ATR Pipeline"),
            layout: None,
            module: &atr_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let batch_sma_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Batch SMA Pipeline"),
            layout: None,
            module: &batch_sma_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let batch_rsi_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Batch RSI Pipeline"),
            layout: None,
            module: &batch_rsi_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let batch_atr_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Batch ATR Pipeline"),
            layout: None,
            module: &batch_atr_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(GpuContext {
            device,
            queue,
            sma_pipeline,
            ema_pipeline,
            rsi_pipeline,
            atr_pipeline,
            batch_sma_pipeline,
            batch_rsi_pipeline,
            batch_atr_pipeline,
            adapter_name,
        })
    }
}

// ── Public API ─────────────────────────────────────────────────────────

/// Get or initialize the global GPU context. Returns None if no GPU available.
pub async fn get_gpu_context() -> Option<&'static GpuContext> {
    let ctx = GPU_CONTEXT
        .get_or_init(|| async { GpuContext::init().await })
        .await;
    ctx.as_ref()
}

/// Check if a GPU adapter is available. Fast, cached, non-blocking after first call.
/// First call may block briefly to probe for an adapter.
pub fn is_gpu_available() -> bool {
    *GPU_AVAILABLE.get_or_init(|| {
        // Quick synchronous check — just see if an adapter exists.
        let instance = create_instance();
        // Use pollster for the one-shot adapter check (synchronous context).
        let adapter_result = pollster_block(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }));
        let available = adapter_result.is_ok();
        if let Ok(adapter) = adapter_result {
            let name = adapter.get_info().name.clone();
            log::info!("GPU available for compute: {}", name);
            // Cache the adapter name while we have it.
            let _ = GPU_ADAPTER_NAME.set(Some(name));
        } else {
            log::info!("No GPU adapter found — all compute will use CPU");
            let _ = GPU_ADAPTER_NAME.set(None);
        }
        available
    })
}

/// Minimal pollster — block on a future. Only used for the one-shot GPU probe.
fn pollster_block<F: std::future::Future>(f: F) -> F::Output {
    // We can't use tokio::block_on here because we might be inside a tokio runtime.
    // Use a simple spin-based executor for this single await.
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::pin::Pin;

    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = unsafe { Pin::new_unchecked(Box::new(f)) };

    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}

/// Get the name of the GPU adapter, if available.
pub fn gpu_adapter_name() -> Option<String> {
    // Ensure the probe has run.
    let _ = is_gpu_available();
    GPU_ADAPTER_NAME.get().and_then(|opt| opt.clone())
}

/// Compute SMA on the GPU. Returns None if GPU is unavailable.
///
/// Input f64 data is converted to f32 for the GPU, then results are
/// converted back to f64. This loses ~1e-7 precision which is acceptable
/// for financial indicators.
pub async fn gpu_compute_sma(data: &[f64], period: usize) -> Option<Vec<f64>> {
    let ctx = get_gpu_context().await?;
    let n = data.len();

    if n == 0 || period == 0 || period > n {
        return Some(vec![f64::NAN; n]);
    }

    // Convert f64 -> f32 for GPU.
    let input_f32: Vec<f32> = data.iter().map(|&x| x as f32).collect();

    let result = run_compute_shader(
        ctx,
        &ctx.sma_pipeline,
        &input_f32,
        GpuParams {
            len: n as u32,
            period: period as u32,
        },
    )
    .await;

    // Convert f32 -> f64.
    Some(result.iter().map(|&x| x as f64).collect())
}

/// Compute EMA on the GPU. Returns None if GPU is unavailable.
pub async fn gpu_compute_ema(data: &[f64], period: usize) -> Option<Vec<f64>> {
    let ctx = get_gpu_context().await?;
    let n = data.len();

    if n == 0 || period == 0 {
        return Some(vec![f64::NAN; n]);
    }

    if n <= period {
        return Some(vec![f64::NAN; n]);
    }

    let input_f32: Vec<f32> = data.iter().map(|&x| x as f32).collect();

    let result = run_compute_shader(
        ctx,
        &ctx.ema_pipeline,
        &input_f32,
        GpuParams {
            len: n as u32,
            period: period as u32,
        },
    )
    .await;

    Some(result.iter().map(|&x| x as f64).collect())
}

/// Batch SMA: compute SMA for multiple symbols' data in one GPU dispatch.
///
/// Takes a slice of (data, period) pairs. Returns a Vec of Vec<f64> results.
/// All symbols must use the same period (GPU uniform limitation).
pub async fn gpu_batch_sma(datasets: &[&[f64]], period: usize) -> Option<Vec<Vec<f64>>> {
    let ctx = get_gpu_context().await?;

    if datasets.is_empty() || period == 0 {
        return Some(Vec::new());
    }

    // Pack all data contiguously.
    let mut packed: Vec<f32> = Vec::new();
    let mut symbol_infos: Vec<SymbolInfo> = Vec::new();

    for data in datasets {
        let offset = packed.len() as u32;
        let len = data.len() as u32;
        symbol_infos.push(SymbolInfo { offset, len });
        packed.extend(data.iter().map(|&x| x as f32));
    }

    if packed.is_empty() {
        return Some(datasets.iter().map(|d| vec![f64::NAN; d.len()]).collect());
    }

    let result = run_batch_sma_shader(
        ctx,
        &packed,
        &symbol_infos,
        BatchParams {
            num_symbols: datasets.len() as u32,
            period: period as u32,
        },
    )
    .await;

    // Unpack results per symbol.
    let mut outputs = Vec::with_capacity(datasets.len());
    for info in &symbol_infos {
        let start = info.offset as usize;
        let end = start + info.len as usize;
        let slice = &result[start..end];
        outputs.push(slice.iter().map(|&x| x as f64).collect());
    }

    Some(outputs)
}

/// Compute RSI on the GPU. Returns None if GPU is unavailable.
///
/// Input: close prices as f64. Output: RSI values [0, 100] as f64.
/// Pre-period values are 50.0 (neutral), matching the CPU implementation.
pub async fn gpu_compute_rsi(closes: &[f64], period: usize) -> Option<Vec<f64>> {
    let ctx = get_gpu_context().await?;
    let n = closes.len();

    if n == 0 || period == 0 {
        return Some(vec![50.0; n]);
    }

    if n <= period + 1 {
        return Some(vec![50.0; n]);
    }

    let input_f32: Vec<f32> = closes.iter().map(|&x| x as f32).collect();

    let result = run_compute_shader(
        ctx,
        &ctx.rsi_pipeline,
        &input_f32,
        GpuParams {
            len: n as u32,
            period: period as u32,
        },
    )
    .await;

    Some(result.iter().map(|&x| x as f64).collect())
}

/// Compute ATR on the GPU. Returns None if GPU is unavailable.
///
/// Input: separate high, low, close arrays (f64). Output: ATR values (f64).
/// Uses a scratch buffer for True Range intermediate values.
/// Leading values are NaN until enough data for the period.
pub async fn gpu_compute_atr(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    period: usize,
) -> Option<Vec<f64>> {
    let ctx = get_gpu_context().await?;
    let n = highs.len();

    if n == 0 || period == 0 || n < period + 1 {
        return Some(vec![f64::NAN; n]);
    }

    // Pack input as: highs[0..n], lows[n..2n], closes[2n..3n].
    let mut packed: Vec<f32> = Vec::with_capacity(3 * n);
    packed.extend(highs.iter().map(|&x| x as f32));
    packed.extend(lows.iter().map(|&x| x as f32));
    packed.extend(closes.iter().map(|&x| x as f32));

    let result = run_atr_shader(
        ctx,
        &ctx.atr_pipeline,
        &packed,
        n,
        GpuParams {
            len: n as u32,
            period: period as u32,
        },
    )
    .await;

    Some(result.iter().map(|&x| x as f64).collect())
}

/// Batch RSI: compute RSI for multiple symbols' close data in one GPU dispatch.
///
/// Takes a slice of close price arrays. All symbols use the same period.
/// Returns a Vec of Vec<f64> RSI results.
pub async fn gpu_batch_rsi(datasets: &[&[f64]], period: usize) -> Option<Vec<Vec<f64>>> {
    let ctx = get_gpu_context().await?;

    if datasets.is_empty() || period == 0 {
        return Some(Vec::new());
    }

    // Pack all close data contiguously.
    let mut packed: Vec<f32> = Vec::new();
    let mut symbol_infos: Vec<SymbolInfo> = Vec::new();

    for data in datasets {
        let offset = packed.len() as u32;
        let len = data.len() as u32;
        symbol_infos.push(SymbolInfo { offset, len });
        packed.extend(data.iter().map(|&x| x as f32));
    }

    if packed.is_empty() {
        return Some(datasets.iter().map(|d| vec![50.0; d.len()]).collect());
    }

    let result = run_batch_sequential_shader(
        ctx,
        &ctx.batch_rsi_pipeline,
        &packed,
        &symbol_infos,
        BatchParams {
            num_symbols: datasets.len() as u32,
            period: period as u32,
        },
        50.0, // default fill value for RSI
    )
    .await;

    // Unpack results per symbol.
    let mut outputs = Vec::with_capacity(datasets.len());
    for info in &symbol_infos {
        let start = info.offset as usize;
        let end = start + info.len as usize;
        let slice = &result[start..end];
        outputs.push(slice.iter().map(|&x| x as f64).collect());
    }

    Some(outputs)
}

/// Batch ATR: compute ATR for multiple symbols in one GPU dispatch.
///
/// Each dataset is a tuple of (highs, lows, closes) for one symbol.
/// All symbols use the same period. Returns Vec of Vec<f64> ATR results.
pub async fn gpu_batch_atr(
    datasets: &[(&[f64], &[f64], &[f64])],
    period: usize,
) -> Option<Vec<Vec<f64>>> {
    let ctx = get_gpu_context().await?;

    if datasets.is_empty() || period == 0 {
        return Some(Vec::new());
    }

    // Pack input: for each symbol, pack high[n], low[n], close[n] contiguously.
    // So symbol i occupies 3*n_i floats in the input buffer.
    let mut packed_input: Vec<f32> = Vec::new();
    let mut symbol_infos: Vec<SymbolInfo> = Vec::new();
    let mut bar_counts: Vec<usize> = Vec::new();

    for &(highs, lows, closes) in datasets {
        let n = highs.len();
        let offset = packed_input.len() as u32;
        // SymbolInfo offset points to start of this symbol's 3*n block,
        // len = number of bars (not total floats).
        symbol_infos.push(SymbolInfo {
            offset,
            len: n as u32,
        });
        bar_counts.push(n);
        packed_input.extend(highs.iter().map(|&x| x as f32));
        packed_input.extend(lows.iter().map(|&x| x as f32));
        packed_input.extend(closes.iter().map(|&x| x as f32));
    }

    if packed_input.is_empty() {
        return Some(datasets.iter().map(|(h, _, _)| vec![f64::NAN; h.len()]).collect());
    }

    // Output buffer: n floats per symbol (ATR values).
    let total_output: usize = bar_counts.iter().sum();

    let result = run_batch_atr_shader(
        ctx,
        &packed_input,
        &symbol_infos,
        total_output,
        BatchParams {
            num_symbols: datasets.len() as u32,
            period: period as u32,
        },
    )
    .await;

    // Unpack results per symbol.
    let mut outputs = Vec::with_capacity(datasets.len());
    let mut out_offset = 0;
    for &n in &bar_counts {
        let slice = &result[out_offset..out_offset + n];
        outputs.push(slice.iter().map(|&x| x as f64).collect());
        out_offset += n;
    }

    Some(outputs)
}

// ── Internal GPU execution ─────────────────────────────────────────────

/// Run a compute shader with input data and params, return output data.
async fn run_compute_shader(
    ctx: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    input: &[f32],
    params: GpuParams,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let n = input.len();

    // Create buffers.
    let input_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Input Buffer"),
            contents: bytemuck::cast_slice(input),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let output_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (n * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Params Buffer"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: (n * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Bind group.
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Compute Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    // Encode and submit.
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        // Dispatch workgroups: ceil(n / 256) for SMA, 1 for EMA.
        let workgroups_x = ((n as u32) + 255) / 256;
        pass.dispatch_workgroups(workgroups_x, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    // Read back results.
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.await;

    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();

    result
}

/// Run the batch SMA shader for multiple symbols.
async fn run_batch_sma_shader(
    ctx: &GpuContext,
    packed_input: &[f32],
    symbol_infos: &[SymbolInfo],
    params: BatchParams,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let n = packed_input.len();

    let input_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch Input"),
            contents: bytemuck::cast_slice(packed_input),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let output_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch Output"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let symbols_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Symbol Infos"),
            contents: bytemuck::cast_slice(symbol_infos),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch Staging"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let bind_group_layout = ctx.batch_sma_pipeline.get_bind_group_layout(0);
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Batch Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: symbols_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Batch Encoder"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Batch Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&ctx.batch_sma_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        // Find the longest symbol to determine X workgroups.
        let max_len = symbol_infos.iter().map(|s| s.len).max().unwrap_or(0);
        let workgroups_x = (max_len + 255) / 256;
        let workgroups_y = params.num_symbols;
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.await;

    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();

    result
}

/// Run the ATR shader which needs an extra scratch buffer for True Range.
/// Input is packed as [highs, lows, closes], output is n floats.
async fn run_atr_shader(
    ctx: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    packed_input: &[f32],
    n: usize,
    params: GpuParams,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let input_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ATR Input"),
            contents: bytemuck::cast_slice(packed_input),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let output_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ATR Output"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ATR Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    // Scratch buffer for True Range intermediate values.
    let scratch_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ATR Scratch"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ATR Staging"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ATR Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: scratch_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ATR Encoder"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("ATR Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.await;

    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();

    result
}

/// Run a batch sequential shader (RSI pattern): each workgroup handles one symbol.
/// Output buffer is same size as input (packed contiguously).
async fn run_batch_sequential_shader(
    ctx: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    packed_input: &[f32],
    symbol_infos: &[SymbolInfo],
    params: BatchParams,
    _default_fill: f32,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let n = packed_input.len();

    let input_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch Seq Input"),
            contents: bytemuck::cast_slice(packed_input),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let output_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch Seq Output"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch Seq Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let symbols_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch Seq Symbols"),
            contents: bytemuck::cast_slice(symbol_infos),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch Seq Staging"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Batch Seq Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: symbols_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Batch Seq Encoder"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Batch Seq Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        // One workgroup per symbol (workgroup_size(1), dispatched along X).
        pass.dispatch_workgroups(params.num_symbols, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, (n * 4) as u64);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.await;

    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();

    result
}

/// Run the batch ATR shader. Input is packed [h0,l0,c0, h1,l1,c1, ...] per symbol.
/// Output buffer is total_output floats (sum of all symbols' bar counts).
async fn run_batch_atr_shader(
    ctx: &GpuContext,
    packed_input: &[f32],
    symbol_infos: &[SymbolInfo],
    total_output: usize,
    params: BatchParams,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let input_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch ATR Input"),
            contents: bytemuck::cast_slice(packed_input),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let output_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch ATR Output"),
        size: (total_output * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let params_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch ATR Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let symbols_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Batch ATR Symbols"),
            contents: bytemuck::cast_slice(symbol_infos),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Batch ATR Staging"),
        size: (total_output * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let bind_group_layout = ctx.batch_atr_pipeline.get_bind_group_layout(0);
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Batch ATR Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: symbols_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Batch ATR Encoder"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Batch ATR Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&ctx.batch_atr_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        // One workgroup per symbol.
        pass.dispatch_workgroups(params.num_symbols, 1, 1);
    }

    encoder.copy_buffer_to_buffer(
        &output_buffer,
        0,
        &staging_buffer,
        0,
        (total_output * 4) as u64,
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    let _ = rx.await;

    let data = buffer_slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();

    result
}

// ── Benchmark utility ──────────────────────────────────────────────────

/// Benchmark result comparing GPU vs CPU for a given operation.
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

/// Run a GPU vs CPU benchmark for SMA computation.
pub async fn benchmark_sma(n: usize, period: usize) -> GpuBenchmarkResult {
    use std::time::Instant;

    // Generate test data: monotonically increasing prices.
    let data: Vec<f64> = (0..n).map(|i| 100.0 + (i as f64) * 0.01).collect();

    // CPU benchmark.
    let cpu_start = Instant::now();
    let cpu_result = super::indicators::compute_sma(&data, period);
    let cpu_time = cpu_start.elapsed();

    // GPU benchmark.
    let gpu_start = Instant::now();
    let gpu_result = gpu_compute_sma(&data, period).await;
    let gpu_time = gpu_start.elapsed();

    let (results_match, max_error, gpu_adapter) = if let Some(ref gpu_res) = gpu_result {
        // Compare results (allowing for f32 precision loss).
        let mut max_err: f64 = 0.0;
        let mut all_match = true;
        for (i, (&cpu, &gpu)) in cpu_result.iter().zip(gpu_res.iter()).enumerate() {
            if cpu.is_nan() && gpu.is_nan() {
                continue;
            }
            if cpu.is_nan() != gpu.is_nan() {
                all_match = false;
                log::warn!("NaN mismatch at index {}: cpu={} gpu={}", i, cpu, gpu);
                break;
            }
            let err = (cpu - gpu).abs();
            max_err = max_err.max(err);
            // f32 precision: ~1e-4 for values around 100-200.
            if err > 0.01 {
                all_match = false;
            }
        }
        let name = gpu_adapter_name().unwrap_or_else(|| "Unknown".to_string());
        (all_match, max_err, name)
    } else {
        (false, f64::NAN, "No GPU".to_string())
    };

    let speedup = if gpu_time.as_micros() > 0 {
        cpu_time.as_micros() as f64 / gpu_time.as_micros() as f64
    } else {
        0.0
    };

    GpuBenchmarkResult {
        operation: "SMA".to_string(),
        data_points: n,
        period,
        cpu_time_us: cpu_time.as_micros() as u64,
        gpu_time_us: gpu_time.as_micros() as u64,
        speedup,
        gpu_adapter,
        results_match,
        max_abs_error: max_error,
    }
}

// ── Tauri IPC commands ─────────────────────────────────────────────────

/// GPU status returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GpuStatus {
    pub available: bool,
    pub adapter_name: String,
    pub backend: String,
    pub threshold_bars: usize,
}

/// IPC command: check GPU compute availability.
#[tauri::command]
pub async fn gpu_compute_status() -> GpuStatus {
    let available = is_gpu_available();
    let adapter_name = gpu_adapter_name().unwrap_or_else(|| "None".to_string());
    let backend = if cfg!(target_os = "windows") {
        "DX12/Vulkan"
    } else if cfg!(target_os = "macos") {
        "Metal"
    } else {
        "Vulkan"
    };

    GpuStatus {
        available,
        adapter_name,
        backend: backend.to_string(),
        threshold_bars: super::indicators::GPU_THRESHOLD,
    }
}

/// IPC command: run a GPU vs CPU benchmark.
///
/// Computes SMA on `data_points` bars with the given `period` and
/// returns timing comparison.
#[tauri::command]
pub async fn gpu_benchmark(data_points: Option<usize>, period: Option<usize>) -> GpuBenchmarkResult {
    let n = data_points.unwrap_or(10_000);
    let p = period.unwrap_or(20);
    benchmark_sma(n, p).await
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_gpu_available_does_not_panic() {
        // This should work even without a GPU — just returns false.
        let _available = is_gpu_available();
    }

    #[test]
    fn test_gpu_params_layout() {
        // Verify the struct is correctly sized for the GPU.
        assert_eq!(std::mem::size_of::<GpuParams>(), 8);
        assert_eq!(std::mem::size_of::<SymbolInfo>(), 8);
        assert_eq!(std::mem::size_of::<BatchParams>(), 8);
    }
}
