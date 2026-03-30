//! Rust-native backtest engine — port of compute_worker.py lines 181-620.
//!
//! Zero Python dependencies. Users download an .exe and it works on any machine.
//! Processes symbols in parallel via rayon.

use super::data;
use super::indicators;
use super::metrics;
use super::types::*;
use log::info;
use rayon::prelude::*;

// ============================================================================
// Entry logic — port of _check_entry (compute_worker.py:181-305)
// ============================================================================

/// Computed indicator arrays for a single symbol, passed to entry checks.
/// Public so that the scanner module can reuse entry logic.
pub struct IndicatorSet {
    pub closes: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub volumes: Vec<f64>,
    pub atr: Vec<f64>,
    pub ema_fast: Vec<f64>,
    pub ema_slow: Vec<f64>,
    pub rsi: Vec<f64>,
    pub vol_sma: Vec<f64>,
    pub sma: Vec<f64>,
    pub obv: Vec<f64>,
    pub bb_upper: Option<Vec<f64>>,
    pub bb_middle: Option<Vec<f64>>,
    pub bb_lower: Option<Vec<f64>>,
}

/// Check if entry conditions are met at bar index `i`.
///
/// Returns true when >= 50% of conditions in `entry_logic` fire.
/// Mirrors Python `_check_entry` exactly — 25+ condition types.
pub fn check_entry(
    i: usize,
    entry_logic: &[String],
    ind: &IndicatorSet,
    params: &BacktestParams,
    direction: &str,
) -> bool {
    let conditions_total = if entry_logic.is_empty() { 1 } else { entry_logic.len() };
    let mut conditions_met: usize = 0;

    for cond in entry_logic {
        let ok = match cond.as_str() {
            "ema_cross_up" => {
                let ef = &ind.ema_fast;
                let es = &ind.ema_slow;
                i > 0
                    && !ef[i].is_nan()
                    && !es[i].is_nan()
                    && ef[i] > es[i]
                    && ef[i - 1] <= es[i - 1]
            }
            "ema_cross_down" => {
                let ef = &ind.ema_fast;
                let es = &ind.ema_slow;
                i > 0
                    && !ef[i].is_nan()
                    && !es[i].is_nan()
                    && ef[i] < es[i]
                    && ef[i - 1] >= es[i - 1]
            }
            "rsi_above_threshold" => {
                let thr = params.rsi_entry_threshold;
                ind.rsi[i] > thr
            }
            "rsi_oversold" => {
                let thr = params.rsi_oversold;
                ind.rsi[i] < thr
            }
            "rsi_above_floor" => {
                let thr = params.rsi_floor;
                ind.rsi[i] > thr
            }
            "volume_surge" | "volume_spike" => {
                let vm = params.volume_multiplier;
                let vs = ind.vol_sma[i];
                let vol = ind.volumes[i];
                !vs.is_nan() && vs > 0.0 && vol > vs * vm
            }
            "price_above_high" => {
                let lb = params.lookback_period;
                if i >= lb {
                    let max_high = ind.highs[i - lb..i]
                        .iter()
                        .cloned()
                        .fold(f64::NEG_INFINITY, f64::max);
                    ind.closes[i] > max_high
                } else {
                    false
                }
            }
            "volume_breakout" => {
                let vm = params.volume_multiplier.max(2.0); // default 2.0 for breakout
                let vs = ind.vol_sma[i];
                let vol = ind.volumes[i];
                !vs.is_nan() && vs > 0.0 && vol > vs * vm
            }
            "consolidation_check" => {
                let cd = params.consolidation_days;
                let rp = params.consolidation_range_pct;
                if i >= cd {
                    let hi = ind.highs[i - cd..i]
                        .iter()
                        .cloned()
                        .fold(f64::NEG_INFINITY, f64::max);
                    let lo = ind.lows[i - cd..i]
                        .iter()
                        .cloned()
                        .fold(f64::INFINITY, f64::min);
                    lo > 0.0 && (hi - lo) / lo < rp
                } else {
                    false
                }
            }
            "squeeze_fire" | "band_expansion" => {
                if let (Some(bb_u), Some(bb_m), Some(bb_l)) =
                    (&ind.bb_upper, &ind.bb_middle, &ind.bb_lower)
                {
                    if !bb_u[i].is_nan() && !bb_l[i].is_nan() && i > 0 {
                        let squeeze = params.squeeze_threshold;
                        let width = (bb_u[i] - bb_l[i]) / (bb_m[i] + 1e-10);
                        let prev_width = if !bb_u[i - 1].is_nan() {
                            (bb_u[i - 1] - bb_l[i - 1]) / (bb_m[i - 1] + 1e-10)
                        } else {
                            width
                        };
                        prev_width < squeeze && width > squeeze
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "direction_filter" => {
                let ef = &ind.ema_fast;
                if !ef[i].is_nan() {
                    if direction == "long" {
                        ind.closes[i] > ef[i]
                    } else {
                        ind.closes[i] < ef[i]
                    }
                } else {
                    false
                }
            }
            "zscore_extreme" => {
                let lb = params.spread_lookback;
                let ez = params.entry_zscore;
                if i >= lb {
                    let window = &ind.closes[i - lb..i];
                    let mean = window.iter().sum::<f64>() / lb as f64;
                    let variance =
                        window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / lb as f64;
                    let std = variance.sqrt();
                    if std > 0.0 {
                        let z = (ind.closes[i] - mean) / std;
                        z.abs() >= ez
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "correlation_stable" => {
                // Always passes (placeholder for pair strategy).
                true
            }
            "top_sector_rank" | "momentum_positive" => {
                let lb = params.ranking_period;
                i >= lb && ind.closes[i] > ind.closes[i - lb]
            }
            "gap_detection" => {
                let gt = params.gap_threshold_pct;
                i > 0 && (ind.closes[i] / ind.closes[i - 1] - 1.0).abs() > gt
            }
            "event_window" => {
                // Always passes (external event data not available locally).
                true
            }
            "obv_rising" => {
                i > 5 && ind.obv[i] > ind.obv[i - 5]
            }
            "price_below_lower_band" => {
                if let Some(bb_l) = &ind.bb_lower {
                    !bb_l[i].is_nan() && ind.closes[i] < bb_l[i]
                } else {
                    false
                }
            }
            "distance_from_sma" => {
                let md = params.min_distance_from_sma;
                let sma_val = ind.sma[i];
                !sma_val.is_nan() && sma_val > 0.0 && {
                    let dist = (ind.closes[i] - sma_val).abs() / sma_val;
                    dist >= md
                }
            }
            "price_above_vwap" => {
                let lb = params.volume_sma_period;
                if i >= lb {
                    let start = i - lb;
                    let mut weighted_sum = 0.0;
                    let mut vol_sum = 0.0;
                    for j in start..i {
                        weighted_sum += ind.closes[j] * ind.volumes[j];
                        vol_sum += ind.volumes[j];
                    }
                    let vwap = weighted_sum / (vol_sum + 1e-10);
                    ind.closes[i] > vwap
                } else {
                    false
                }
            }
            // Pass-through conditions (always met).
            "bollinger_squeeze" | "rsi_confirmation" | "volume_confirm" => true,
            // Unknown conditions — pass (matches Python fallback).
            _ => true,
        };

        if ok {
            conditions_met += 1;
        }
    }

    let threshold = (conditions_total as f64 * 0.5).ceil().max(1.0) as usize;
    conditions_met >= threshold
}

// ============================================================================
// Trade simulation — port of _simulate_trades (compute_worker.py:312-470)
// ============================================================================

/// Build the full indicator set for a symbol's bar data.
fn build_indicators(bars: &OhlcvBars, params: &BacktestParams, entry_logic: &[String]) -> IndicatorSet {
    let n = bars.len();

    let vol_sma = if bars.volumes.iter().any(|&v| v > 0.0) {
        indicators::compute_sma(&bars.volumes, params.volume_sma_period)
    } else {
        vec![f64::NAN; n]
    };

    let obv = if bars.volumes.iter().any(|&v| v > 0.0) {
        indicators::compute_obv(&bars.closes, &bars.volumes)
    } else {
        vec![0.0; n]
    };

    // Only compute Bollinger Bands if needed by entry logic.
    let needs_bb = entry_logic.iter().any(|c| {
        c.contains("squeeze")
            || c.contains("band")
            || c.contains("bollinger")
            || c.contains("lower_band")
    });

    let (bb_upper, bb_middle, bb_lower) = if needs_bb {
        let (u, m, l) = indicators::compute_bbands(&bars.closes, params.bbands_period, params.bbands_std);
        (Some(u), Some(m), Some(l))
    } else {
        (None, None, None)
    };

    IndicatorSet {
        closes: bars.closes.clone(),
        highs: bars.highs.clone(),
        lows: bars.lows.clone(),
        volumes: bars.volumes.clone(),
        atr: indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, params.atr_period),
        ema_fast: indicators::compute_ema(&bars.closes, params.ema_fast),
        ema_slow: indicators::compute_ema(&bars.closes, params.ema_slow),
        rsi: indicators::compute_rsi(&bars.closes, params.rsi_period),
        vol_sma,
        sma: indicators::compute_sma(&bars.closes, params.bbands_period),
        obv,
        bb_upper,
        bb_middle,
        bb_lower,
    }
}

/// Simulate trades on a single symbol's bar data.
///
/// Port of `_simulate_trades` (compute_worker.py:312-470).
/// Walks through bars, checks entry conditions, manages positions with
/// ATR-based stops, trailing stop, and max hold days.
fn simulate_trades(
    bars: &OhlcvBars,
    params: &BacktestParams,
    direction: &str,
    date_start: &str,
    date_end: &str,
    entry_logic: &[String],
) -> Vec<Trade> {
    let n = bars.len();
    if n < 50 {
        return Vec::new();
    }

    let stop_atr = params.stop_loss_atr_mult;
    let tp_atr = params.take_profit_atr_mult;
    let mut trail_pct = params.trailing_stop_pct;
    if trail_pct > 1.0 {
        trail_pct /= 100.0;
    }
    let max_hold = params.max_hold_days;

    let actual_logic: Vec<String> = if entry_logic.is_empty() {
        vec![
            "ema_cross_up".to_string(),
            "rsi_above_threshold".to_string(),
            "volume_surge".to_string(),
        ]
    } else {
        entry_logic.to_vec()
    };

    let ind = build_indicators(bars, params, &actual_logic);

    // Date range filtering.
    let mut start_idx: usize = 0;
    let mut end_idx: usize = n;

    if !date_start.is_empty() {
        for (i, d) in bars.dates.iter().enumerate() {
            if d.as_str() >= date_start {
                start_idx = i;
                break;
            }
        }
    }
    if !date_end.is_empty() {
        for i in (0..n).rev() {
            if bars.dates[i].as_str() <= date_end {
                end_idx = i + 1;
                break;
            }
        }
    }

    // Minimum lookback to avoid NaN indicators.
    let min_lookback = *[
        params.ema_slow,
        params.atr_period,
        params.rsi_period,
        params.volume_sma_period,
        params.bbands_period,
    ]
    .iter()
    .max()
    .unwrap_or(&0)
        + 5;
    start_idx = start_idx.max(min_lookback);

    let mut trades: Vec<Trade> = Vec::new();
    let mut in_trade = false;
    let mut entry_price = 0.0_f64;
    let mut entry_idx = 0_usize;
    let mut stop_price = 0.0_f64;
    let mut tp_price = 0.0_f64;
    let mut trail_high = 0.0_f64;
    let mut trail_low = f64::INFINITY;

    let atr = &ind.atr;

    for i in start_idx..end_idx.min(n) {
        if atr[i].is_nan() {
            continue;
        }

        if !in_trade {
            if check_entry(i, &actual_logic, &ind, params, direction) {
                entry_price = bars.closes[i];
                entry_idx = i;
                if direction == "long" {
                    stop_price = entry_price - atr[i] * stop_atr;
                    tp_price = entry_price + atr[i] * tp_atr;
                    trail_high = entry_price;
                } else {
                    stop_price = entry_price + atr[i] * stop_atr;
                    tp_price = entry_price - atr[i] * tp_atr;
                    trail_low = entry_price;
                }
                in_trade = true;
            }
        } else {
            let hold_days = i - entry_idx;
            let mut exit_price: Option<f64> = None;
            let mut exit_reason = "";

            if direction == "long" {
                trail_high = trail_high.max(bars.highs[i]);
                let trail_stop = trail_high * (1.0 - trail_pct);

                if bars.lows[i] <= stop_price {
                    exit_price = Some(stop_price);
                    exit_reason = "stop_loss";
                } else if bars.highs[i] >= tp_price {
                    exit_price = Some(tp_price);
                    exit_reason = "take_profit";
                } else if bars.closes[i] <= trail_stop && hold_days > 1 {
                    exit_price = Some(trail_stop);
                    exit_reason = "trailing_stop";
                } else if hold_days >= max_hold {
                    exit_price = Some(bars.closes[i]);
                    exit_reason = "max_hold";
                }
            } else {
                trail_low = trail_low.min(bars.lows[i]);
                let trail_stop = trail_low * (1.0 + trail_pct);

                if bars.highs[i] >= stop_price {
                    exit_price = Some(stop_price);
                    exit_reason = "stop_loss";
                } else if bars.lows[i] <= tp_price {
                    exit_price = Some(tp_price);
                    exit_reason = "take_profit";
                } else if bars.closes[i] >= trail_stop && hold_days > 1 {
                    exit_price = Some(trail_stop);
                    exit_reason = "trailing_stop";
                } else if hold_days >= max_hold {
                    exit_price = Some(bars.closes[i]);
                    exit_reason = "max_hold";
                }
            }

            if let Some(ep) = exit_price {
                let pnl_pct = if direction == "long" {
                    (ep - entry_price) / entry_price
                } else {
                    (entry_price - ep) / entry_price
                };
                trades.push(Trade {
                    entry_date: bars.dates[entry_idx].clone(),
                    exit_date: bars.dates[i].clone(),
                    entry_price: round4(entry_price),
                    exit_price: round4(ep),
                    pnl_pct: round6(pnl_pct),
                    hold_days: hold_days as f64,
                    exit_reason: exit_reason.to_string(),
                    direction: direction.to_string(),
                });
                in_trade = false;
            }
        }
    }

    // Close any open position at window end.
    if in_trade && end_idx > entry_idx {
        let final_idx = (end_idx - 1).min(n - 1);
        let ep = bars.closes[final_idx];
        let pnl_pct = if direction == "long" {
            (ep - entry_price) / entry_price
        } else {
            (entry_price - ep) / entry_price
        };
        trades.push(Trade {
            entry_date: bars.dates[entry_idx].clone(),
            exit_date: bars.dates[final_idx].clone(),
            entry_price: round4(entry_price),
            exit_price: round4(ep),
            pnl_pct: round6(pnl_pct),
            hold_days: (final_idx - entry_idx) as f64,
            exit_reason: "window_end".to_string(),
            direction: direction.to_string(),
        });
    }

    trades
}

// ============================================================================
// Backtest executor — port of execute_backtest (compute_worker.py:537-620)
// ============================================================================

/// Execute a full backtest across a universe of symbols.
///
/// Uses rayon::par_iter to process symbols in parallel across all CPU cores.
/// Returns a `BacktestJobResult` matching the Python output format.
pub fn execute_backtest(params: &BacktestParams) -> BacktestJobResult {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };

    // Resolve symbol universe.
    let mut symbols = params.symbol_universe.clone();
    if let Some(ref sym) = params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }
    if symbols.is_empty() {
        // Auto-sample from cached data.
        let available = data::list_available_symbols(region, &cache_dir);
        let sample_size = available.len().min(15);
        if sample_size > 0 {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let mut sampled = available;
            sampled.shuffle(&mut rng);
            sampled.truncate(sample_size);
            symbols = sampled;
        }
    }

    let direction = if params.direction.is_empty() { "long" } else { &params.direction };
    let entry_logic = if params.entry_logic.is_empty() {
        vec![
            "ema_cross_up".to_string(),
            "rsi_above_threshold".to_string(),
            "volume_surge".to_string(),
        ]
    } else {
        params.entry_logic.clone()
    };

    info!(
        "backtest: running {} symbols, direction={}, conditions={:?}",
        symbols.len(),
        direction,
        entry_logic
    );

    // Process symbols in parallel with rayon.
    let results: Vec<(Vec<Trade>, bool)> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir);
            match bars {
                Some(b) if b.len() >= 50 => {
                    let trades = simulate_trades(
                        &b,
                        params,
                        direction,
                        &params.date_start,
                        &params.date_end,
                        &entry_logic,
                    );
                    (trades, true) // (trades, was_tested)
                }
                _ => (Vec::new(), false), // skipped
            }
        })
        .collect();

    let mut all_trades: Vec<Trade> = Vec::new();
    let mut symbols_tested = 0_usize;
    let mut symbols_skipped = 0_usize;

    for (trades, tested) in results {
        if tested {
            symbols_tested += 1;
            all_trades.extend(trades);
        } else {
            symbols_skipped += 1;
        }
    }

    let base_metrics = metrics::compute_metrics(&all_trades);

    let date_window = if params.date_start.is_empty() && params.date_end.is_empty() {
        ":".to_string()
    } else {
        format!("{}:{}", params.date_start, params.date_end)
    };

    BacktestJobResult {
        status: "completed".to_string(),
        job_type: "backtest".to_string(),
        metrics: BacktestMetricsExt {
            base: base_metrics,
            symbols_tested,
            symbols_skipped,
            strategy_family: params.strategy_family.clone(),
            date_window,
        },
    }
}

