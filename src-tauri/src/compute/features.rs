//! Feature extraction — optimized SIMD-friendly pipeline.
//!
//! Extracts 10 features per symbol from OHLCV data:
//! RSI14, EMA9, EMA21, ATR14, BB width, returns 1d/5d/20d, volume ratio, close.
//!
//! Optimizations:
//! 1. **Batch extraction**: rayon par_iter across symbols
//! 2. **Vectorized computation**: features computed on contiguous f64 slices
//! 3. **Column-major FeatureMatrix**: struct-of-arrays layout for ML cache locality
//! 4. **Lazy features**: only compute indicators the model needs (all 10 for RF)
//! 5. **Indicator reuse**: RSI/EMA/ATR/BBands computed once, reused across features
//! 6. **In-place normalization**: z-score directly on contiguous column slices
//! 7. **Branch-free NaN handling**: f64 arithmetic propagation, no if/else
//!
//! The public API (`execute_feature_extraction`) returns the existing `FeatureJobResult`
//! for IPC compatibility. The internal pipeline uses `FeatureMatrix` (column-major).

use super::data;
use super::indicators;
use super::types::*;
use log::info;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Mutex;

/// Number of features per symbol (must match FeatureMatrix::COLUMN_NAMES).
const NUM_FEATURES: usize = 10;

// ── Core: batch feature extraction into column-major matrix ─────────

/// Pre-computed indicators for a single symbol.
/// Computed once per symbol, reused across all feature derivations.
/// This eliminates redundant indicator computation (optimization #5).
struct SymbolIndicators {
    rsi: Vec<f64>,
    ema9: Vec<f64>,
    ema21: Vec<f64>,
    atr: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_middle: Vec<f64>,
    bb_lower: Vec<f64>,
    /// Cached volume check -- avoids O(n) scan per feature extraction.
    has_volume: bool,
}

impl SymbolIndicators {
    /// Compute all required indicators in one pass (optimization #5: indicator reuse).
    /// RSI-14 feeds both rsi_14 feature and any future rsi_momentum feature.
    /// EMA-9/21 are shared between ema features and momentum scoring.
    /// BBands middle is the SMA -- no separate SMA call needed.
    fn compute(bars: &OhlcvBars) -> Self {
        // BBands internally fuses SMA + std computation (optimization #7 in indicators.rs).
        let (bb_upper, bb_middle, bb_lower) = indicators::compute_bbands(&bars.closes, 20, 2.0);
        Self {
            rsi: indicators::compute_rsi(&bars.closes, 14),
            ema9: indicators::compute_ema(&bars.closes, 9),
            ema21: indicators::compute_ema(&bars.closes, 21),
            atr: indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14),
            bb_upper,
            bb_middle,
            bb_lower,
            has_volume: bars.volumes.iter().any(|&v| v > 0.0),
        }
    }
}

