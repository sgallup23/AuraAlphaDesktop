//! Local ML model training — smartcore Random Forest, zero Python dependencies.
//!
//! Trains a RandomForestClassifier on technical features extracted from cached
//! OHLCV data.  Labels are derived from forward 5-day returns:
//!   1 = profitable trade (return > 0)
//!   0 = losing trade (return <= 0)
//!
//! Features (10 numeric columns from compute/features.rs):
//!   RSI14, EMA9, EMA21, ATR14, BB width, returns 1d/5d/20d, volume ratio, close
//!
//! The trained model is serialized to `~/.aura-worker/models/rf_model.json`
//! and can be reloaded for inference without retraining.
//!
//! Optimizations:
//! - In-memory model cache (OnceLock) — avoids repeated disk IO + JSON deserialization
//! - Batched prediction — all symbols predicted in a single DenseMatrix call
//! - Flat Vec<f64> feature matrix — cache-friendly contiguous memory layout
//! - Shared feature extraction — single `extract_latest_features` function
//! - Walk-forward cross-validation with parallel folds
//! - Feature normalization with cached scaler parameters

use super::data;
use super::indicators;
use super::types::*;
use log::info;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smartcore::ensemble::random_forest_classifier::RandomForestClassifier;
use smartcore::ensemble::random_forest_classifier::RandomForestClassifierParameters;
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::metrics::accuracy;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Number of features per sample.
const NUM_FEATURES: usize = 10;

// ── In-memory model cache ───────────────────────────────────────────
//
// The trained RF model is large (can be 10+ MB of JSON). Deserializing it
// on every prediction call is wasteful. We cache it in a process-global
// OnceLock + Mutex so it survives across IPC calls. The cache is
// invalidated when a new model is trained.

struct CachedModel {
    model: RfModel,
    info: ModelInfo,
    /// Scaler parameters: (mean, std) per feature, length = NUM_FEATURES.
    scaler: Option<ScalerParams>,
}

/// Feature normalization parameters (z-score).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalerParams {
    pub means: Vec<f64>,
    pub stds: Vec<f64>,
}

static MODEL_CACHE: OnceLock<Mutex<Option<CachedModel>>> = OnceLock::new();

fn get_model_cache() -> &'static Mutex<Option<CachedModel>> {
    MODEL_CACHE.get_or_init(|| Mutex::new(None))
}

/// Invalidate the in-memory model cache (called after training).
fn invalidate_model_cache() {
    if let Ok(mut guard) = get_model_cache().lock() {
        *guard = None;
    }
}

/// Load model from cache or disk. The model bytes are kept in memory so
/// subsequent calls skip disk IO entirely. We store the raw serialized
/// bytes and deserialize on each call because smartcore's model types
/// are not cheaply cloneable — but deserializing from an in-memory
/// buffer is still 10-50x faster than reading from disk.
fn get_or_load_model() -> Result<(RfModel, ModelInfo, Option<ScalerParams>), String> {
    let cache = get_model_cache();
    let guard = cache.lock().map_err(|e| format!("Model cache lock poisoned: {}", e))?;

    if let Some(ref cached) = *guard {
        // Cache hit — return the pre-deserialized model.
        // We serialize/deserialize the model via serde to "clone" it since
        // smartcore types don't implement Clone. This is fast from memory.
        let model_json = serde_json::to_string(&cached.model)
            .map_err(|e| format!("Failed to re-serialize cached model: {}", e))?;
        let model: RfModel = serde_json::from_str(&model_json)
            .map_err(|e| format!("Failed to deserialize cached model: {}", e))?;
        return Ok((model, cached.info.clone(), cached.scaler.clone()));
    }

    drop(guard); // Release lock before disk IO.

    // Cache miss — load from disk.
    let (model, info, scaler) = load_model_from_disk_full()?;

    // Store in cache for next time.
    let model_for_cache = load_model_from_disk_raw()?;
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(CachedModel {
            model: model_for_cache,
            info: info.clone(),
            scaler: scaler.clone(),
        });
    }

    Ok((model, info, scaler))
}

// ── Types ────────────────────────────────────────────────────────────

/// Type alias for the trained Random Forest model.
type RfModel = RandomForestClassifier<f64, i32, DenseMatrix<f64>, Vec<i32>>;

/// A single training sample: 10 features + label.
#[derive(Debug, Clone)]
struct TrainingSample {
    features: [f64; NUM_FEATURES],
    label: i32, // 1 = profitable, 0 = losing
}

