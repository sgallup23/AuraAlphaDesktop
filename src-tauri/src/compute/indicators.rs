//! Technical indicator functions — direct ports from compute_worker.py lines 89-174.
//!
//! Every function matches the Python implementation exactly:
//! same algorithm, same edge-case handling, same NaN propagation.
//! Uses `f64::NAN` where Python uses `np.nan`.
//!
//! GPU-accelerated paths are available for SMA and EMA when:
//! 1. A GPU adapter is detected (Vulkan/Metal/DX12)
//! 2. The dataset exceeds 1000 bars (amortizes GPU overhead)
//! Use `compute_sma_auto` / `compute_ema_auto` for the adaptive path.

/// Minimum data length to prefer GPU over CPU.
/// Below this threshold, buffer upload/download overhead dominates.
pub const GPU_THRESHOLD: usize = 1000;

/// Compute Average True Range (ATR).
///
/// Port of `_compute_atr` (compute_worker.py:89-103).
/// Returns a Vec of length `highs.len()` with leading NaN values.
pub fn compute_atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let n = highs.len();
    if n < period + 1 {
        return vec![f64::NAN; n];
    }

    // True Range: starts at index 1 (needs previous close).
    // tr[i] corresponds to bar index i+1.
    let tr: Vec<f64> = (1..n)
        .map(|i| {
            let hl = highs[i] - lows[i];
            let hc = (highs[i] - closes[i - 1]).abs();
            let lc = (lows[i] - closes[i - 1]).abs();
            hl.max(hc.max(lc))
        })
        .collect();

    let mut atr = vec![f64::NAN; n];

    if tr.len() >= period {
        // Initial ATR = simple mean of first `period` TR values.
        let initial: f64 = tr[..period].iter().sum::<f64>() / period as f64;
        atr[period] = initial;

        // Wilder smoothing from there.
        for i in (period + 1)..=tr.len() {
            atr[i] = (atr[i - 1] * (period as f64 - 1.0) + tr[i - 1]) / period as f64;
        }
    }

    atr
}

/// Compute Exponential Moving Average (EMA).
///
/// Port of `_compute_ema` (compute_worker.py:106-116).
/// Seeds with SMA of the first `period` values, then applies
/// the standard EMA formula with alpha = 2 / (period + 1).
pub fn compute_ema(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut ema = vec![f64::NAN; n];

    if n <= period {
        return ema;
    }

    let alpha = 2.0 / (period as f64 + 1.0);

    // Seed: SMA of data[0..period]
    let seed: f64 = data[..period].iter().sum::<f64>() / period as f64;
    ema[period - 1] = seed;

    for i in period..n {
        ema[i] = data[i] * alpha + ema[i - 1] * (1.0 - alpha);
    }

    ema
}

/// Compute Relative Strength Index (RSI).
///
/// Port of `_compute_rsi` (compute_worker.py:119-135).
/// Returns RSI values in [0, 100]. Fills initial values with 50.0
/// (matching the Python default).
pub fn compute_rsi(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut rsi = vec![50.0; n];

    if n <= period + 1 {
        return rsi;
    }

    // Price deltas (diff).
    let deltas: Vec<f64> = (1..n).map(|i| closes[i] - closes[i - 1]).collect();
    let gains: Vec<f64> = deltas.iter().map(|&d| if d > 0.0 { d } else { 0.0 }).collect();
    let losses: Vec<f64> = deltas.iter().map(|&d| if d < 0.0 { -d } else { 0.0 }).collect();

    // Initial averages: simple mean of first `period` values.
    let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

    // Wilder smoothing from index `period` onward.
    for i in period..deltas.len() {
        avg_gain = (avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;
        let rs = avg_gain / (avg_loss + 1e-10);
        rsi[i + 1] = 100.0 - (100.0 / (1.0 + rs));
    }

    rsi
}

/// Compute Bollinger Bands (upper, middle, lower).
///
/// Port of `_compute_bbands` (compute_worker.py:138-151).
/// Returns a tuple of three Vecs, each of length `closes.len()`.
/// Uses population std (ddof=0), matching `np.std()` default.
pub fn compute_bbands(
    closes: &[f64],
    period: usize,
    std_mult: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = closes.len();
    let mut upper = vec![f64::NAN; n];
    let mut middle = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];

    for i in (period - 1)..n {
        let window = &closes[(i + 1 - period)..=i];
        let mean: f64 = window.iter().sum::<f64>() / period as f64;

        // Population standard deviation (ddof=0), matching np.std() default.
        let variance: f64 =
            window.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        middle[i] = mean;
        upper[i] = mean + std_mult * std_dev;
        lower[i] = mean - std_mult * std_dev;
    }

    (upper, middle, lower)
}

/// Compute Simple Moving Average (SMA).
///
/// Port of `_compute_sma` (compute_worker.py:154-160).
pub fn compute_sma(data: &[f64], period: usize) -> Vec<f64> {
    let n = data.len();
    let mut sma = vec![f64::NAN; n];

    for i in (period - 1)..n {
        let window = &data[(i + 1 - period)..=i];
        sma[i] = window.iter().sum::<f64>() / period as f64;
    }

    sma
}

/// Compute On-Balance Volume (OBV).
///
/// Port of `_compute_obv` (compute_worker.py:163-174).
/// Returns a Vec of cumulative volume direction values.
pub fn compute_obv(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let n = closes.len();
    let mut obv = vec![0.0; n];

    for i in 1..n {
        if closes[i] > closes[i - 1] {
            obv[i] = obv[i - 1] + volumes[i];
        } else if closes[i] < closes[i - 1] {
            obv[i] = obv[i - 1] - volumes[i];
        } else {
            obv[i] = obv[i - 1];
        }
    }

    obv
}

