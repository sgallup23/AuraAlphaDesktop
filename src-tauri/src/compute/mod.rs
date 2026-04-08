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
//! - `ml_train`   — Local ML training via smartcore Random Forest (zero Python)
//! - `features`   — Technical feature extraction for ML pipeline
//! - `gpu`        — GPU-accelerated compute via wgpu (Vulkan/Metal/DX12)
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
#[cfg(feature = "gpu")]
#[path = "gpu.rs"]
pub mod gpu;

#[cfg(not(feature = "gpu"))]
#[path = "gpu_stub.rs"]
pub mod gpu;
pub mod hardware;
pub mod indicators;
pub mod metrics;
pub mod ml;
pub mod ml_train;
pub mod scanner;
pub mod types;

// Re-export primary types for convenience.
pub use types::{
    BacktestJobResult, BacktestMetrics, BacktestMetricsExt, BacktestParams, BacktestResult,
    FeatureExtractionParams, FeatureJobResult, FeatureMatrix, FeatureRow, FeatureValues,
    MlFeatureScores, MlJobResult, MlParams, MlPrediction, OhlcvBars, ScanJobResult, ScanParams,
    ScanSignal, Trade,
};

// Re-export ML helpers for use by scanner/backtest modules.
pub use ml_train::{extract_latest_features, predict_single};

// Re-export feature matrix extraction for Rust callers (ML, scanner).
pub use features::{extract_feature_matrix, execute_feature_extraction_matrix};

// Re-export IPC commands for registration in lib.rs.
pub use backtest::run_local_backtest;
pub use backtest::run_backtest_sweep;
pub use cache::{clear_data_cache, download_batch, ensure_local_data, get_cache_status};
pub use features::run_local_feature_extraction;
pub use ml::run_local_ml_inference;
pub use ml_train::{get_model_status, predict_with_local_model, train_local_model};
pub use scanner::{run_local_scan, run_streaming_scan, clear_scanner_cache};