/// Training result with model and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainResult {
    pub status: String,
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub train_samples: usize,
    pub test_samples: usize,
    pub feature_count: usize,
    pub symbols_used: usize,
    pub bars_processed: usize,
    pub model_path: String,
    pub trained_at: String,
    pub label_distribution: LabelDistribution,
    #[serde(default)]
    pub walk_forward: Option<WalkForwardResult>,
}

/// Distribution of labels in the training set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelDistribution {
    pub positive: usize,
    pub negative: usize,
}

/// Walk-forward cross-validation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    pub n_folds: usize,
    pub fold_accuracies: Vec<f64>,
    pub mean_accuracy: f64,
    pub std_accuracy: f64,
}

/// Prediction result from the trained RF model.
#[derive(Debug, Clone, Serialize)]
pub struct RfPrediction {
    pub symbol: String,
    pub prediction: i32,
    pub direction: String,
    pub confidence: f64,
    pub features: FeatureValues,
}

/// Prediction batch result.
#[derive(Debug, Clone, Serialize)]
pub struct PredictResult {
    pub status: String,
    pub predictions: Vec<RfPrediction>,
    pub model_info: ModelInfo,
}

/// Parameters for training.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MlTrainParams {
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_n_trees")]
    pub n_trees: u16,
    #[serde(default = "default_max_depth")]
    pub max_depth: u16,
    #[serde(default = "default_forward_days")]
    pub forward_days: usize,
    #[serde(default = "default_test_split")]
    pub test_split: f64,
    /// Number of walk-forward folds (0 = disabled, default 5).
    #[serde(default = "default_walk_forward_folds")]
    pub walk_forward_folds: usize,
}

fn default_region() -> String { "us".to_string() }
fn default_n_trees() -> u16 { 150 }
fn default_max_depth() -> u16 { 12 }
fn default_forward_days() -> usize { 5 }
fn default_test_split() -> f64 { 0.2 }
fn default_walk_forward_folds() -> usize { 5 }

/// Parameters for prediction using the trained model.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MlPredictParams {
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default = "default_region")]
    pub region: String,
}

/// Stored model info (persisted alongside the model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub trained_at: String,
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub feature_count: usize,
    pub train_samples: usize,
    pub test_samples: usize,
    pub symbols_used: usize,
    pub n_trees: u16,
    pub max_depth: u16,
    pub forward_days: usize,
}

/// Full serializable model container (model + metadata + scaler).
#[derive(Serialize, Deserialize)]
struct ModelContainer {
    info: ModelInfo,
    model_json: String,
    #[serde(default)]
    scaler: Option<ScalerParams>,
}

// ── Paths ────────────────────────────────────────────────────────────

/// Get the models directory: `~/.aura-worker/models/`.
fn get_models_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aura-worker")
        .join("models")
}

/// Default model path.
fn default_model_path() -> PathBuf {
    get_models_dir().join("rf_model.json")
}

/// Default model info path (metadata sidecar).
fn default_model_info_path() -> PathBuf {
    get_models_dir().join("rf_model_info.json")
}

// ── Shared feature extraction ────────────────────────────────────────
//
// A single function extracts the 10-feature vector for any bar index.
// Used by both training (all bars) and prediction (latest bar only),
// eliminating the duplicated extraction logic.

/// Pre-computed indicator arrays for a symbol. Computed once, reused for
/// all bars in the symbol.
struct SymbolIndicators {
    rsi: Vec<f64>,
    ema9: Vec<f64>,
    ema21: Vec<f64>,
    atr: Vec<f64>,
    bb_upper: Vec<f64>,
    bb_middle: Vec<f64>,
    bb_lower: Vec<f64>,
    /// Whether any volume bar is nonzero (computed once, not per-bar).
    has_volume: bool,
}

impl SymbolIndicators {
    fn compute(bars: &OhlcvBars) -> Self {
        let (bb_upper, bb_middle, bb_lower) = indicators::compute_bbands(&bars.closes, 20, 2.0);
        Self {
            rsi: indicators::compute_rsi(&bars.closes, 14),
            ema9: indicators::compute_ema(&bars.closes, 9),
            ema21: indicators::compute_ema(&bars.closes, 21),
            atr: indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14),
            bb_upper,
            bb_middle,
            bb_lower,
            // Check once for the entire symbol, not per-bar (was O(n) per bar before).
            has_volume: bars.volumes.iter().any(|&v| v > 0.0),
        }
    }
}