// ── GPU-accelerated indicator wrappers ────────────────────────────────

/// Compute SMA with automatic GPU/CPU selection.
///
/// Uses GPU when available AND data length exceeds `GPU_THRESHOLD`.
/// Falls back to CPU otherwise. The GPU path uses f32 internally
/// (WGSL limitation) so results may differ by ~1e-4 from f64 CPU path.
pub async fn compute_sma_auto(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() >= GPU_THRESHOLD && super::gpu::is_gpu_available() {
        if let Some(result) = super::gpu::gpu_compute_sma(data, period).await {
            log::debug!("SMA computed on GPU ({} bars, period {})", data.len(), period);
            return result;
        }
    }
    compute_sma(data, period)
}

/// Compute EMA with automatic GPU/CPU selection.
///
/// Note: EMA is sequential, so GPU provides less speedup than SMA.
/// The GPU path is mainly useful when batching many symbols together.
pub async fn compute_ema_auto(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() >= GPU_THRESHOLD && super::gpu::is_gpu_available() {
        if let Some(result) = super::gpu::gpu_compute_ema(data, period).await {
            log::debug!("EMA computed on GPU ({} bars, period {})", data.len(), period);
            return result;
        }
    }
    compute_ema(data, period)
}

// ── Unit tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        if a.is_nan() && b.is_nan() {
            return true;
        }
        (a - b).abs() < eps
    }

    fn assert_vec_approx(actual: &[f64], expected: &[f64], eps: f64, label: &str) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "{}: length mismatch {} vs {}",
            label,
            actual.len(),
            expected.len()
        );
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                approx_eq(a, e, eps),
                "{} at index {}: got {} expected {}",
                label,
                i,
                a,
                e
            );
        }
    }

    #[test]
    fn test_sma_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let sma = compute_sma(&data, 3);
        assert_eq!(sma.len(), 5);
        assert!(sma[0].is_nan());
        assert!(sma[1].is_nan());
        assert!(approx_eq(sma[2], 2.0, 1e-9));
        assert!(approx_eq(sma[3], 3.0, 1e-9));
        assert!(approx_eq(sma[4], 4.0, 1e-9));
    }

    #[test]
    fn test_ema_basic() {
        let data = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        let ema = compute_ema(&data, 3);
        assert_eq!(ema.len(), 6);
        assert!(ema[0].is_nan());
        assert!(ema[1].is_nan());
        // Seed = mean(10, 11, 12) = 11.0
        assert!(approx_eq(ema[2], 11.0, 1e-9));
        // alpha = 2/4 = 0.5
        // ema[3] = 13 * 0.5 + 11 * 0.5 = 12.0
        assert!(approx_eq(ema[3], 12.0, 1e-9));
    }

    #[test]
    fn test_rsi_small_input() {
        let closes = vec![44.0, 44.5, 45.0];
        let rsi = compute_rsi(&closes, 14);
        // n <= period + 1, so all 50.0
        assert_vec_approx(&rsi, &vec![50.0; 3], 1e-9, "rsi_small");
    }

    #[test]
    fn test_obv_basic() {
        let closes = vec![10.0, 11.0, 10.5, 10.5, 12.0];
        let volumes = vec![100.0, 200.0, 150.0, 100.0, 300.0];
        let obv = compute_obv(&closes, &volumes);
        // obv[0] = 0
        // obv[1] = 0 + 200 = 200 (close up)
        // obv[2] = 200 - 150 = 50 (close down)
        // obv[3] = 50 (equal)
        // obv[4] = 50 + 300 = 350 (close up)
        let expected = vec![0.0, 200.0, 50.0, 50.0, 350.0];
        assert_vec_approx(&obv, &expected, 1e-9, "obv");
    }

    #[test]
    fn test_bbands_basic() {
        let closes = vec![20.0, 21.0, 22.0, 21.0, 20.0];
        let (upper, middle, lower) = compute_bbands(&closes, 3, 2.0);
        assert_eq!(upper.len(), 5);
        // First two are NaN.
        assert!(upper[0].is_nan());
        assert!(upper[1].is_nan());
        // middle[2] = mean(20, 21, 22) = 21.0
        assert!(approx_eq(middle[2], 21.0, 1e-9));
    }

    #[test]
    fn test_atr_short_input() {
        let highs = vec![11.0, 12.0];
        let lows = vec![9.0, 10.0];
        let closes = vec![10.0, 11.0];
        let atr = compute_atr(&highs, &lows, &closes, 14);
        // n < period + 1, all NaN.
        assert!(atr.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn test_atr_basic() {
        // 5 bars, period 3 → first ATR at index 3.
        let highs = vec![12.0, 13.0, 14.0, 13.5, 14.5];
        let lows = vec![10.0, 11.0, 12.0, 11.5, 12.5];
        let closes = vec![11.0, 12.0, 13.0, 12.0, 14.0];
        let atr = compute_atr(&highs, &lows, &closes, 3);
        assert_eq!(atr.len(), 5);
        assert!(atr[0].is_nan());
        assert!(atr[1].is_nan());
        assert!(atr[2].is_nan());
        // tr[0] = max(13-11, |13-11|, |11-11|) = max(2, 2, 0) = 2
        // tr[1] = max(14-12, |14-12|, |12-12|) = max(2, 2, 0) = 2
        // tr[2] = max(13.5-11.5, |13.5-13|, |11.5-13|) = max(2, 0.5, 1.5) = 2
        // atr[3] = mean(2, 2, 2) = 2.0
        assert!(approx_eq(atr[3], 2.0, 1e-9));
    }
}
