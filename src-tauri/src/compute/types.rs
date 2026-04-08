//! Core data types for the computation engine.
//!
//! Every struct derives `Serialize` so it can cross the Tauri IPC boundary
//! directly — no conversion layer needed.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Market data ──────────────────────────────────────────────────────

/// OHLCV bar series for a single symbol.
#[derive(Debug, Clone, Serialize)]
pub struct OhlcvBars {
    /// ISO-8601 date strings (e.g. "2025-03-15").
    pub dates: Vec<String>,
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub closes: Vec<f64>,
    pub volumes: Vec<f64>,
}

impl OhlcvBars {
    /// Number of bars in the series.
    pub fn len(&self) -> usize {
        self.closes.len()
    }

    /// Returns true if the series is empty.
    pub fn is_empty(&self) -> bool {
        self.closes.is_empty()
    }
}

// ── Backtest types ───────────────────────────────────────────────────

/// A single simulated trade produced by a backtest.
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub entry_date: String,
    pub exit_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl_pct: f64,
    pub hold_days: f64,
    pub exit_reason: String,
    pub direction: String,
}

/// Aggregate performance metrics for a set of trades.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestMetrics {
    pub num_trades: usize,
    pub sharpe: f64,
    pub sortino: f64,
    pub profit_factor: f64,
    pub win_rate: f64,
    pub avg_return: f64,
    pub max_drawdown: f64,
    pub total_return: f64,
    pub avg_hold_days: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
    pub wins: usize,
    pub losses: usize,
    pub exit_reasons: HashMap<String, usize>,
}

impl Default for BacktestMetrics {
    fn default() -> Self {
        Self {
            num_trades: 0,
            sharpe: 0.0,
            sortino: 0.0,
            profit_factor: 0.0,
            win_rate: 0.0,
            avg_return: 0.0,
            max_drawdown: 0.0,
            total_return: 0.0,
            avg_hold_days: 0.0,
            gross_profit: 0.0,
            gross_loss: 0.0,
            wins: 0,
            losses: 0,
            exit_reasons: HashMap::new(),
        }
    }
}

/// Full backtest result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestResult {
    pub symbol: String,
    pub strategy: String,
    pub timeframe: String,
    pub metrics: BacktestMetrics,
    pub trades: Vec<Trade>,
}

// ── Scanner types ────────────────────────────────────────────────────

/// A signal emitted by the real-time scanner.
#[derive(Debug, Clone, Serialize)]
pub struct ScanSignal {
    pub symbol: String,
    pub signal: String,     // "buy" | "sell"
    pub scan_type: String,  // "momentum" | "reversal" | "breakout" | "volume"
    pub confidence: f64,
    pub price: f64,
    pub rsi: f64,
    pub date: String,
}

// ── ML types ─────────────────────────────────────────────────────────

/// A row of computed features for a single symbol (matches Python feature extraction).
#[derive(Debug, Clone, Serialize)]
pub struct FeatureRow {
    pub symbol: String,
    pub date: String,
    pub features: FeatureValues,
}

/// The 11 extracted feature values per symbol.
#[derive(Debug, Clone, Serialize)]
pub struct FeatureValues {
    pub rsi_14: f64,
    pub ema_9: f64,
    pub ema_21: f64,
    pub atr_14: f64,
    pub bb_width: f64,
    pub returns_1d: f64,
    pub returns_5d: f64,
    pub returns_20d: f64,
    pub volume_ratio: f64,
    pub close: f64,
}