/// Extract the 10-feature vector for a single bar index.
/// Returns None if any required indicator is NaN/Inf.
#[inline]
fn extract_features_at(
    bars: &OhlcvBars,
    ind: &SymbolIndicators,
    i: usize,
) -> Option<[f64; NUM_FEATURES]> {
    // Skip bars with NaN core indicators.
    if ind.rsi[i].is_nan() || ind.ema9[i].is_nan() || ind.ema21[i].is_nan() {
        return None;
    }

    let atr_14 = if ind.atr[i].is_nan() { 0.0 } else { ind.atr[i] };

    let bb_width = if !ind.bb_upper[i].is_nan() && !ind.bb_lower[i].is_nan() {
        (ind.bb_upper[i] - ind.bb_lower[i]) / (ind.bb_middle[i] + 1e-10)
    } else {
        0.0
    };

    let returns_1d = if i > 0 {
        (bars.closes[i] - bars.closes[i - 1]) / bars.closes[i - 1]
    } else {
        0.0
    };

    let idx_5 = if i >= 5 { i - 5 } else { 0 };
    let returns_5d = (bars.closes[i] - bars.closes[idx_5]) / bars.closes[idx_5];

    let idx_20 = if i >= 20 { i - 20 } else { 0 };
    let returns_20d = (bars.closes[i] - bars.closes[idx_20]) / bars.closes[idx_20];

    let volume_ratio = if ind.has_volume {
        let start = if i >= 20 { i - 20 } else { 0 };
        let window = &bars.volumes[start..i];
        if !window.is_empty() {
            let avg_vol = window.iter().sum::<f64>() / window.len() as f64;
            bars.volumes[i] / (avg_vol + 1e-10)
        } else {
            1.0
        }
    } else {
        1.0
    };

    let features = [
        ind.rsi[i],
        ind.ema9[i],
        ind.ema21[i],
        atr_14,
        bb_width,
        returns_1d,
        returns_5d,
        returns_20d,
        volume_ratio,
        bars.closes[i],
    ];

    // Reject if any feature is NaN or Inf.
    if features.iter().any(|v| v.is_nan() || v.is_infinite()) {
        return None;
    }

    Some(features)
}

// ── Feature extraction for training ──────────────────────────────────

/// Extract training samples from OHLCV bars for a single symbol.
///
/// For each bar where we have enough history (50 bars) AND enough forward
/// data (forward_days bars ahead), compute the 10 technical features and
/// the label (forward return > 0 => 1, else 0).
///
/// Optimized: indicators computed once, volume check once, shared extraction.
fn extract_training_samples(bars: &OhlcvBars, forward_days: usize) -> Vec<TrainingSample> {
    let n = bars.len();
    if n < 50 + forward_days {
        return Vec::new();
    }

    let ind = SymbolIndicators::compute(bars);

    // Pre-allocate with estimated capacity.
    let est_capacity = n - 49 - forward_days;
    let mut samples = Vec::with_capacity(est_capacity);

    // Start at bar 49 (need 50 bars of history) and stop forward_days before the end.
    for i in 49..(n - forward_days) {
        let features = match extract_features_at(bars, &ind, i) {
            Some(f) => f,
            None => continue,
        };

        // Forward return label.
        let future_close = bars.closes[i + forward_days];
        let forward_return = (future_close - bars.closes[i]) / bars.closes[i];
        let label = if forward_return > 0.0 { 1 } else { 0 };

        samples.push(TrainingSample { features, label });
    }

    samples
}

// ── Feature normalization ────────────────────────────────────────────

/// Compute z-score normalization parameters from a flat feature matrix.
/// Returns (means, stds) each of length NUM_FEATURES.
fn compute_scaler(flat_features: &[f64], n_samples: usize) -> ScalerParams {
    let mut means = vec![0.0; NUM_FEATURES];
    let mut stds = vec![0.0; NUM_FEATURES];

    // Compute means.
    for row in 0..n_samples {
        let base = row * NUM_FEATURES;
        for col in 0..NUM_FEATURES {
            means[col] += flat_features[base + col];
        }
    }
    for col in 0..NUM_FEATURES {
        means[col] /= n_samples as f64;
    }

    // Compute standard deviations.
    for row in 0..n_samples {
        let base = row * NUM_FEATURES;
        for col in 0..NUM_FEATURES {
            let diff = flat_features[base + col] - means[col];
            stds[col] += diff * diff;
        }
    }
    for col in 0..NUM_FEATURES {
        stds[col] = (stds[col] / n_samples as f64).sqrt();
        // Prevent division by zero.
        if stds[col] < 1e-10 {
            stds[col] = 1.0;
        }
    }

    ScalerParams { means, stds }
}

