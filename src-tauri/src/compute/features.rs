//! Feature extraction — port of execute_feature_extraction (compute_worker.py:791-849).
//!
//! Extracts 10 features per symbol from OHLCV data:
//! RSI14, EMA9, EMA21, ATR14, BB width, returns 1d/5d/20d, volume ratio, close.
//! Uses rayon for parallel symbol processing.
//!
//! Uses the shared `extract_latest_features` from ml_train to avoid
//! duplicating feature extraction logic across modules.

use super::data;
use super::ml_train;
use super::types::*;
use log::info;
use rayon::prelude::*;

/// Execute feature extraction across symbols.
///
/// Port of `execute_feature_extraction` (compute_worker.py:791-849).
/// Extracts 10 features from the latest bar of each symbol.
///
/// Now delegates to the shared `extract_latest_features` function in ml_train,
/// ensuring feature parity between extraction and ML training/prediction.
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

    // Process symbols in parallel using the shared feature extractor.
    let features: Vec<Option<FeatureRow>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 {
                return None;
            }

            let last = bars.len() - 1;

            // Use shared feature extraction (same code as ML training).
            let raw = ml_train::extract_latest_features(&bars)?;

            Some(FeatureRow {
                symbol: sym.clone(),
                date: bars.dates[last].clone(),
                features: FeatureValues {
                    rsi_14: round4(raw[0]),
                    ema_9: round4(raw[1]),
                    ema_21: round4(raw[2]),
                    atr_14: round4(raw[3]),
                    bb_width: round4(raw[4]),
                    returns_1d: round6(raw[5]),
                    returns_5d: round6(raw[6]),
                    returns_20d: round6(raw[7]),
                    volume_ratio: round4(raw[8]),
                    close: round4(raw[9]),
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
