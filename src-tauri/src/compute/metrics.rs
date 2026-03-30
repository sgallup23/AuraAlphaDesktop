//! Backtest metrics — direct port of `_compute_metrics` from
//! compute_worker.py lines 477-530.

use super::types::{BacktestMetrics, Trade};
use std::collections::HashMap;

/// Compute aggregate performance metrics from a list of trades.
///
/// Matches the Python implementation exactly:
/// - Sharpe = mean / (std + eps)
/// - Sortino = mean / (downside_std + eps)
/// - Max drawdown via cumulative product of (1 + return)
/// - Win rate as percentage
pub fn compute_metrics(trades: &[Trade]) -> BacktestMetrics {
    if trades.is_empty() {
        return BacktestMetrics::default();
    }

    let returns: Vec<f64> = trades.iter().map(|t| t.pnl_pct).collect();
    let n = returns.len();
    let wins = returns.iter().filter(|&&r| r > 0.0).count();

    // Mean return.
    let avg_ret = returns.iter().sum::<f64>() / n as f64;

    // Sample std dev (ddof=1).
    let std_ret = if n > 1 {
        let variance =
            returns.iter().map(|&r| (r - avg_ret).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        variance.sqrt()
    } else {
        1e-9
    };

    // Downside std dev (ddof=1, only negative returns).
    let neg_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).cloned().collect();
    let downside = if !neg_returns.is_empty() {
        let neg_n = neg_returns.len();
        let neg_mean = neg_returns.iter().sum::<f64>() / neg_n as f64;
        if neg_n > 1 {
            let neg_var = neg_returns
                .iter()
                .map(|&r| (r - neg_mean).powi(2))
                .sum::<f64>()
                / (neg_n as f64 - 1.0);
            neg_var.sqrt()
        } else {
            1e-9
        }
    } else {
        1e-9
    };

    let sharpe = avg_ret / (std_ret + 1e-9);
    let sortino = avg_ret / (downside + 1e-9);

    // Profit factor.
    let gross_profit: f64 = returns.iter().filter(|&&r| r > 0.0).sum();
    let gross_loss: f64 = returns.iter().filter(|&&r| r < 0.0).sum::<f64>().abs();
    let profit_factor = gross_profit / (gross_loss + 1e-9);

    // Max drawdown via cumulative product.
    let mut cum = Vec::with_capacity(n);
    let mut prod = 1.0;
    for &r in &returns {
        prod *= 1.0 + r;
        cum.push(prod);
    }

    let mut peak = Vec::with_capacity(n);
    let mut current_peak = f64::NEG_INFINITY;
    for &c in &cum {
        current_peak = current_peak.max(c);
        peak.push(current_peak);
    }

    let dd: Vec<f64> = cum
        .iter()
        .zip(peak.iter())
        .map(|(&c, &p)| (c - p) / p)
        .collect();

    let max_dd = dd.iter().cloned().fold(0.0_f64, f64::min);

    let total_ret = if !cum.is_empty() {
        cum[cum.len() - 1] - 1.0
    } else {
        0.0
    };

    let avg_hold = trades.iter().map(|t| t.hold_days).sum::<f64>() / n as f64;

    // Exit reason counts.
    let mut exit_reasons: HashMap<String, usize> = HashMap::new();
    for t in trades {
        let reason = if t.exit_reason.is_empty() {
            "unknown".to_string()
        } else {
            t.exit_reason.clone()
        };
        *exit_reasons.entry(reason).or_insert(0) += 1;
    }

    BacktestMetrics {
        num_trades: n,
        sharpe: round_f64(sharpe, 4),
        sortino: round_f64(sortino, 4),
        profit_factor: round_f64(profit_factor, 4),
        win_rate: round_f64(wins as f64 / n as f64 * 100.0, 1),
        avg_return: round_f64(avg_ret, 6),
        max_drawdown: round_f64(max_dd, 6),
        total_return: round_f64(total_ret, 6),
        avg_hold_days: round_f64(avg_hold, 1),
        gross_profit: round_f64(gross_profit, 6),
        gross_loss: round_f64(gross_loss, 6),
        wins,
        losses: n - wins,
        exit_reasons,
    }
}

/// Round an f64 to `decimals` places, matching Python's `round()`.
fn round_f64(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

// ── Unit tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trade(pnl_pct: f64, hold_days: f64, exit_reason: &str) -> Trade {
        Trade {
            entry_date: "2025-01-01".to_string(),
            exit_date: "2025-01-02".to_string(),
            entry_price: 100.0,
            exit_price: 100.0 * (1.0 + pnl_pct),
            pnl_pct,
            hold_days,
            exit_reason: exit_reason.to_string(),
            direction: "long".to_string(),
        }
    }

    #[test]
    fn test_empty_trades() {
        let metrics = compute_metrics(&[]);
        assert_eq!(metrics.num_trades, 0);
        assert_eq!(metrics.sharpe, 0.0);
        assert_eq!(metrics.win_rate, 0.0);
    }

    #[test]
    fn test_all_wins() {
        let trades = vec![
            make_trade(0.05, 3.0, "target"),
            make_trade(0.03, 2.0, "target"),
            make_trade(0.02, 1.0, "target"),
        ];
        let m = compute_metrics(&trades);
        assert_eq!(m.num_trades, 3);
        assert_eq!(m.wins, 3);
        assert_eq!(m.losses, 0);
        assert!(m.win_rate > 99.9);
        assert!(m.sharpe > 0.0);
        assert!(m.total_return > 0.0);
        assert!(m.max_drawdown >= -1e-9); // No drawdown expected.
    }

    #[test]
    fn test_mixed_trades() {
        let trades = vec![
            make_trade(0.05, 3.0, "target"),
            make_trade(-0.02, 2.0, "stop_loss"),
            make_trade(0.03, 4.0, "target"),
            make_trade(-0.01, 1.0, "stop_loss"),
        ];
        let m = compute_metrics(&trades);
        assert_eq!(m.num_trades, 4);
        assert_eq!(m.wins, 2);
        assert_eq!(m.losses, 2);
        assert!((m.win_rate - 50.0).abs() < 0.1);
        assert!(m.profit_factor > 1.0); // Wins > losses.

        // Check exit reasons.
        assert_eq!(m.exit_reasons.get("target"), Some(&2));
        assert_eq!(m.exit_reasons.get("stop_loss"), Some(&2));
    }

    #[test]
    fn test_round_f64() {
        assert_eq!(round_f64(1.23456, 4), 1.2346);
        assert_eq!(round_f64(0.1, 1), 0.1);
        assert_eq!(round_f64(99.95, 0), 100.0);
    }
}