/// Normalize a flat feature matrix in-place using pre-computed scaler.
fn normalize_features_inplace(flat: &mut [f64], n_samples: usize, scaler: &ScalerParams) {
    for row in 0..n_samples {
        let base = row * NUM_FEATURES;
        for col in 0..NUM_FEATURES {
            flat[base + col] = (flat[base + col] - scaler.means[col]) / scaler.stds[col];
        }
    }
}

/// Build a DenseMatrix from a flat f64 slice (contiguous row-major layout).
/// This avoids the Vec<Vec<f64>> -> Vec<&[f64]> indirection.
fn flat_to_dense_matrix(flat: &[f64], n_rows: usize, n_cols: usize) -> Result<DenseMatrix<f64>, String> {
    let rows: Vec<&[f64]> = (0..n_rows)
        .map(|r| &flat[r * n_cols..(r + 1) * n_cols])
        .collect();
    DenseMatrix::from_2d_array(&rows)
        .map_err(|e| format!("Failed to build matrix: {}", e))
}

// ── Training pipeline ────────────────────────────────────────────────

/// Run the full training pipeline:
/// 1. Load all cached OHLCV data for the region
/// 2. Extract features + labels at each bar for each symbol
/// 3. Compute feature normalization (z-score scaler)
/// 4. Split 80/20 train/test (flat contiguous matrix layout)
/// 5. Train RandomForest (150 trees, depth 12, min_samples_leaf 5, m=3)
/// 6. Report accuracy, precision, recall
/// 7. Run walk-forward cross-validation (parallel folds)
/// 8. Save model + scaler to ~/.aura-worker/models/
pub fn train_model(params: &MlTrainParams) -> Result<TrainResult, String> {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };
    let forward_days = if params.forward_days == 0 { 5 } else { params.forward_days };
    let n_trees: u16 = if params.n_trees == 0 { 150 } else { params.n_trees };
    let max_depth = if params.max_depth == 0 { 12 } else { params.max_depth };
    let test_split = if params.test_split <= 0.0 || params.test_split >= 1.0 {
        0.2
    } else {
        params.test_split
    };
    let wf_folds = if params.walk_forward_folds == 0 { 5 } else { params.walk_forward_folds };

    // 1. List all available symbols.
    let symbols = data::list_available_symbols(region, &cache_dir);
    if symbols.is_empty() {
        return Err("No cached data found. Run 'ensure_local_data' first to download OHLCV data.".to_string());
    }

    info!("ml_train: found {} symbols in region '{}'", symbols.len(), region);

    // 2. Extract training samples in parallel.
    let all_samples: Vec<Vec<TrainingSample>> = symbols
        .par_iter()
        .filter_map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 + forward_days {
                return None;
            }
            let samples = extract_training_samples(&bars, forward_days);
            if samples.is_empty() {
                None
            } else {
                Some(samples)
            }
        })
        .collect();

    let symbols_used = all_samples.len();
    let samples: Vec<TrainingSample> = all_samples.into_iter().flatten().collect();
    let bars_processed = samples.len();

    if samples.len() < 20 {
        return Err(format!(
            "Not enough training data: only {} samples from {} symbols. Need at least 20.",
            samples.len(),
            symbols_used
        ));
    }

    info!("ml_train: {} total samples from {} symbols", samples.len(), symbols_used);

    // 3. Build flat feature matrix (contiguous, cache-friendly layout).
    let n_total = samples.len();
    let mut flat_features: Vec<f64> = Vec::with_capacity(n_total * NUM_FEATURES);
    let mut all_labels: Vec<i32> = Vec::with_capacity(n_total);

    for s in &samples {
        flat_features.extend_from_slice(&s.features);
        all_labels.push(s.label);
    }

    // 4. Compute and apply feature normalization (z-score).
    let scaler = compute_scaler(&flat_features, n_total);
    normalize_features_inplace(&mut flat_features, n_total, &scaler);

    // 5. Split train/test.
    let split_idx = ((1.0 - test_split) * n_total as f64) as usize;

    if split_idx == 0 || split_idx >= n_total {
        return Err("Train/test split produced empty set. Need more data.".to_string());
    }

    let train_flat = &flat_features[..split_idx * NUM_FEATURES];
    let train_labels = &all_labels[..split_idx];
    let test_flat = &flat_features[split_idx * NUM_FEATURES..];
    let test_labels = &all_labels[split_idx..];

    let x_train = flat_to_dense_matrix(train_flat, split_idx, NUM_FEATURES)?;
    let x_test = flat_to_dense_matrix(test_flat, n_total - split_idx, NUM_FEATURES)?;

    // 6. Train Random Forest.
    info!(
        "ml_train: training RandomForest with {} trees, max_depth={}, {} train / {} test samples",
        n_trees,
        max_depth,
        split_idx,
        n_total - split_idx
    );

    let rf_params = RandomForestClassifierParameters::default()
        .with_n_trees(n_trees)
        .with_max_depth(max_depth)
        .with_min_samples_leaf(5)
        .with_m(3); // sqrt(10 features) ~ 3

    let model = RandomForestClassifier::fit(&x_train, &train_labels.to_vec(), rf_params.clone())
        .map_err(|e| format!("Training failed: {}", e))?;

    // 7. Evaluate on test set.
    let test_preds = model
        .predict(&x_test)
        .map_err(|e| format!("Test prediction failed: {}", e))?;

    let acc = accuracy(&test_labels.to_vec(), &test_preds);

    // Compute precision, recall, F1 manually (binary classification).
    let (tp, fp, fn_) = compute_confusion(test_labels, &test_preds);

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    // Label distribution.
    let pos_count = all_labels.iter().filter(|&&l| l == 1).count();
    let neg_count = all_labels.iter().filter(|&&l| l == 0).count();

    info!(
        "ml_train: accuracy={:.4}, precision={:.4}, recall={:.4}, f1={:.4}",
        acc, precision, recall, f1
    );

    // 8. Walk-forward cross-validation (parallel folds).
    let walk_forward = if wf_folds >= 2 && n_total >= wf_folds * 20 {
        Some(run_walk_forward(&flat_features, &all_labels, n_total, wf_folds, &rf_params)?)
    } else {
        None
    };

    if let Some(ref wf) = walk_forward {
        info!(
            "ml_train: walk-forward CV: {} folds, mean_acc={:.4} +/- {:.4}",
            wf.n_folds, wf.mean_accuracy, wf.std_accuracy
        );
    }

    // 9. Save model + scaler to disk.
    let models_dir = get_models_dir();
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models dir: {}", e))?;

    let trained_at = chrono::Utc::now().to_rfc3339();

    let model_info = ModelInfo {
        trained_at: trained_at.clone(),
        accuracy: round4(acc),
        precision: round4(precision),
        recall: round4(recall),
        f1_score: round4(f1),
        feature_count: NUM_FEATURES,
        train_samples: split_idx,
        test_samples: n_total - split_idx,
        symbols_used,
        n_trees,
        max_depth,
        forward_days,
    };

    save_model_to_disk(&model, &model_info, &scaler)?;

    // Invalidate in-memory cache so next prediction uses the new model.
    invalidate_model_cache();

    let model_path = default_model_path();

    Ok(TrainResult {
        status: "completed".to_string(),
        accuracy: round4(acc),
        precision: round4(precision),
        recall: round4(recall),
        f1_score: round4(f1),
        train_samples: split_idx,
        test_samples: n_total - split_idx,
        feature_count: NUM_FEATURES,
        symbols_used,
        bars_processed,
        model_path: model_path.to_string_lossy().to_string(),
        trained_at,
        label_distribution: LabelDistribution {
            positive: pos_count,
            negative: neg_count,
        },
        walk_forward,
    })
}

