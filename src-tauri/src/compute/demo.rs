//! One-click demo backtest using bundled sample data.
//!
//! Shows users instant value on first launch -- no internet, no login, no setup.
//! Runs an SMA 10/30 crossover backtest on bundled CSV data and returns
//! aggregated results for multiple symbols in under 2 seconds.

use super::data::load_bars_from_csv;
use super::indicators::compute_sma;
use super::metrics::compute_metrics;
use super::types::{BacktestResult, OhlcvBars, Trade};
use serde::Serialize;
use std::path::PathBuf;

/// Symbols included in the bundled sample data.
const DEMO_SYMBOLS: &[&str] = &[
    "SPY", "QQQ", "AAPL", "MSFT", "NVDA", "TSLA", "AMZN", "GOOGL", "META", "AMD",
];

/// Result returned to the frontend for the demo backtest.
#[derive(Debug, Clone, Serialize)]
pub struct DemoBacktestResult {
    /// Per-symbol backtest results.
    pub results: Vec<BacktestResult>,
    /// Aggregate metrics across all symbols.
    pub summary: DemoSummary,
    /// How long the backtest took in milliseconds.
    pub elapsed_ms: u64,
    /// Number of symbols processed.
    pub symbols_processed: usize,
    /// Total number of OHLCV bars analyzed.
    pub total_bars: usize,
}

/// Aggregate summary across all demo symbols.
#[derive(Debug, Clone, Serialize)]
pub struct DemoSummary {
    pub total_trades: usize,
    pub avg_sharpe: f64,
    pub avg_win_rate: f64,
    pub best_symbol: String,
    pub best_return: f64,
    pub worst_symbol: String,
    pub worst_return: f64,
    pub combined_return: f64,
}

/// Resolve the path to the bundled sample data directory.
///
/// In a Tauri app, bundled resources are placed alongside the binary.
/// We check multiple locations:
/// 1. Tauri resource_dir (production builds)
/// 2. Adjacent to executable (installed app)
/// 3. Source tree (development)
fn find_sample_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    // 1. Try Tauri's resource resolver
    if let Ok(resource_path) = app.path().resource_dir() {
        let candidate = resource_path.join("resources").join("sample_data");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        // Also try without the extra "resources" prefix
        let candidate2 = resource_path.join("sample_data");
        if candidate2.is_dir() {
            return Ok(candidate2);
        }
    }

    // 2. Try adjacent to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("resources").join("sample_data");
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }

    // 3. Development fallback — source tree
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("sample_data");
    if dev_path.is_dir() {
        return Ok(dev_path);
    }

    Err("Could not locate bundled sample data. Try reinstalling the app.".to_string())
}

/// Run the SMA crossover backtest on a single symbol's data.
fn run_sma_crossover(bars: &OhlcvBars, symbol: &str) -> BacktestResult {
    let fast_period = 10;
    let slow_period = 30;
    let stop_loss = -0.05; // -5%
    let take_profit = 0.10; // +10%

    let sma_fast = compute_sma(&bars.closes, fast_period);
    let sma_slow = compute_sma(&bars.closes, slow_period);

    let mut trades: Vec<Trade> = Vec::new();
    let mut in_trade = false;
    let mut entry_price = 0.0;
    let mut entry_date = String::new();
    let mut entry_idx = 0;

    // Start after enough bars for the slow SMA to be valid
    let start = slow_period;

    for i in start..bars.len() {
        let fast = sma_fast[i];
        let slow = sma_slow[i];
        let prev_fast = sma_fast[i - 1];
        let prev_slow = sma_slow[i - 1];

        // Skip if any NaN
        if fast.is_nan() || slow.is_nan() || prev_fast.is_nan() || prev_slow.is_nan() {
            continue;
        }

        if in_trade {
            let current_price = bars.closes[i];
            let pnl_pct = (current_price - entry_price) / entry_price;

            // Check stop loss
            if pnl_pct <= stop_loss {
                trades.push(Trade {
                    entry_date: entry_date.clone(),
                    exit_date: bars.dates[i].clone(),
                    entry_price,
                    exit_price: current_price,
                    pnl_pct,
                    hold_days: (i - entry_idx) as f64,
                    exit_reason: "stop_loss".to_string(),
                    direction: "long".to_string(),
                });
                in_trade = false;
                continue;
            }

            // Check take profit
            if pnl_pct >= take_profit {
                trades.push(Trade {
                    entry_date: entry_date.clone(),
                    exit_date: bars.dates[i].clone(),
                    entry_price,
                    exit_price: current_price,
                    pnl_pct,
                    hold_days: (i - entry_idx) as f64,
                    exit_reason: "take_profit".to_string(),
                    direction: "long".to_string(),
                });
                in_trade = false;
                continue;
            }

            // Check bearish crossover (exit signal)
            if prev_fast > prev_slow && fast <= slow {
                trades.push(Trade {
                    entry_date: entry_date.clone(),
                    exit_date: bars.dates[i].clone(),
                    entry_price,
                    exit_price: current_price,
                    pnl_pct,
                    hold_days: (i - entry_idx) as f64,
                    exit_reason: "signal_exit".to_string(),
                    direction: "long".to_string(),
                });
                in_trade = false;
            }
        } else {
            // Bullish crossover → enter long
            if prev_fast <= prev_slow && fast > slow {
                entry_price = bars.closes[i];
                entry_date = bars.dates[i].clone();
                entry_idx = i;
                in_trade = true;
            }
        }
    }

    // Close any open trade at the end
    if in_trade {
        let last_idx = bars.len() - 1;
        let pnl_pct = (bars.closes[last_idx] - entry_price) / entry_price;
        trades.push(Trade {
            entry_date,
            exit_date: bars.dates[last_idx].clone(),
            entry_price,
            exit_price: bars.closes[last_idx],
            pnl_pct,
            hold_days: (last_idx - entry_idx) as f64,
            exit_reason: "end_of_data".to_string(),
            direction: "long".to_string(),
        });
    }

    let metrics = compute_metrics(&trades);

    BacktestResult {
        symbol: symbol.to_string(),
        strategy: "SMA Crossover (10/30)".to_string(),
        timeframe: "Daily".to_string(),
        metrics,
        trades,
    }
}

