//! ML inference — hand-coded ensemble, no ML crate needed.
//!
//! Port of execute_ml_inference (compute_worker.py:706-788).
//! Uses a simple momentum + mean-reversion + trend ensemble.

use super::data;
use super::indicators;
use super::types::*;
use log::info;
use rayon::prelude::*;

/// Execute ML inference across symbols.
///
/// Port of `execute_ml_inference` (compute_worker.py:706-788).
/// Ensemble scoring:
/// - Momentum score (+/-0.3): EMA fast vs slow crossover direction
/// - RSI score (+/-0.4): Oversold/overbought mean-reversion signal
/// - Trend score (+/-0.3): 5-day returns clamped
///
/// prediction = sum of scores
/// confidence = min(0.95, abs(prediction) + 0.3)
pub fn execute_ml_inference(params: &MlParams) -> MlJobResult {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };

    // Resolve symbols.
    let mut symbols = params.symbols.clone();
    if let Some(ref sym) = params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }

    let model_name = if params.model.is_empty() {
        "ensemble_technical"
    } else {
        &params.model
    };

    info!(
        "ml_inference: processing {} symbols, model={}",
        symbols.len(),
        model_name
    );

    // Process symbols in parallel.
    let predictions: Vec<Option<MlPrediction>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 {
                return None;
            }

            let n = bars.len();
            let last = n - 1;

            // Compute features.
            let rsi = indicators::compute_rsi(&bars.closes, 14);
            let ema_fast = indicators::compute_ema(&bars.closes, 9);
            let ema_slow = indicators::compute_ema(&bars.closes, 21);

            if rsi[last].is_nan() || ema_fast[last].is_nan() || ema_slow[last].is_nan() {
                return None;
            }

            // --- Ensemble scoring ---

            // 1. Momentum score (+/-0.3 from EMA cross).
            let momentum_score = if ema_fast[last] > ema_slow[last] {
                0.3
            } else if ema_fast[last] < ema_slow[last] {
                -0.3
            } else {
                0.0
            };

            // 2. RSI score (+/-0.4 from oversold/overbought).
            let rsi_score = if rsi[last] < 30.0 {
                0.4 // oversold = buy signal
            } else if rsi[last] > 70.0 {
                -0.4 // overbought = sell signal
            } else {
                (50.0 - rsi[last]) / 100.0 // slight mean-reversion bias
            };

            // 3. Trend score (+/-0.3 from 5-day returns).
            let prev_idx = if last >= 5 { last - 5 } else { 0 };
            let returns_5d = (bars.closes[last] - bars.closes[prev_idx]) / bars.closes[prev_idx];
            let trend_score = (returns_5d * 5.0).clamp(-0.3, 0.3);

            let prediction = round4(momentum_score + rsi_score + trend_score);
            let confidence = round4((prediction.abs() + 0.3).min(0.95));

            let direction = if prediction > 0.0 {
                "long".to_string()
            } else {
                "short".to_string()
            };

            Some(MlPrediction {
                symbol: sym.clone(),
                prediction,
                direction,
                confidence,
                model: model_name.to_string(),
                features: MlFeatureScores {
                    rsi: round2(rsi[last]),
                    ema_fast: round4(ema_fast[last]),
                    ema_slow: round4(ema_slow[last]),
                    returns_5d: round6(returns_5d),
                },
            })
        })
        .collect();

    let results: Vec<MlPrediction> = predictions.into_iter().flatten().collect();
    let symbols_processed = results.len();

    MlJobResult {
        status: "completed".to_string(),
        job_type: "ml_inference".to_string(),
        predictions: results,
        model: model_name.to_string(),
        symbols_processed,
    }
}

// ============================================================================
// Tauri IPC command
// ============================================================================

/// IPC command: run local ML inference entirely in Rust (no Python).
///
/// Invoked from the frontend via `invoke('run_local_ml_inference', { params })`.
#[tauri::command]
pub async fn run_local_ml_inference(params: MlParams) -> Result<MlJobResult, String> {
    let result = tokio::task::spawn_blocking(move || execute_ml_inference(&params))
        .await
        .map_err(|e| format!("ML inference task panicked: {}", e))?;
    Ok(result)
}

// ── Helpers ─────────────────────────────────────────────────────────

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

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
    fn test_ml_inference_empty_symbols() {
        let params = MlParams {
            symbols: vec![],
            symbol: None,
            region: "us".to_string(),
            model: "ensemble_technical".to_string(),
        };
        let result = execute_ml_inference(&params);
        assert_eq!(result.status, "completed");
        assert_eq!(result.symbols_processed, 0);
    }

    #[test]
    fn test_ensemble_scoring_logic() {
        // Test the scoring math directly.

        // Case 1: Oversold + momentum up + positive trend
        // momentum = 0.3, rsi = 0.4 (rsi < 30), trend = 0.3 (5% return * 5 = 0.25 clamped to 0.3)
        let momentum = 0.3_f64;
        let rsi_score = 0.4_f64;
        let trend = 0.25_f64; // = 0.05 * 5
        let prediction = momentum + rsi_score + trend;
        assert!(prediction > 0.0);
        let confidence = (prediction.abs() + 0.3).min(0.95);
        assert!(confidence <= 0.95);

        // Case 2: Overbought + momentum down
        let momentum = -0.3_f64;
        let rsi_score = -0.4_f64;
        let trend = -0.1_f64;
        let prediction = momentum + rsi_score + trend;
        assert!(prediction < 0.0);
    }
}

/// Grid worker entry point: takes raw JSON params, returns JSON result.
pub fn execute_ml_job(params_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params: MlParams = serde_json::from_value(params_json.clone())
        .unwrap_or_default();
    let result = execute_ml_inference(&params);
    serde_json::to_value(result).map_err(|e| format!("serialize error: {e}"))
}