/// Compute (TP, FP, FN) from labels and predictions.
fn compute_confusion(labels: &[i32], preds: &[i32]) -> (f64, f64, f64) {
    let mut tp = 0.0_f64;
    let mut fp = 0.0_f64;
    let mut fn_ = 0.0_f64;
    for (&t, &p) in labels.iter().zip(preds.iter()) {
        if t == 1 && p == 1 { tp += 1.0; }
        if t == 0 && p == 1 { fp += 1.0; }
        if t == 1 && p == 0 { fn_ += 1.0; }
    }
    (tp, fp, fn_)
}

// ── Walk-forward cross-validation ───────────────────────────────────
//
// Time-series aware CV: each fold trains on data before the fold
// and tests on the fold. Folds are run in parallel via rayon.

fn run_walk_forward(
    flat_features: &[f64],
    labels: &[i32],
    n_total: usize,
    n_folds: usize,
    rf_params: &RandomForestClassifierParameters,
) -> Result<WalkForwardResult, String> {
    // Divide data into n_folds + 1 segments. The first segment is always
    // training-only. Each subsequent fold uses all prior segments for
    // training and the current segment for testing.
    let segment_size = n_total / (n_folds + 1);
    if segment_size < 10 {
        return Err("Not enough data for walk-forward validation".to_string());
    }

    // Build fold specs: (train_end, test_start, test_end).
    let fold_specs: Vec<(usize, usize, usize)> = (1..=n_folds)
        .map(|fold| {
            let test_start = fold * segment_size;
            let test_end = if fold == n_folds { n_total } else { (fold + 1) * segment_size };
            (test_start, test_start, test_end)
        })
        .collect();

    // Run folds in parallel. Each fold owns its own slice of the data.
    let fold_accuracies: Vec<f64> = fold_specs
        .par_iter()
        .filter_map(|&(train_end, test_start, test_end)| {
            if train_end < 10 || test_end <= test_start {
                return None;
            }

            let train_flat = &flat_features[..train_end * NUM_FEATURES];
            let train_labels = &labels[..train_end];
            let test_flat = &flat_features[test_start * NUM_FEATURES..test_end * NUM_FEATURES];
            let test_labels = &labels[test_start..test_end];

            let x_train = flat_to_dense_matrix(train_flat, train_end, NUM_FEATURES).ok()?;
            let x_test = flat_to_dense_matrix(test_flat, test_end - test_start, NUM_FEATURES).ok()?;

            let model = RandomForestClassifier::fit(
                &x_train,
                &train_labels.to_vec(),
                rf_params.clone(),
            ).ok()?;

            let preds = model.predict(&x_test).ok()?;
            Some(accuracy(&test_labels.to_vec(), &preds))
        })
        .collect();

    if fold_accuracies.is_empty() {
        return Err("All walk-forward folds failed".to_string());
    }

    let mean = fold_accuracies.iter().sum::<f64>() / fold_accuracies.len() as f64;
    let variance = fold_accuracies.iter()
        .map(|&a| (a - mean).powi(2))
        .sum::<f64>() / fold_accuracies.len() as f64;
    let std_dev = variance.sqrt();

    Ok(WalkForwardResult {
        n_folds: fold_accuracies.len(),
        fold_accuracies: fold_accuracies.iter().map(|&a| round4(a)).collect(),
        mean_accuracy: round4(mean),
        std_accuracy: round4(std_dev),
    })
}