/// Column-major (struct-of-arrays) feature matrix for batch ML workloads.
///
/// Each column is a contiguous f64 slice -- all RSI values together, all EMA values
/// together, etc. This layout is cache-line friendly for ML training where the model
/// reads one feature across all samples (column scan pattern), and enables SIMD
/// auto-vectorization of per-column normalization.
///
/// Column order matches `FeatureValues` field order (same as NUM_FEATURES = 10):
/// [rsi_14, ema_9, ema_21, atr_14, bb_width, returns_1d, returns_5d, returns_20d, volume_ratio, close]
#[derive(Debug, Clone, Serialize)]
pub struct FeatureMatrix {
    /// Column names for documentation/debugging.
    pub column_names: Vec<String>,
    /// Number of symbols (rows) in the matrix.
    pub n_rows: usize,
    /// Number of features (columns) -- always 10.
    pub n_cols: usize,
    /// Column-major storage: columns[col_idx][row_idx].
    /// Each inner Vec is a contiguous f64 slice of length n_rows.
    pub columns: Vec<Vec<f64>>,
    /// Symbol names corresponding to each row.
    pub symbols: Vec<String>,
    /// Dates corresponding to each row.
    pub dates: Vec<String>,
}

impl FeatureMatrix {
    /// Feature column names in canonical order.
    pub const COLUMN_NAMES: [&'static str; 10] = [
        "rsi_14", "ema_9", "ema_21", "atr_14", "bb_width",
        "returns_1d", "returns_5d", "returns_20d", "volume_ratio", "close",
    ];

    /// Create an empty feature matrix with pre-allocated columns.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            column_names: Self::COLUMN_NAMES.iter().map(|s| s.to_string()).collect(),
            n_rows: 0,
            n_cols: 10,
            columns: (0..10).map(|_| Vec::with_capacity(capacity)).collect(),
            symbols: Vec::with_capacity(capacity),
            dates: Vec::with_capacity(capacity),
        }
    }

    /// Push a row of features (from a FeatureValues struct).
    pub fn push_row(&mut self, symbol: String, date: String, fv: &FeatureValues) {
        self.columns[0].push(fv.rsi_14);
        self.columns[1].push(fv.ema_9);
        self.columns[2].push(fv.ema_21);
        self.columns[3].push(fv.atr_14);
        self.columns[4].push(fv.bb_width);
        self.columns[5].push(fv.returns_1d);
        self.columns[6].push(fv.returns_5d);
        self.columns[7].push(fv.returns_20d);
        self.columns[8].push(fv.volume_ratio);
        self.columns[9].push(fv.close);
        self.symbols.push(symbol);
        self.dates.push(date);
        self.n_rows += 1;
    }

    /// Normalize a single column in-place using z-score: (x - mean) / std.
    /// Operates on a contiguous f64 slice -- SIMD auto-vectorizable.
    pub fn normalize_column_inplace(&mut self, col_idx: usize) {
        if col_idx >= self.n_cols || self.n_rows == 0 {
            return;
        }
        let col = &mut self.columns[col_idx];
        let n = col.len() as f64;

        // Compute mean in one pass.
        let mean: f64 = col.iter().sum::<f64>() / n;

        // Compute std in one pass.
        let variance: f64 = col.iter().map(|&x| {
            let d = x - mean;
            d * d
        }).sum::<f64>() / n;
        let std = if variance < 1e-20 { 1.0 } else { variance.sqrt() };

        // Normalize in-place -- single contiguous slice, SIMD-friendly.
        let inv_std = 1.0 / std;
        for x in col.iter_mut() {
            *x = (*x - mean) * inv_std;
        }
    }

    /// Normalize all columns in-place.
    pub fn normalize_all_inplace(&mut self) {
        for col_idx in 0..self.n_cols {
            self.normalize_column_inplace(col_idx);
        }
    }

    /// Convert to row-major flat Vec<f64> (for compatibility with DenseMatrix).
    /// Layout: [row0_col0, row0_col1, ..., row0_col9, row1_col0, ...]
    pub fn to_flat_row_major(&self) -> Vec<f64> {
        let mut flat = vec![0.0_f64; self.n_rows * self.n_cols];
        for col in 0..self.n_cols {
            for row in 0..self.n_rows {
                flat[row * self.n_cols + col] = self.columns[col][row];
            }
        }
        flat
    }

    /// Convert to Vec<FeatureRow> for IPC serialization (compatibility with existing API).
    pub fn to_feature_rows(&self) -> Vec<FeatureRow> {
        (0..self.n_rows).map(|i| FeatureRow {
            symbol: self.symbols[i].clone(),
            date: self.dates[i].clone(),
            features: FeatureValues {
                rsi_14: self.columns[0][i],
                ema_9: self.columns[1][i],
                ema_21: self.columns[2][i],
                atr_14: self.columns[3][i],
                bb_width: self.columns[4][i],
                returns_1d: self.columns[5][i],
                returns_5d: self.columns[6][i],
                returns_20d: self.columns[7][i],
                volume_ratio: self.columns[8][i],
                close: self.columns[9][i],
            },
        }).collect()
    }
}