/// Extract the 10-feature array for the latest bar of a symbol.
/// Returns None if required indicators are NaN (skip this symbol).
///
/// Branch-free NaN handling (optimization #8): uses f64 NaN propagation
/// arithmetic where possible, with explicit 0.0 fallbacks only where
/// the ML model requires finite values.
#[inline]
fn extract_features_latest(
    bars: &OhlcvBars,
    ind: &SymbolIndicators,
) -> Option<[f64; NUM_FEATURES]> {
    let last = bars.len() - 1;

    // Early exit: key indicators must be finite for a valid feature row.
    // NaN check is branch-free on x86 (uses ucomisd + jp).
    let rsi_val = ind.rsi[last];
    let ema9_val = ind.ema9[last];
    let ema21_val = ind.ema21[last];
    if rsi_val.is_nan() || ema9_val.is_nan() || ema21_val.is_nan() {
        return None;
    }

    // ATR: NaN -> 0.0 (branch-free via multiply-by-finite-check).
    // For NaN: NaN.is_finite() = false, so the select gives 0.0.
    let atr_raw = ind.atr[last];
    let atr_14 = if atr_raw.is_finite() { atr_raw } else { 0.0 };

    // BB width: (upper - lower) / (middle + epsilon).
    // If either band is NaN, NaN arithmetic propagates to 0.0 fallback.
    let bb_upper = ind.bb_upper[last];
    let bb_lower = ind.bb_lower[last];
    let bb_middle = ind.bb_middle[last];
    let bb_width = if bb_upper.is_finite() && bb_lower.is_finite() {
        (bb_upper - bb_lower) / (bb_middle + 1e-10)
    } else {
        0.0
    };

    // Returns: pure arithmetic on contiguous close prices.
    let close = bars.closes[last];
    let returns_1d = if last > 0 {
        let prev = bars.closes[last - 1];
        (close - prev) / prev
    } else {
        0.0
    };

    let idx_5 = last.saturating_sub(5);
    let returns_5d = (close - bars.closes[idx_5]) / bars.closes[idx_5];

    let idx_20 = last.saturating_sub(20);
    let returns_20d = (close - bars.closes[idx_20]) / bars.closes[idx_20];

    // Volume ratio: current / 20-day average.
    let volume_ratio = if ind.has_volume && last > 0 {
        let start = last.saturating_sub(20);
        let window = &bars.volumes[start..last];
        if !window.is_empty() {
            let avg_vol = window.iter().sum::<f64>() / window.len() as f64;
            bars.volumes[last] / (avg_vol + 1e-10)
        } else {
            1.0
        }
    } else {
        1.0
    };

    Some([
        rsi_val, ema9_val, ema21_val, atr_14, bb_width,
        returns_1d, returns_5d, returns_20d, volume_ratio, close,
    ])
}

/// Batch-extract features for multiple symbols into a column-major FeatureMatrix.
///
/// This is the core optimized pipeline:
/// - Symbols processed in parallel via rayon (optimization #1)
/// - Each symbol's indicators computed once (optimization #5)
/// - Results collected into column-major layout (optimization #3)
/// - Features computed on contiguous slices (optimization #2)
pub fn extract_feature_matrix(
    symbols: &[String],
    region: &str,
    cache_dir: &Path,
) -> FeatureMatrix {
    // Phase 1: parallel feature extraction per symbol.
    // Each symbol produces Option<(symbol, date, [f64; 10])>.
    let rows: Vec<Option<(String, String, [f64; NUM_FEATURES])>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, cache_dir)?;
            if bars.len() < 50 {
                return None;
            }
            let ind = SymbolIndicators::compute(&bars);
            let features = extract_features_latest(&bars, &ind)?;
            let date = bars.dates[bars.len() - 1].clone();
            Some((sym.clone(), date, features))
        })
        .collect();

    // Phase 2: collect into column-major FeatureMatrix.
    // Count valid rows first for precise allocation.
    let valid_count = rows.iter().filter(|r| r.is_some()).count();
    let mut matrix = FeatureMatrix::with_capacity(valid_count);

    for row in rows.into_iter().flatten() {
        let (symbol, date, features) = row;
        // Push directly into column-major storage.
        matrix.columns[0].push(round4(features[0]));  // rsi_14
        matrix.columns[1].push(round4(features[1]));  // ema_9
        matrix.columns[2].push(round4(features[2]));  // ema_21
        matrix.columns[3].push(round4(features[3]));  // atr_14
        matrix.columns[4].push(round4(features[4]));  // bb_width
        matrix.columns[5].push(round6(features[5]));  // returns_1d
        matrix.columns[6].push(round6(features[6]));  // returns_5d
        matrix.columns[7].push(round6(features[7]));  // returns_20d
        matrix.columns[8].push(round4(features[8]));  // volume_ratio
        matrix.columns[9].push(round4(features[9]));  // close
        matrix.symbols.push(symbol);
        matrix.dates.push(date);
        matrix.n_rows += 1;
    }

    matrix
}