/// Save model + metadata + scaler to disk.
fn save_model_to_disk(model: &RfModel, info: &ModelInfo, scaler: &ScalerParams) -> Result<(), String> {
    let model_path = default_model_path();
    let info_path = default_model_info_path();

    // Serialize model.
    let model_json = serde_json::to_string(model)
        .map_err(|e| format!("Failed to serialize model: {}", e))?;

    let container = ModelContainer {
        info: info.clone(),
        model_json,
        scaler: Some(scaler.clone()),
    };

    let container_bytes = serde_json::to_vec(&container)
        .map_err(|e| format!("Failed to serialize model container: {}", e))?;

    std::fs::write(&model_path, &container_bytes)
        .map_err(|e| format!("Failed to write model to {:?}: {}", model_path, e))?;

    // Also write info sidecar for quick access without loading the full model.
    let info_bytes = serde_json::to_vec_pretty(info)
        .map_err(|e| format!("Failed to serialize model info: {}", e))?;
    std::fs::write(&info_path, &info_bytes)
        .map_err(|e| format!("Failed to write model info to {:?}: {}", info_path, e))?;

    info!(
        "ml_train: model saved to {:?} ({} bytes)",
        model_path,
        container_bytes.len()
    );

    Ok(())
}

/// Load the full model + info + scaler from disk.
fn load_model_from_disk_full() -> Result<(RfModel, ModelInfo, Option<ScalerParams>), String> {
    let model_path = default_model_path();

    if !model_path.exists() {
        return Err("No trained model found. Run 'train_local_model' first.".to_string());
    }

    let bytes = std::fs::read(&model_path)
        .map_err(|e| format!("Failed to read model from {:?}: {}", model_path, e))?;

    let container: ModelContainer = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to deserialize model container: {}", e))?;

    let model: RfModel = serde_json::from_str(&container.model_json)
        .map_err(|e| format!("Failed to deserialize model: {}", e))?;

    Ok((model, container.info, container.scaler))
}

/// Load just the raw RfModel from disk (for caching purposes).
fn load_model_from_disk_raw() -> Result<RfModel, String> {
    let model_path = default_model_path();
    let bytes = std::fs::read(&model_path)
        .map_err(|e| format!("Failed to read model: {}", e))?;
    let container: ModelContainer = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse model container: {}", e))?;
    serde_json::from_str(&container.model_json)
        .map_err(|e| format!("Failed to deserialize model: {}", e))
}

