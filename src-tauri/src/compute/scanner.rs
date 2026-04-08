//! Signal scanner — port of execute_scan (compute_worker.py:622-703).
//!
//! Checks the latest bar of each symbol for entry conditions.
//! Supports scan types: momentum, reversal, breakout, volume.
//! Uses rayon for parallel scanning across symbols.
//!
//! ML-enhanced: when a trained RF model is available, the scanner
//! incorporates the model's prediction into the confidence score,
//! blending rule-based signals with ML predictions.

use super::backtest::{self, IndicatorSet};
use super::data;
use super::indicators;
use super::ml_train;
use super::types::*;
use log::info;
use rayon::prelude::*;

/// Map a scan type name to its default entry logic conditions.
fn scan_type_to_logic(scan_type: &str) -> Vec<String> {
    match scan_type {
        "momentum" => vec![
            "ema_cross_up".to_string(),
            "rsi_above_threshold".to_string(),
            "volume_surge".to_string(),
        ],
        "reversal" => vec![
            "rsi_oversold".to_string(),
            "price_below_lower_band".to_string(),
            "volume_spike".to_string(),
        ],
        "breakout" => vec![
            "price_above_high".to_string(),
            "volume_breakout".to_string(),
            "band_expansion".to_string(),
        ],
        "volume" => vec![
            "volume_surge".to_string(),
            "obv_rising".to_string(),
            "volume_breakout".to_string(),
        ],
        _ => vec![
            "ema_cross_up".to_string(),
            "rsi_above_threshold".to_string(),
            "volume_surge".to_string(),
        ],
    }
}

/// Build a `BacktestParams` suitable for scanner entry checking.
fn scanner_params(scan_params: &ScanParams) -> BacktestParams {
    BacktestParams {
        strategy_family: String::new(),
        symbol_universe: vec![],
        symbol: None,
        region: scan_params.region.clone(),
        direction: "long".to_string(),
        date_start: String::new(),
        date_end: String::new(),
        entry_logic: vec![],
        stop_loss_atr_mult: 2.0,
        take_profit_atr_mult: 4.0,
        trailing_stop_pct: 0.05,
        max_hold_days: 30,
        atr_period: 14,
        ema_fast: scan_params.ema_fast,
        ema_slow: scan_params.ema_slow,
        rsi_period: 14,
        volume_sma_period: 20,
        bbands_period: 20,
        bbands_std: 2.0,
        rsi_entry_threshold: 55.0,
        rsi_oversold: 30.0,
        rsi_floor: 40.0,
        volume_multiplier: 1.5,
        lookback_period: 20,
        consolidation_days: 10,
        consolidation_range_pct: 0.05,
        squeeze_threshold: 0.02,
        spread_lookback: 30,
        entry_zscore: 2.0,
        ranking_period: 30,
        gap_threshold_pct: 0.03,
        min_distance_from_sma: 0.02,
    }
}

/// Build the indicator set for scanner usage (same as backtest).
fn build_scan_indicators(bars: &OhlcvBars, entry_logic: &[String], params: &BacktestParams) -> IndicatorSet {
    let n = bars.len();

    let vol_sma = if bars.volumes.iter().any(|&v| v > 0.0) {
        indicators::compute_sma(&bars.volumes, 20)
    } else {
        vec![f64::NAN; n]
    };

    let obv = if bars.volumes.iter().any(|&v| v > 0.0) {
        indicators::compute_obv(&bars.closes, &bars.volumes)
    } else {
        vec![0.0; n]
    };

    let needs_bb = entry_logic.iter().any(|c| {
        c.contains("squeeze")
            || c.contains("band")
            || c.contains("bollinger")
            || c.contains("lower_band")
    });

    let (bb_upper, bb_middle, bb_lower) = if needs_bb {
        let (u, m, l) = indicators::compute_bbands(&bars.closes, 20, 2.0);
        (Some(u), Some(m), Some(l))
    } else {
        (None, None, None)
    };

    IndicatorSet {
        closes: bars.closes.clone(),
        highs: bars.highs.clone(),
        lows: bars.lows.clone(),
        volumes: bars.volumes.clone(),
        atr: indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14),
        ema_fast: indicators::compute_ema(&bars.closes, params.ema_fast),
        ema_slow: indicators::compute_ema(&bars.closes, params.ema_slow),
        rsi: indicators::compute_rsi(&bars.closes, 14),
        vol_sma,
        sma: indicators::compute_sma(&bars.closes, 20),
        obv,
        bb_upper,
        bb_middle,
        bb_lower,
    }
}

