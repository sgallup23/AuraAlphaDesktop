//! Feature extraction — port of execute_feature_extraction (compute_worker.py:791-849).
//!
//! Extracts 11 features per symbol from OHLCV data:
//! RSI14, EMA9, EMA21, ATR14, BB width, returns 1d/5d/20d, volume ratio, close.
//! Uses rayon for parallel symbol processing.

use super::data;
use super::indicators;
use super::types::*;
use log::info;
use rayon::prelude::*;

/// Execute feature extraction across symbols.
///
/// Port of `execute_feature_extraction` (compute_worker.py:791-849).
/// Extracts 11 features from the latest bar of each symbol.
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

    // Process symbols in parallel.
    let features: Vec<Option<FeatureRow>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 {
                return None;
            }

            let n = bars.len();
            let last = n - 1;

            // Compute indicators.
            let rsi = indicators::compute_rsi(&bars.closes, 14);
            let ema9 = indicators::compute_ema(&bars.closes, 9);
            let ema21 = indicators::compute_ema(&bars.closes, 21);
            let atr = indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14);
            let (bb_u, bb_m, bb_l) = indicators::compute_bbands(&bars.closes, 20, 2.0);

            // Skip if key indicators are NaN.
            if rsi[last].is_nan() || ema9[last].is_nan() {
                return None;
            }

            // ATR (handle NaN).
            let atr_14 = if atr[last].is_nan() { 0.0 } else { round4(atr[last]) };

            // Bollinger Band width.
            let bb_width = if !bb_u[last].is_nan() && !bb_l[last].is_nan() {
                round4((bb_u[last] - bb_l[last]) / (bb_m[last] + 1e-10))
            } else {
                0.0
            };

            // Returns.
            let returns_1d = if last > 0 {
                round6((bars.closes[last] - bars.closes[last - 1]) / bars.closes[last - 1])
            } else {
                0.0
            };

            let idx_5 = if last >= 5 { last - 5 } else { 0 };
            let returns_5d = round6(
                (bars.closes[last] - bars.closes[idx_5]) / bars.closes[idx_5],
            );

            let idx_20 = if last >= 20 { last - 20 } else { 0 };
            let returns_20d = round6(
                (bars.closes[last] - bars.closes[idx_20]) / bars.closes[idx_20],
            );

            // Volume ratio (current vs 20-day avg).
            let volume_ratio = if bars.volumes.iter().any(|&v| v > 0.0) {
                let start = if last >= 20 { last - 20 } else { 0 };
                let window = &bars.volumes[start..last];
                if !window.is_empty() {
                    let avg_vol = window.iter().sum::<f64>() / window.len() as f64;
                    round4(bars.volumes[last] / (avg_vol + 1e-10))
                } else {
                    1.0
                }
            } else {
                1.0
            };

            Some(FeatureRow {
                symbol: sym.clone(),
                date: bars.dates[last].clone(),
                features: FeatureValues {
                    rsi_14: round4(rsi[last]),
                    ema_9: round4(ema9[last]),
                    ema_21: round4(ema21[last]),
                    atr_14,
                    bb_width,
                    returns_1d,
                    returns_5d,
                    returns_20d,
                    volume_ratio,
                    close: round4(bars.closes[last]),
                },
            })
        })
        .collect();

    let results: Vec<FeatureRow> = features.into_iter().flatten().collect();
    let features_count = results.len();

    FeatureJobResult {
        status: "completed".to_string(),
        job_type: "feature_extraction".to_string(),
        features_count,
        dataset: dataset.to_string(),
        features: results,
    }
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

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn round6(v: f64) -> f64 {
    (v * 1000000.0).round() / 1000000.0
}

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
}

/// Grid worker entry point: takes raw JSON params, returns JSON result.
pub fn execute_features_job(params_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params: FeatureExtractionParams = serde_json::from_value(params_json.clone())
        .unwrap_or_default();
    let result = execute_feature_extraction(&params);
    serde_json::to_value(result).map_err(|e| format!("serialize error: {e}"))
}