/// Feature scores used inside ML prediction output.
#[derive(Debug, Clone, Serialize)]
pub struct MlFeatureScores {
    pub rsi: f64,
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub returns_5d: f64,
}

/// Prediction output from the hand-coded ML ensemble.
#[derive(Debug, Clone, Serialize)]
pub struct MlPrediction {
    pub symbol: String,
    pub prediction: f64,
    pub direction: String, // "long" | "short"
    pub confidence: f64,
    pub model: String,
    pub features: MlFeatureScores,
}

// ── Backtest job params (deserialized from IPC) ─────────────────────

/// Parameters for a local backtest invocation from the frontend.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BacktestParams {
    #[serde(default)]
    pub strategy_family: String,
    #[serde(default)]
    pub symbol_universe: Vec<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub region: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default)]
    pub date_start: String,
    #[serde(default)]
    pub date_end: String,
    #[serde(default)]
    pub entry_logic: Vec<String>,
    // Strategy-specific parameters
    #[serde(default = "default_stop_atr")]
    pub stop_loss_atr_mult: f64,
    #[serde(default = "default_tp_atr")]
    pub take_profit_atr_mult: f64,
    #[serde(default = "default_trail_pct")]
    pub trailing_stop_pct: f64,
    #[serde(default = "default_max_hold")]
    pub max_hold_days: usize,
    #[serde(default = "default_atr_period")]
    pub atr_period: usize,
    #[serde(default = "default_ema_fast")]
    pub ema_fast: usize,
    #[serde(default = "default_ema_slow")]
    pub ema_slow: usize,
    #[serde(default = "default_rsi_period")]
    pub rsi_period: usize,
    #[serde(default = "default_vol_sma_period")]
    pub volume_sma_period: usize,
    #[serde(default = "default_bbands_period")]
    pub bbands_period: usize,
    #[serde(default = "default_bbands_std")]
    pub bbands_std: f64,
    // Entry thresholds
    #[serde(default = "default_rsi_entry")]
    pub rsi_entry_threshold: f64,
    #[serde(default = "default_rsi_oversold")]
    pub rsi_oversold: f64,
    #[serde(default = "default_rsi_floor")]
    pub rsi_floor: f64,
    #[serde(default = "default_volume_multiplier")]
    pub volume_multiplier: f64,
    #[serde(default = "default_lookback_period")]
    pub lookback_period: usize,
    #[serde(default = "default_consolidation_days")]
    pub consolidation_days: usize,
    #[serde(default = "default_consolidation_range_pct")]
    pub consolidation_range_pct: f64,
    #[serde(default = "default_squeeze_threshold")]
    pub squeeze_threshold: f64,
    #[serde(default = "default_spread_lookback")]
    pub spread_lookback: usize,
    #[serde(default = "default_entry_zscore")]
    pub entry_zscore: f64,
    #[serde(default = "default_ranking_period")]
    pub ranking_period: usize,
    #[serde(default = "default_gap_threshold_pct")]
    pub gap_threshold_pct: f64,
    #[serde(default = "default_min_distance_from_sma")]
    pub min_distance_from_sma: f64,
}