/// IPC command: Run the one-click demo backtest.
///
/// Loads bundled CSV sample data and runs an SMA 10/30 crossover strategy
/// across all demo symbols. Returns results in under 2 seconds.
#[tauri::command]
pub async fn run_demo_backtest(
    app: tauri::AppHandle,
) -> Result<DemoBacktestResult, String> {
    let start = std::time::Instant::now();

    let data_dir = find_sample_data_dir(&app)?;

    let mut results: Vec<BacktestResult> = Vec::new();
    let mut total_bars = 0;

    for &symbol in DEMO_SYMBOLS {
        let csv_path = data_dir.join(format!("{}.csv", symbol));
        if let Some(bars) = load_bars_from_csv(&csv_path) {
            total_bars += bars.len();
            let result = run_sma_crossover(&bars, symbol);
            results.push(result);
        }
    }

    if results.is_empty() {
        return Err("No sample data found. The app bundle may be incomplete.".to_string());
    }

    // Compute aggregate summary
    let total_trades: usize = results.iter().map(|r| r.metrics.num_trades).sum();
    let avg_sharpe = results.iter().map(|r| r.metrics.sharpe).sum::<f64>() / results.len() as f64;
    let avg_win_rate =
        results.iter().map(|r| r.metrics.win_rate).sum::<f64>() / results.len() as f64;

    let best = results
        .iter()
        .max_by(|a, b| {
            a.metrics
                .total_return
                .partial_cmp(&b.metrics.total_return)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    let worst = results
        .iter()
        .min_by(|a, b| {
            a.metrics
                .total_return
                .partial_cmp(&b.metrics.total_return)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    // Combined return: product of (1 + each symbol return) - 1
    let combined = results
        .iter()
        .map(|r| 1.0 + r.metrics.total_return)
        .product::<f64>()
        - 1.0;

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(DemoBacktestResult {
        symbols_processed: results.len(),
        total_bars,
        summary: DemoSummary {
            total_trades,
            avg_sharpe: (avg_sharpe * 100.0).round() / 100.0,
            avg_win_rate: (avg_win_rate * 10.0).round() / 10.0,
            best_symbol: best.symbol.clone(),
            best_return: (best.metrics.total_return * 10000.0).round() / 10000.0,
            worst_symbol: worst.symbol.clone(),
            worst_return: (worst.metrics.total_return * 10000.0).round() / 10000.0,
            combined_return: (combined * 10000.0).round() / 10000.0,
        },
        results,
        elapsed_ms: elapsed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma_crossover_basic() {
        // Create simple synthetic data that should produce at least one trade
        let bars = OhlcvBars {
            dates: (0..100).map(|i| format!("2025-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1)).collect(),
            opens: (0..100).map(|i| 100.0 + (i as f64 * 0.5).sin() * 10.0).collect(),
            highs: (0..100).map(|i| 102.0 + (i as f64 * 0.5).sin() * 10.0).collect(),
            lows: (0..100).map(|i| 98.0 + (i as f64 * 0.5).sin() * 10.0).collect(),
            closes: (0..100).map(|i| 100.0 + (i as f64 * 0.5).sin() * 10.0).collect(),
            volumes: vec![1_000_000.0; 100],
        };

        let result = run_sma_crossover(&bars, "TEST");
        assert_eq!(result.symbol, "TEST");
        assert_eq!(result.strategy, "SMA Crossover (10/30)");
        // With sinusoidal data there should be at least some trades
        // (exact count depends on the crossover pattern)
    }
}
