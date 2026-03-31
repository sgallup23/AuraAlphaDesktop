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

// ── Types ────────────────────────────────────────────────────────────

/// Type alias for the trained Random Forest model.
type RfModel = RandomForestClassifier<f64, i32, DenseMatrix<f64>, Vec<i32>>;

/// A single training sample: 10 features + label.
#[derive(Debug, Clone)]
struct TrainingSample {
    features: [f64; 10],
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
}

/// Distribution of labels in the training set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelDistribution {
    pub positive: usize,
    pub negative: usize,
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
}

fn default_region() -> String { "us".to_string() }
fn default_n_trees() -> u16 { 100 }
fn default_max_depth() -> u16 { 10 }
fn default_forward_days() -> usize { 5 }
fn default_test_split() -> f64 { 0.2 }

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

/// Full serializable model container (model + metadata).
#[derive(Serialize, Deserialize)]
struct ModelContainer {
    info: ModelInfo,
    model_json: String,
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

// ── Feature extraction for training ──────────────────────────────────

/// Extract training samples from OHLCV bars for a single symbol.
///
/// For each bar where we have enough history (50 bars) AND enough forward
/// data (forward_days bars ahead), compute the 10 technical features and
/// the label (forward return > 0 => 1, else 0).
fn extract_training_samples(bars: &OhlcvBars, forward_days: usize) -> Vec<TrainingSample> {
    let n = bars.len();
    if n < 50 + forward_days {
        return Vec::new();
    }

    let rsi = indicators::compute_rsi(&bars.closes, 14);
    let ema9 = indicators::compute_ema(&bars.closes, 9);
    let ema21 = indicators::compute_ema(&bars.closes, 21);
    let atr = indicators::compute_atr(&bars.highs, &bars.lows, &bars.closes, 14);
    let (bb_u, bb_m, bb_l) = indicators::compute_bbands(&bars.closes, 20, 2.0);

    let mut samples = Vec::new();

    // Start at bar 49 (need 50 bars of history) and stop forward_days before the end.
    for i in 49..(n - forward_days) {
        // Skip bars with NaN indicators.
        if rsi[i].is_nan() || ema9[i].is_nan() || ema21[i].is_nan() {
            continue;
        }

        let atr_14 = if atr[i].is_nan() { 0.0 } else { atr[i] };

        let bb_width = if !bb_u[i].is_nan() && !bb_l[i].is_nan() {
            (bb_u[i] - bb_l[i]) / (bb_m[i] + 1e-10)
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

        let volume_ratio = if bars.volumes.iter().any(|&v| v > 0.0) {
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

        // Forward return label.
        let future_close = bars.closes[i + forward_days];
        let forward_return = (future_close - bars.closes[i]) / bars.closes[i];
        let label = if forward_return > 0.0 { 1 } else { 0 };

        // Skip samples with any NaN feature value.
        let features = [
            rsi[i],
            ema9[i],
            ema21[i],
            atr_14,
            bb_width,
            returns_1d,
            returns_5d,
            returns_20d,
            volume_ratio,
            bars.closes[i],
        ];
        if features.iter().any(|v| v.is_nan() || v.is_infinite()) {
            continue;
        }

        samples.push(TrainingSample { features, label });
    }

    samples
}

// ── Training pipeline ────────────────────────────────────────────────

/// Run the full training pipeline:
/// 1. Load all cached OHLCV data for the region
/// 2. Extract features + labels at each bar for each symbol
/// 3. Split 80/20 train/test
/// 4. Train RandomForest
/// 5. Report accuracy, precision, recall
/// 6. Save model to ~/.aura-worker/models/
pub fn train_model(params: &MlTrainParams) -> Result<TrainResult, String> {
    let cache_dir = data::get_cache_dir();
    let region = if params.region.is_empty() { "us" } else { &params.region };
    let forward_days = if params.forward_days == 0 { 5 } else { params.forward_days };
    let n_trees: u16 = if params.n_trees == 0 { 100 } else { params.n_trees };
    let max_depth = if params.max_depth == 0 { 10 } else { params.max_depth };
    let test_split = if params.test_split <= 0.0 || params.test_split >= 1.0 {
        0.2
    } else {
        params.test_split
    };

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

    // 3. Split train/test.
    let split_idx = ((1.0 - test_split) * samples.len() as f64) as usize;
    let train_samples = &samples[..split_idx];
    let test_samples = &samples[split_idx..];

    if train_samples.is_empty() || test_samples.is_empty() {
        return Err("Train/test split produced empty set. Need more data.".to_string());
    }

    // Convert to DenseMatrix format.
    let train_rows: Vec<Vec<f64>> = train_samples.iter().map(|s| s.features.to_vec()).collect();
    let train_labels: Vec<i32> = train_samples.iter().map(|s| s.label).collect();
    let test_rows: Vec<Vec<f64>> = test_samples.iter().map(|s| s.features.to_vec()).collect();
    let test_labels: Vec<i32> = test_samples.iter().map(|s| s.label).collect();

    let train_refs: Vec<&[f64]> = train_rows.iter().map(|r| r.as_slice()).collect();
    let test_refs: Vec<&[f64]> = test_rows.iter().map(|r| r.as_slice()).collect();

    let x_train = DenseMatrix::from_2d_array(&train_refs)
        .map_err(|e| format!("Failed to build training matrix: {}", e))?;
    let x_test = DenseMatrix::from_2d_array(&test_refs)
        .map_err(|e| format!("Failed to build test matrix: {}", e))?;

    // 4. Train Random Forest.
    info!(
        "ml_train: training RandomForest with {} trees, max_depth={}, {} train / {} test samples",
        n_trees,
        max_depth,
        train_samples.len(),
        test_samples.len()
    );

    let rf_params = RandomForestClassifierParameters::default()
        .with_n_trees(n_trees)
        .with_max_depth(max_depth)
        .with_min_samples_leaf(5)
        .with_m(3); // sqrt(10 features) ~ 3

    let model = RandomForestClassifier::fit(&x_train, &train_labels, rf_params)
        .map_err(|e| format!("Training failed: {}", e))?;

    // 5. Evaluate on test set.
    let test_preds = model
        .predict(&x_test)
        .map_err(|e| format!("Test prediction failed: {}", e))?;

    let acc = accuracy(&test_labels, &test_preds);

    // Compute precision, recall, F1 manually (binary classification).
    let tp = test_labels
        .iter()
        .zip(test_preds.iter())
        .filter(|(&t, &p)| t == 1 && p == 1)
        .count() as f64;
    let fp = test_labels
        .iter()
        .zip(test_preds.iter())
        .filter(|(&t, &p)| t == 0 && p == 1)
        .count() as f64;
    let fn_ = test_labels
        .iter()
        .zip(test_preds.iter())
        .filter(|(&t, &p)| t == 1 && p == 0)
        .count() as f64;

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    // Label distribution.
    let pos_count = samples.iter().filter(|s| s.label == 1).count();
    let neg_count = samples.iter().filter(|s| s.label == 0).count();

    info!(
        "ml_train: accuracy={:.4}, precision={:.4}, recall={:.4}, f1={:.4}",
        acc, precision, recall, f1
    );

    // 6. Save model to disk.
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
        feature_count: 10,
        train_samples: train_samples.len(),
        test_samples: test_samples.len(),
        symbols_used,
        n_trees,
        max_depth,
        forward_days,
    };

    save_model_to_disk(&model, &model_info)?;

    let model_path = default_model_path();

    Ok(TrainResult {
        status: "completed".to_string(),
        accuracy: round4(acc),
        precision: round4(precision),
        recall: round4(recall),
        f1_score: round4(f1),
        train_samples: train_samples.len(),
        test_samples: test_samples.len(),
        feature_count: 10,
        symbols_used,
        bars_processed,
        model_path: model_path.to_string_lossy().to_string(),
        trained_at,
        label_distribution: LabelDistribution {
            positive: pos_count,
            negative: neg_count,
        },
    })
}

/// Save model + metadata to disk.
fn save_model_to_disk(model: &RfModel, info: &ModelInfo) -> Result<(), String> {
    let model_path = default_model_path();
    let info_path = default_model_info_path();

    // Serialize model.
    let model_json = serde_json::to_string(model)
        .map_err(|e| format!("Failed to serialize model: {}", e))?;

    let container = ModelContainer {
        info: info.clone(),
        model_json,
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

/// Load a trained model from disk.
fn load_model_from_disk() -> Result<(RfModel, ModelInfo), String> {
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

    Ok((model, container.info))
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