fn default_direction() -> String { "long".to_string() }
fn default_stop_atr() -> f64 { 2.0 }
fn default_tp_atr() -> f64 { 4.0 }
fn default_trail_pct() -> f64 { 0.05 }
fn default_max_hold() -> usize { 30 }
fn default_atr_period() -> usize { 14 }
fn default_ema_fast() -> usize { 9 }
fn default_ema_slow() -> usize { 21 }
fn default_rsi_period() -> usize { 14 }
fn default_vol_sma_period() -> usize { 20 }
fn default_bbands_period() -> usize { 20 }
fn default_bbands_std() -> f64 { 2.0 }
fn default_rsi_entry() -> f64 { 55.0 }
fn default_rsi_oversold() -> f64 { 30.0 }
fn default_rsi_floor() -> f64 { 40.0 }
fn default_volume_multiplier() -> f64 { 1.5 }
fn default_lookback_period() -> usize { 20 }
fn default_consolidation_days() -> usize { 10 }
fn default_consolidation_range_pct() -> f64 { 0.05 }
fn default_squeeze_threshold() -> f64 { 0.02 }
fn default_spread_lookback() -> usize { 30 }
fn default_entry_zscore() -> f64 { 2.0 }
fn default_ranking_period() -> usize { 30 }
fn default_gap_threshold_pct() -> f64 { 0.03 }
fn default_min_distance_from_sma() -> f64 { 0.02 }

/// Parameters for a local scan invocation.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScanParams {
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_scan_type")]
    pub scan_type: String,
    #[serde(default)]
    pub entry_logic: Vec<String>,
    #[serde(default = "default_ema_fast")]
    pub ema_fast: usize,
    #[serde(default = "default_ema_slow")]
    pub ema_slow: usize,
    /// Return only the top N signals by confidence (0 = return all).
    #[serde(default)]
    pub top_n: usize,
    /// Minimum average volume on last bar to pass pre-filter (0 = no filter).
    #[serde(default)]
    pub min_volume: f64,
    /// Minimum price on last bar to pass pre-filter (0 = no filter).
    #[serde(default)]
    pub min_price: f64,
}

fn default_region() -> String { "us".to_string() }
fn default_scan_type() -> String { "momentum".to_string() }

/// Parameters for a local ML inference invocation.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MlParams {
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String { "ensemble_technical".to_string() }

/// Parameters for feature extraction.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeatureExtractionParams {
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default)]
    pub dataset: String,
}

// ── Aggregate result types for IPC responses ────────────────────────

/// Full backtest result returned to the frontend (aggregate across symbols).
#[derive(Debug, Clone, Serialize)]
pub struct BacktestJobResult {
    pub status: String,
    pub job_type: String,
    pub metrics: BacktestMetricsExt,
}

/// Extended metrics with symbol/strategy metadata.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestMetricsExt {
    #[serde(flatten)]
    pub base: BacktestMetrics,
    pub symbols_tested: usize,
    pub symbols_skipped: usize,
    pub strategy_family: String,
    pub date_window: String,
}

/// Scan result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ScanJobResult {
    pub status: String,
    pub job_type: String,
    pub results: Vec<ScanSignal>,
    pub symbols_scanned: usize,
    pub signals_found: usize,
    /// Wall-clock milliseconds for the scan.
    #[serde(default)]
    pub elapsed_ms: u64,
    /// Number of symbols eliminated by the pre-filter.
    #[serde(default)]
    pub pre_filtered: usize,
    /// Whether GPU batch dispatch was used.
    #[serde(default)]
    pub gpu_used: bool,
    /// Number of symbols served from indicator cache.
    #[serde(default)]
    pub cache_hits: usize,
}

/// ML inference result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct MlJobResult {
    pub status: String,
    pub job_type: String,
    pub predictions: Vec<MlPrediction>,
    pub model: String,
    pub symbols_processed: usize,
}

/// Feature extraction result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct FeatureJobResult {
    pub status: String,
    pub job_type: String,
    pub features_count: usize,
    pub dataset: String,
    pub features: Vec<FeatureRow>,
}