// ============================================================================
// Tauri IPC command
// ============================================================================

/// IPC command: run a local backtest entirely in Rust (no Python).
///
/// Invoked from the frontend via `invoke('run_local_backtest', { params })`.
#[tauri::command]
pub async fn run_local_backtest(params: BacktestParams) -> Result<BacktestJobResult, String> {
    let result = tokio::task::spawn_blocking(move || execute_backtest(&params))
        .await
        .map_err(|e| format!("Backtest task panicked: {}", e))?;
    Ok(result)
}

/// Grid worker entry point: takes raw JSON params, returns JSON result.
pub fn execute_backtest_job(params_json: &serde_json::Value) -> Result<serde_json::Value, String> {
    let params: BacktestParams = serde_json::from_value(params_json.clone())
        .unwrap_or_default();
    let result = execute_backtest(&params);
    serde_json::to_value(result).map_err(|e| format!("serialize error: {e}"))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn round6(v: f64) -> f64 {
    (v * 1000000.0).round() / 1000000.0
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bars(n: usize) -> OhlcvBars {
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut volumes = Vec::with_capacity(n);
        let mut dates = Vec::with_capacity(n);
        let mut opens = Vec::with_capacity(n);

        for i in 0..n {
            let base = 100.0 + (i as f64) * 0.5;
            closes.push(base);
            highs.push(base + 1.0);
            lows.push(base - 1.0);
            opens.push(base - 0.2);
            volumes.push(1000.0 + (i as f64) * 10.0);
            dates.push(format!("2025-01-{:02}", (i % 28) + 1));
        }

        OhlcvBars {
            dates,
            opens,
            highs,
            lows,
            closes,
            volumes,
        }
    }

    #[test]
    fn test_check_entry_ema_cross_up() {
        let n = 100;
        let bars = make_bars(n);
        let params = BacktestParams {
            strategy_family: String::new(),
            symbol_universe: vec![],
            symbol: None,
            region: "us".to_string(),
            direction: "long".to_string(),
            date_start: String::new(),
            date_end: String::new(),
            entry_logic: vec!["ema_cross_up".to_string()],
            stop_loss_atr_mult: 2.0,
            take_profit_atr_mult: 4.0,
            trailing_stop_pct: 0.05,
            max_hold_days: 30,
            atr_period: 14,
            ema_fast: 9,
            ema_slow: 21,
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
        };

        let logic = vec!["ema_cross_up".to_string()];
        let ind = build_indicators(&bars, &params, &logic);

        // With monotonically increasing prices, EMA fast > EMA slow after warmup,
        // but there should be no cross (it's always above after the first cross).
        // The function should run without panicking.
        let _ = check_entry(50, &logic, &ind, &params, "long");
    }

    #[test]
    fn test_simulate_trades_short_data() {
        let bars = make_bars(10); // < 50 bars
        let params = BacktestParams {
            strategy_family: String::new(),
            symbol_universe: vec![],
            symbol: None,
            region: "us".to_string(),
            direction: "long".to_string(),
            date_start: String::new(),
            date_end: String::new(),
            entry_logic: vec![],
            stop_loss_atr_mult: 2.0,
            take_profit_atr_mult: 4.0,
            trailing_stop_pct: 0.05,
            max_hold_days: 30,
            atr_period: 14,
            ema_fast: 9,
            ema_slow: 21,
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
        };

        let trades = simulate_trades(&bars, &params, "long", "", "", &[]);
        assert!(trades.is_empty());
    }

    #[test]
    fn test_simulate_trades_produces_trades() {
        // 200 bars with a sine wave to force entries and exits.
        let n = 200;
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut volumes = Vec::with_capacity(n);
        let mut dates = Vec::with_capacity(n);
        let mut opens = Vec::with_capacity(n);

        for i in 0..n {
            let base = 100.0 + 20.0 * (i as f64 * 0.1).sin();
            closes.push(base);
            highs.push(base + 2.0);
            lows.push(base - 2.0);
            opens.push(base - 0.5);
            volumes.push(if i % 7 == 0 { 5000.0 } else { 1000.0 });
            dates.push(format!("2025-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1));
        }

        let bars = OhlcvBars {
            dates,
            opens,
            highs,
            lows,
            closes,
            volumes,
        };

        let params = BacktestParams {
            strategy_family: String::new(),
            symbol_universe: vec![],
            symbol: None,
            region: "us".to_string(),
            direction: "long".to_string(),
            date_start: String::new(),
            date_end: String::new(),
            entry_logic: vec!["rsi_above_floor".to_string()],
            stop_loss_atr_mult: 1.0,
            take_profit_atr_mult: 2.0,
            trailing_stop_pct: 0.05,
            max_hold_days: 10,
            atr_period: 5,
            ema_fast: 5,
            ema_slow: 10,
            rsi_period: 5,
            volume_sma_period: 10,
            bbands_period: 10,
            bbands_std: 2.0,
            rsi_entry_threshold: 55.0,
            rsi_oversold: 30.0,
            rsi_floor: 30.0, // low floor to trigger easily
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
        };

        let logic = vec!["rsi_above_floor".to_string()];
        let trades = simulate_trades(&bars, &params, "long", "", "", &logic);
        // With oscillating prices, we should get at least some trades.
        assert!(!trades.is_empty(), "Expected trades from sine-wave data");
        for t in &trades {
            assert_eq!(t.direction, "long");
        }
    }
}