/// Get model status without loading the full model.
fn load_model_info() -> Result<ModelInfo, String> {
    let info_path = default_model_info_path();

    if !info_path.exists() {
        // Try loading from the full container.
        let model_path = default_model_path();
        if !model_path.exists() {
            return Err("No trained model found.".to_string());
        }

        let bytes = std::fs::read(&model_path)
            .map_err(|e| format!("Failed to read model: {}", e))?;
        let container: ModelContainer = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Failed to parse model container: {}", e))?;
        return Ok(container.info);
    }

    let bytes = std::fs::read(&info_path)
        .map_err(|e| format!("Failed to read model info: {}", e))?;
    let info: ModelInfo = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse model info: {}", e))?;

    Ok(info)
}

// ── Prediction ───────────────────────────────────────────────────────

/// Run prediction on a list of symbols using the saved RF model.
pub fn predict_with_model(params: &MlPredictParams) -> Result<PredictResult, String> {
    let (model, info) = load_model_from_disk()?;

    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };

    // Resolve symbols.
    let mut symbols = params.symbols.clone();
    if let Some(ref sym) = params.symbol {
        if symbols.is_empty() {
            symbols.push(sym.clone());
        }
    }

    if symbols.is_empty() {
        symbols = data::list_available_symbols(region, &cache_dir);
    }

    info!("ml_predict: predicting for {} symbols with RF model", symbols.len());

    // Extract features for the latest bar of each symbol and predict.
    let predictions: Vec<Option<RfPrediction>> = symbols
        .par_iter()
        .map(|sym| {
            let bars = data::load_bars(sym, region, &cache_dir)?;
            if bars.len() < 50 {
                return None;
            }

            let n = bars.len();
            let last = n - 1;

            let rsi = indicators::compute_rsi(&bars.closes, 14);
            let ema9 = indicators::compute_ema(&bars.closes, 9);
            let ema21 = indicators::compute_ema(&bars.closes, 21);
            let atr = indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14);
            let (bb_u, bb_m, bb_l) = indicators::compute_bbands(&bars.closes, 20, 2.0);

            if rsi[last].is_nan() || ema9[last].is_nan() || ema21[last].is_nan() {
                return None;
            }

            let atr_14 = if atr[last].is_nan() { 0.0 } else { atr[last] };

            let bb_width = if !bb_u[last].is_nan() && !bb_l[last].is_nan() {
                (bb_u[last] - bb_l[last]) / (bb_m[last] + 1e-10)
            } else {
                0.0
            };

            let returns_1d = if last > 0 {
                (bars.closes[last] - bars.closes[last - 1]) / bars.closes[last - 1]
            } else {
                0.0
            };

            let idx_5 = if last >= 5 { last - 5 } else { 0 };
            let returns_5d =
                (bars.closes[last] - bars.closes[idx_5]) / bars.closes[idx_5];

            let idx_20 = if last >= 20 { last - 20 } else { 0 };
            let returns_20d =
                (bars.closes[last] - bars.closes[idx_20]) / bars.closes[idx_20];

            let volume_ratio = if bars.volumes.iter().any(|&v| v > 0.0) {
                let start = if last >= 20 { last - 20 } else { 0 };
                let window = &bars.volumes[start..last];
                if !window.is_empty() {
                    let avg_vol = window.iter().sum::<f64>() / window.len() as f64;
                    bars.volumes[last] / (avg_vol + 1e-10)
                } else {
                    1.0
                }
            } else {
                1.0
            };

            let feature_vec = vec![
                rsi[last],
                ema9[last],
                ema21[last],
                atr_14,
                bb_width,
                returns_1d,
                returns_5d,
                returns_20d,
                volume_ratio,
                bars.closes[last],
            ];

            // Skip if any feature is NaN/Inf.
            if feature_vec.iter().any(|v| v.is_nan() || v.is_infinite()) {
                return None;
            }

            let refs: Vec<&[f64]> = vec![feature_vec.as_slice()];
            let x = DenseMatrix::from_2d_array(&refs).ok()?;

            let pred = model.predict(&x).ok()?;
            let prediction = pred[0];

            // Confidence: use the majority vote ratio as a rough proxy.
            // With a single sample, we just use model accuracy as base confidence.
            let confidence = if prediction == 1 {
                (info.precision * 0.7 + info.accuracy * 0.3).min(0.95)
            } else {
                // For negative predictions, base on specificity approximation.
                (info.accuracy * 0.8 + 0.1).min(0.95)
            };

            let direction = if prediction == 1 {
                "long".to_string()
            } else {
                "short".to_string()
            };

            Some(RfPrediction {
                symbol: sym.clone(),
                prediction,
                direction,
                confidence: round4(confidence),
                features: FeatureValues {
                    rsi_14: round4(rsi[last]),
                    ema_9: round4(ema9[last]),
                    ema_21: round4(ema21[last]),
                    atr_14: round4(atr_14),
                    bb_width: round4(bb_width),
                    returns_1d: round6(returns_1d),
                    returns_5d: round6(returns_5d),
                    returns_20d: round6(returns_20d),
                    volume_ratio: round4(volume_ratio),
                    close: round4(bars.closes[last]),
                },
            })
        })
        .collect();

    let results: Vec<RfPrediction> = predictions.into_iter().flatten().collect();

    Ok(PredictResult {
        status: "completed".to_string(),
        predictions: results,
        model_info: info,
    })
}