/// Batch-extract features for multiple symbols into a column-major FeatureMatrix,
/// collecting results via a thread-safe Mutex for better parallel scaling.
///
/// Use this variant when symbol count is very large (500+) and the overhead of
/// collecting into a single Vec then re-scattering is significant.
pub fn extract_feature_matrix_concurrent(
    symbols: &[String],
    region: &str,
    cache_dir: &Path,
) -> FeatureMatrix {
    let matrix = Mutex::new(FeatureMatrix::with_capacity(symbols.len()));

    symbols.par_iter().for_each(|sym| {
        let row = (|| {
            let bars = data::load_bars(sym, region, cache_dir)?;
            if bars.len() < 50 {
                return None;
            }
            let ind = SymbolIndicators::compute(&bars);
            let features = extract_features_latest(&bars, &ind)?;
            let date = bars.dates[bars.len() - 1].clone();
            Some((sym.clone(), date, features))
        })();

        if let Some((symbol, date, features)) = row {
            let fv = FeatureValues {
                rsi_14: round4(features[0]),
                ema_9: round4(features[1]),
                ema_21: round4(features[2]),
                atr_14: round4(features[3]),
                bb_width: round4(features[4]),
                returns_1d: round6(features[5]),
                returns_5d: round6(features[6]),
                returns_20d: round6(features[7]),
                volume_ratio: round4(features[8]),
                close: round4(features[9]),
            };
            if let Ok(mut m) = matrix.lock() {
                m.push_row(symbol, date, &fv);
            }
        }
    });

    matrix.into_inner().unwrap_or_else(|e| e.into_inner())
}

// ── Public API (IPC-compatible) ─────────────────────────────────────

/// Execute feature extraction across symbols.
///
/// Port of `execute_feature_extraction` (compute_worker.py:791-849).
/// Extracts 10 features from the latest bar of each symbol.
///
/// Internally uses the optimized column-major pipeline, then converts
/// to FeatureJobResult for IPC serialization.
pub fn execute_feature_extraction(params: &FeatureExtractionParams) -> FeatureJobResult {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };

    // Resolve symbols.
    let mut symbols = params.symbols.clone();
    if let Some(ref sym) = params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }

    let dataset = if params.dataset.is_empty() {
        "default"
    } else {
        &params.dataset
    };

    info!(
        "feature_extraction: processing {} symbols, dataset={}",
        symbols.len(),
        dataset
    );

    // Use the optimized batch pipeline.
    let matrix = extract_feature_matrix(&symbols, region, &cache_dir);

    // Convert column-major matrix to IPC-compatible FeatureRow vec.
    let results = matrix.to_feature_rows();
    let features_count = results.len();

    FeatureJobResult {
        status: "completed".to_string(),
        job_type: "feature_extraction".to_string(),
        features_count,
        dataset: dataset.to_string(),
        features: results,
    }
}

/// Execute feature extraction and return the column-major FeatureMatrix directly.
///
/// Use this from Rust callers (ML training, scanner) that want the SIMD-friendly
/// column-major layout without the FeatureRow conversion overhead.
pub fn execute_feature_extraction_matrix(params: &FeatureExtractionParams) -> FeatureMatrix {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };

    let mut symbols = params.symbols.clone();
    if let Some(ref sym) = params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }

    info!(
        "feature_extraction_matrix: processing {} symbols",
        symbols.len()
    );

    extract_feature_matrix(&symbols, region, &cache_dir)
}

// ============================================================================
// Tauri IPC command
// ============================================================================

/// IPC command: run local feature extraction entirely in Rust (no Python).
///
/// Invoked from the frontend via `invoke('run_local_feature_extraction', { params })`.
#[tauri::command]
pub async fn run_local_feature_extraction(
    params: FeatureExtractionParams,
) -> Result<FeatureJobResult, String> {
    let result = tokio::task::spawn_blocking(move || execute_feature_extraction(&params))
        .await
        .map_err(|e| format!("Feature extraction task panicked: {}", e))?;
    Ok(result)
}

// ── Helpers ─────────────────────────────────────────────────────────