/// Execute a signal scan across symbols.
///
/// Port of `execute_scan` (compute_worker.py:622-703).
/// Uses rayon for parallel symbol processing.
pub fn execute_scan(scan_params: &ScanParams) -> ScanJobResult {
    let cache_dir = data::get_cache_dir();
    let region = if scan_params.region.is_empty() {
        "us"
    } else {
        &scan_params.region
    };

    // Resolve symbols.
    let mut symbols = scan_params.symbols.clone();
    if let Some(ref sym) = scan_params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }

    let scan_type = if scan_params.scan_type.is_empty() {
        "momentum"
    } else {
        &scan_params.scan_type
    };

    let entry_logic = if scan_params.entry_logic.is_empty() {
        scan_type_to_logic(scan_type)
    } else {
        scan_params.entry_logic.clone()
    };

    let bt_params = scanner_params(scan_params);

    info!(
        "scanner: scanning {} symbols, type={}, conditions={:?}",
        symbols.len(),
        scan_type,
        entry_logic
    );

    // Scan symbols in parallel.
    let signals: Vec<Option<ScanSignal>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 {
                return None;
            }

            let n = bars.len();
            let ind = build_scan_indicators(&bars, &entry_logic, &bt_params);
            let last_idx = n - 1;

            if ind.atr[last_idx].is_nan() {
                return None;
            }

            // Use the backtest check_entry on the last bar.
            if backtest::check_entry(last_idx, &entry_logic, &ind, &bt_params, "long") {
                let rsi_val = if ind.rsi[last_idx].is_nan() {
                    50.0
                } else {
                    ind.rsi[last_idx]
                };

                // Base confidence from rule-based signal.
                let rule_confidence = (0.5 + rsi_val / 200.0).min(0.95);

                // ML-enhanced confidence: blend with RF model prediction if available.
                let confidence = if let Some(features) = ml_train::extract_latest_features(&bars) {
                    if let Some((ml_pred, ml_conf)) = ml_train::predict_single(&features) {
                        // ML agrees with buy signal: boost confidence.
                        // ML disagrees: reduce confidence.
                        if ml_pred == 1 {
                            // Agreement: weighted average favoring the higher signal.
                            round4((rule_confidence * 0.4 + ml_conf * 0.6).min(0.95))
                        } else {
                            // Disagreement: dampen but don't suppress entirely.
                            round4((rule_confidence * 0.7).min(0.85))
                        }
                    } else {
                        rule_confidence
                    }
                } else {
                    rule_confidence
                };

                Some(ScanSignal {
                    symbol: sym.clone(),
                    signal: "buy".to_string(),
                    scan_type: scan_type.to_string(),
                    price: round4(bars.closes[last_idx]),
                    rsi: round2(rsi_val),
                    date: bars.dates[last_idx].clone(),
                    confidence,
                })
            } else {
                None
            }
        })
        .collect();

    let results: Vec<ScanSignal> = signals.into_iter().flatten().collect();
    let signals_found = results.len();

    ScanJobResult {
        status: "completed".to_string(),
        job_type: "scan".to_string(),
        results,
        symbols_scanned: symbols.len(),
        signals_found,
        elapsed_ms: 0, // filled in by caller if needed
        pre_filtered: 0,
        gpu_used: false,
        cache_hits: 0,
    }
}

// ============================================================================
// Tauri IPC command
// ============================================================================

/// IPC command: run a local signal scan entirely in Rust (no Python).
///
/// Invoked from the frontend via `invoke('run_local_scan', { params })`.
#[tauri::command]
pub async fn run_local_scan(params: ScanParams) -> Result<ScanJobResult, String> {
    let result = tokio::task::spawn_blocking(move || execute_scan(&params))
        .await
        .map_err(|e| format!("Scan task panicked: {}", e))?;
    Ok(result)
}

// ── Helpers ─────────────────────────────────────────────────────────

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_type_to_logic() {
        let logic = scan_type_to_logic("momentum");
        assert_eq!(logic.len(), 3);
        assert!(logic.contains(&"ema_cross_up".to_string()));

        let logic = scan_type_to_logic("reversal");
        assert!(logic.contains(&"rsi_oversold".to_string()));

        let logic = scan_type_to_logic("breakout");
        assert!(logic.contains(&"price_above_high".to_string()));
    }

    #[test]
    fn test_scan_empty_symbols() {
        let params = ScanParams {
            symbols: vec![],
            symbol: None,
            region: "us".to_string(),
            scan_type: "momentum".to_string(),
            entry_logic: vec![],
            ema_fast: 9,
            ema_slow: 21,
        };
        let result = execute_scan(&params);
        assert_eq!(result.status, "completed");
        assert_eq!(result.symbols_scanned, 0);
    }
}

/// Grid worker entry point: takes raw JSON params, returns JSON result.
pub fn execute_scan_job(params_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params: ScanParams = serde_json::from_value(params_json.clone())
        .unwrap_or_default();
    let result = execute_scan(&params);
    serde_json::to_value(result).map_err(|e| format!("serialize error: {e}"))
}