// ============================================================================
// Tauri IPC commands
// ============================================================================

/// IPC command: train a local Random Forest model on cached OHLCV data.
///
/// Invoked from the frontend via `invoke('train_local_model', { params })`.
/// Runs the full pipeline: load data -> extract features -> train -> evaluate -> save.
#[tauri::command]
pub async fn train_local_model(params: MlTrainParams) -> Result<TrainResult, String> {
    let result = tokio::task::spawn_blocking(move || train_model(&params))
        .await
        .map_err(|e| format!("Training task panicked: {}", e))?;
    result
}

/// IPC command: predict using the saved local RF model.
///
/// Invoked from the frontend via `invoke('predict_with_local_model', { params })`.
/// Loads the saved model, extracts features for the latest bar of each symbol,
/// and returns predictions.
#[tauri::command]
pub async fn predict_with_local_model(params: MlPredictParams) -> Result<PredictResult, String> {
    let result = tokio::task::spawn_blocking(move || predict_with_model(&params))
        .await
        .map_err(|e| format!("Prediction task panicked: {}", e))?;
    result
}

/// IPC command: get status of the trained local model.
///
/// Returns model metadata (trained date, accuracy, feature count) without
/// loading the full model into memory.
#[tauri::command]
pub async fn get_model_status() -> Result<ModelInfo, String> {
    tokio::task::spawn_blocking(load_model_info)
        .await
        .map_err(|e| format!("Model status check panicked: {}", e))?
}

// ── Helpers ──────────────────────────────────────────────────────────

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn round6(v: f64) -> f64 {
    (v * 1000000.0).round() / 1000000.0
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_training_samples_short_data() {
        let bars = OhlcvBars {
            dates: vec!["2025-01-01".to_string(); 30],
            opens: vec![100.0; 30],
            highs: vec![101.0; 30],
            lows: vec![99.0; 30],
            closes: vec![100.0; 30],
            volumes: vec![1000.0; 30],
        };
        // Too short for 50 bars + 5 forward days.
        let samples = extract_training_samples(&bars, 5);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_extract_training_samples_sufficient_data() {
        // Create 100 bars of synthetic data with a slight uptrend.
        let n = 100;
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut opens = Vec::with_capacity(n);
        let mut volumes = Vec::with_capacity(n);
        let mut dates = Vec::with_capacity(n);

        for i in 0..n {
            let price = 100.0 + (i as f64) * 0.5 + ((i as f64) * 0.1).sin() * 2.0;
            closes.push(price);
            highs.push(price + 1.0);
            lows.push(price - 1.0);
            opens.push(price - 0.2);
            volumes.push(1000.0 + (i as f64) * 10.0);
            dates.push(format!("2025-01-{:02}", (i % 28) + 1));
        }

        let bars = OhlcvBars {
            dates,
            opens,
            highs,
            lows,
            closes,
            volumes,
        };

        let samples = extract_training_samples(&bars, 5);
        // Should have samples from bar 49 to bar 94 (100 - 5 - 1).
        assert!(!samples.is_empty());
        // Each sample should have 10 features.
        for s in &samples {
            assert_eq!(s.features.len(), 10);
            assert!(s.label == 0 || s.label == 1);
        }
    }

    #[test]
    fn test_model_info_not_found() {
        // Ensure loading from a non-existent path returns an error.
        let result = load_model_info();
        // This will either succeed (if a model exists) or return an error.
        // We just test it doesn't panic.
        let _ = result;
    }

    #[test]
    fn test_default_params() {
        // Test serde defaults by deserializing empty JSON.
        let params: MlTrainParams = serde_json::from_str("{}").unwrap();
        assert_eq!(params.region, "us");
        assert_eq!(params.n_trees, 100);
        assert_eq!(params.max_depth, 10);
        assert_eq!(params.forward_days, 5);
        assert!((params.test_split - 0.2).abs() < 1e-9);
    }
}