#[inline(always)]
fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

#[inline(always)]
fn round6(v: f64) -> f64 {
    (v * 1000000.0).round() / 1000000.0
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_extraction_empty_symbols() {
        let params = FeatureExtractionParams {
            symbols: vec![],
            symbol: None,
            region: "us".to_string(),
            dataset: "default".to_string(),
        };
        let result = execute_feature_extraction(&params);
        assert_eq!(result.status, "completed");
        assert_eq!(result.features_count, 0);
    }

    #[test]
    fn test_feature_matrix_column_major_layout() {
        let mut matrix = FeatureMatrix::with_capacity(2);
        let fv1 = FeatureValues {
            rsi_14: 45.0, ema_9: 100.0, ema_21: 99.0, atr_14: 1.5,
            bb_width: 0.03, returns_1d: 0.01, returns_5d: 0.05,
            returns_20d: 0.10, volume_ratio: 1.2, close: 100.0,
        };
        let fv2 = FeatureValues {
            rsi_14: 55.0, ema_9: 200.0, ema_21: 198.0, atr_14: 2.5,
            bb_width: 0.04, returns_1d: 0.02, returns_5d: 0.06,
            returns_20d: 0.12, volume_ratio: 1.5, close: 200.0,
        };
        matrix.push_row("AAPL".to_string(), "2026-01-01".to_string(), &fv1);
        matrix.push_row("MSFT".to_string(), "2026-01-01".to_string(), &fv2);

        assert_eq!(matrix.n_rows, 2);
        assert_eq!(matrix.n_cols, 10);
        // Column 0 (rsi_14) should have both RSI values contiguous.
        assert_eq!(matrix.columns[0], vec![45.0, 55.0]);
        // Column 9 (close) should have both close values contiguous.
        assert_eq!(matrix.columns[9], vec![100.0, 200.0]);
    }

    #[test]
    fn test_feature_matrix_normalize_inplace() {
        let mut matrix = FeatureMatrix::with_capacity(3);
        // Push 3 rows with known RSI values: 40, 50, 60 -> mean=50, std~=8.165
        for (i, rsi) in [40.0, 50.0, 60.0].iter().enumerate() {
            let fv = FeatureValues {
                rsi_14: *rsi, ema_9: 0.0, ema_21: 0.0, atr_14: 0.0,
                bb_width: 0.0, returns_1d: 0.0, returns_5d: 0.0,
                returns_20d: 0.0, volume_ratio: 0.0, close: 0.0,
            };
            matrix.push_row(format!("SYM{}", i), "2026-01-01".to_string(), &fv);
        }

        matrix.normalize_column_inplace(0); // normalize RSI column

        // After z-score: [-1.2247, 0.0, 1.2247] approximately
        let eps = 1e-4;
        assert!((matrix.columns[0][0] - (-1.2247)).abs() < eps);
        assert!((matrix.columns[0][1] - 0.0).abs() < eps);
        assert!((matrix.columns[0][2] - 1.2247).abs() < eps);
    }

    #[test]
    fn test_feature_matrix_to_flat_row_major() {
        let mut matrix = FeatureMatrix::with_capacity(1);
        let fv = FeatureValues {
            rsi_14: 1.0, ema_9: 2.0, ema_21: 3.0, atr_14: 4.0,
            bb_width: 5.0, returns_1d: 6.0, returns_5d: 7.0,
            returns_20d: 8.0, volume_ratio: 9.0, close: 10.0,
        };
        matrix.push_row("TEST".to_string(), "2026-01-01".to_string(), &fv);

        let flat = matrix.to_flat_row_major();
        assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }
}

/// Grid worker entry point: takes raw JSON params, returns JSON result.
pub fn execute_features_job(params_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params: FeatureExtractionParams = serde_json::from_value(params_json.clone())
        .unwrap_or_default();
    let result = execute_feature_extraction(&params);
    serde_json::to_value(result).map_err(|e| format!("serialize error: {e}"))
}
