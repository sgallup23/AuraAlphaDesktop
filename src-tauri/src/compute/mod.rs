//! Rust-native computation engine for Aura Alpha Desktop.
//!
//! Zero Python dependencies — all indicator math, backtest simulation,
//! scanning, ML inference, feature extraction, data loading, and
//! hardware detection run in pure Rust.
//!
//! Modules:
//! - `backtest`   — Full backtest engine with 25+ entry conditions, ATR-based exits
//! - `cache`      — Offline data cache manager (seed, stats, download, eviction)
//! - `scanner`    — Signal scanner (momentum, reversal, breakout, volume)
//! - `ml`         — Hand-coded ML ensemble inference
//! - `features`   — Technical feature extraction for ML pipeline
//! - `indicators` — RSI, EMA, ATR, SMA, Bollinger Bands, OBV
//! - `data`       — OHLCV data loading from Parquet/CSV
//! - `metrics`    — Sharpe, Sortino, profit factor, drawdown
//! - `hardware`   — CPU/RAM/GPU detection
//! - `types`      — All shared data types
//! - `demo`       — Demo/test data generation

pub mod backtest;
pub mod cache;
pub mod data;
pub mod demo;
pub mod features;
pub mod hardware;
pub mod indicators;
pub mod metrics;
pub mod ml;
pub mod scanner;
pub mod types;

// Re-export primary types for convenience.
pub use types::{
    BacktestJobResult, BacktestMetrics, BacktestMetricsExt, BacktestParams, BacktestResult,
    FeatureExtractionParams, FeatureJobResult, FeatureRow, FeatureValues, MlFeatureScores,
    MlJobResult, MlParams, MlPrediction, OhlcvBars, ScanJobResult, ScanParams, ScanSignal, Trade,
};

// Re-export IPC commands for registration in lib.rs.
pub use backtest::run_local_backtest;
pub use cache::{ensure_local_data, get_cache_status};
pub use features::run_local_feature_extraction;
pub use ml::run_local_ml_inference;
pub use scanner::run_local_scan;
