//! Offline data cache manager.
//!
//! Handles bundled sample data seeding, cache statistics, remote data
//! downloading, and stale-data eviction for the local OHLCV cache at
//! `~/.aura-worker/data/{region}/`.
//!
//! Optimizations:
//! - **Concurrent downloads**: `download_symbols_batch()` fetches multiple symbols
//!   in parallel using `tokio::spawn` + `join_all`, limited by a semaphore (20 max).
//! - **Incremental updates**: `download_symbol_data()` checks the last cached date
//!   and only requests bars after that date, appending rather than re-downloading.
//! - **Snappy compression**: All Parquet writes use snappy for smaller disk footprint.
//! - **LRU invalidation**: Downloads invalidate the in-memory LRU cache so the next
//!   `load_bars()` call picks up fresh data.

use super::data::{self, get_cache_dir};
use super::types::OhlcvBars;
use serde::Serialize;
use std::path::PathBuf;

/// Maximum concurrent HTTP downloads for batch operations.
const MAX_CONCURRENT_DOWNLOADS: usize = 20;

/// Statistics about the local data cache for a given region.
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub region: String,
    pub symbol_count: usize,
    pub total_size_bytes: u64,
    pub oldest_file_date: Option<String>,
    pub newest_file_date: Option<String>,
    /// Number of symbols currently in the in-memory LRU cache.
    pub in_memory_cached: usize,
}

// ── Core functions ──────────────────────────────────────────────────

/// Copy bundled CSV sample data into the user's local cache on first launch.
///
/// Source: `resources/sample_data/*.csv` (bundled with the app).
/// Destination: `~/.aura-worker/data/us/` (one CSV per symbol).
///
/// Files that already exist in the destination are skipped so we never
/// overwrite user-downloaded data.
///
/// Returns the number of files copied.
pub fn ensure_sample_data(app: &tauri::AppHandle) -> Result<usize, String> {
    let sample_dir = find_sample_data_dir(app)?;
    let dest_dir = get_cache_dir().join("us");

    // Create destination directory if it doesn't exist.
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create cache dir {:?}: {}", dest_dir, e))?;

    let mut copied = 0usize;

    let entries = std::fs::read_dir(&sample_dir)
        .map_err(|e| format!("Failed to read sample data dir {:?}: {}", sample_dir, e))?;

    for entry in entries.flatten() {
        let src = entry.path();
        // Only copy CSV files.
        if src.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }
        if let Some(filename) = src.file_name() {
            let dest = dest_dir.join(filename);
            if dest.exists() {
                // Already seeded -- skip.
                continue;
            }
            std::fs::copy(&src, &dest)
                .map_err(|e| format!("Failed to copy {:?} -> {:?}: {}", src, dest, e))?;
            copied += 1;
        }
    }

    if copied > 0 {
        log::info!(
            "Seeded {} sample data files into {:?}",
            copied,
            dest_dir
        );
    }

    Ok(copied)
}

/// Compute statistics about the local data cache for a region.
pub fn get_cache_stats(region: &str) -> CacheStats {
    let cache_dir = get_cache_dir().join(region);

    let mut symbol_count = 0usize;
    let mut total_size_bytes = 0u64;
    let mut oldest: Option<std::time::SystemTime> = None;
    let mut newest: Option<std::time::SystemTime> = None;

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "parquet" && ext != "csv" {
                continue;
            }

            symbol_count += 1;

            if let Ok(meta) = std::fs::metadata(&path) {
                total_size_bytes += meta.len();

                if let Ok(modified) = meta.modified() {
                    oldest = Some(match oldest {
                        Some(prev) if modified < prev => modified,
                        Some(prev) => prev,
                        None => modified,
                    });
                    newest = Some(match newest {
                        Some(prev) if modified > prev => modified,
                        Some(prev) => prev,
                        None => modified,
                    });
                }
            }
        }
    }

    CacheStats {
        region: region.to_string(),
        symbol_count,
        total_size_bytes,
        oldest_file_date: oldest.map(system_time_to_iso),
        newest_file_date: newest.map(system_time_to_iso),
        in_memory_cached: data::cache_len(),
    }
}

// ── Single-symbol download (with incremental update) ───────────────

/// Download OHLCV data for a symbol from the auraalpha.cc API and save
/// as a Parquet file in the local cache.
///
/// **Incremental update**: If a Parquet file already exists with data up to
/// some date, the API is called with `&after={last_date}` so only new bars
/// are downloaded. The new bars are appended and deduped, not re-downloaded.
///
/// **Snappy compression**: Output Parquet uses snappy for smaller footprint.
///
/// Endpoint: `GET https://auraalpha.cc/api/data/ohlcv/{symbol}?region={region}[&after={date}]`
/// The response is expected to be JSON with arrays: dates, opens, highs,
/// lows, closes, volumes.
pub async fn download_symbol_data(symbol: &str, region: &str) -> Result<(), String> {
    let cache_dir = get_cache_dir();

    // Check for existing data for incremental update.
    let last_date = data::get_last_cached_date(symbol, region, &cache_dir);

    let mut url = format!(
        "https://auraalpha.cc/api/data/ohlcv/{}?region={}",
        symbol.to_uppercase(),
        region
    );

    // Append after parameter for incremental download.
    if let Some(ref date) = last_date {
        url.push_str(&format!("&after={}", date));
        log::info!(
            "cache: incremental download for {} (after {})",
            symbol.to_uppercase(),
            date
        );
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Download failed for {}: {}", symbol, e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "API returned {} for {}",
            resp.status(),
            symbol
        ));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error for {}: {}", symbol, e))?;

    // Extract arrays from the JSON response.
    let dates = json_str_array(&body, "dates")
        .ok_or_else(|| format!("Missing 'dates' array for {}", symbol))?;
    let opens = json_f64_array(&body, "opens")
        .ok_or_else(|| format!("Missing 'opens' array for {}", symbol))?;
    let highs = json_f64_array(&body, "highs")
        .ok_or_else(|| format!("Missing 'highs' array for {}", symbol))?;
    let lows = json_f64_array(&body, "lows")
        .ok_or_else(|| format!("Missing 'lows' array for {}", symbol))?;
    let closes = json_f64_array(&body, "closes")
        .ok_or_else(|| format!("Missing 'closes' array for {}", symbol))?;
    let volumes = json_f64_array(&body, "volumes")
        .ok_or_else(|| format!("Missing 'volumes' array for {}", symbol))?;

    // If the API returned no new bars (already up to date), skip write.
    if dates.is_empty() {
        log::info!("cache: {} already up to date, no new bars", symbol.to_uppercase());
        return Ok(());
    }

    let new_bars = OhlcvBars {
        dates,
        opens,
        highs,
        lows,
        closes,
        volumes,
    };

    // Use incremental append if we had existing data, otherwise write fresh.
    if last_date.is_some() {
        let total = data::append_bars_to_parquet(symbol, region, &cache_dir, &new_bars)?;
        log::info!(
            "cache: incremental update for {} -> {} total bars",
            symbol.to_uppercase(),
            total
        );
    } else {
        // Fresh download -- write directly with snappy compression.
        let dest_dir = cache_dir.join(region);
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("Failed to create cache dir: {}", e))?;

        let path = dest_dir.join(format!("{}.parquet", symbol.to_uppercase()));
        let mut df = data::bars_to_dataframe(&new_bars)?;
        data::write_parquet_snappy(&path, &mut df)?;

        // Invalidate LRU cache.
        data::invalidate_cache_entry(symbol, region);

        log::info!(
            "cache: downloaded and cached {} -> {:?} ({} bars)",
            symbol.to_uppercase(),
            path,
            new_bars.len()
        );
    }

    Ok(())
}

// ── Batch download (concurrent with semaphore) ─────────────────────

/// Result of a batch download operation.
#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Download OHLCV data for multiple symbols concurrently.
///
/// Uses `tokio::spawn` + `futures::future::join_all` with a semaphore to
/// limit concurrency to `MAX_CONCURRENT_DOWNLOADS` (20) parallel HTTP fetches.
/// This is dramatically faster than sequential downloads for large symbol lists.
///
/// Each download uses incremental updates when possible (see `download_symbol_data`).
pub async fn download_symbols_batch(
    symbols: &[String],
    region: &str,
) -> BatchDownloadResult {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let total = symbols.len();
    if total == 0 {
        return BatchDownloadResult {
            total: 0,
            succeeded: 0,
            failed: 0,
            errors: vec![],
        };
    }

    log::info!(
        "cache: batch downloading {} symbols (max {} concurrent)",
        total,
        MAX_CONCURRENT_DOWNLOADS
    );

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));

    let mut handles = Vec::with_capacity(total);

    for symbol in symbols {
        let sym = symbol.clone();
        let rgn = region.to_string();
        let sem = Arc::clone(&semaphore);

        let handle = tokio::spawn(async move {
            // Acquire semaphore permit (blocks if at max concurrency).
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| format!("Semaphore closed for {}", sym))?;

            download_symbol_data(&sym, &rgn).await
        });

        handles.push((symbol.clone(), handle));
    }

    // Await all downloads.
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();

    for (symbol, handle) in handles {
        match handle.await {
            Ok(Ok(())) => succeeded += 1,
            Ok(Err(e)) => {
                failed += 1;
                errors.push(format!("{}: {}", symbol, e));
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: task panicked: {}", symbol, e));
            }
        }
    }

    // Log errors but don't truncate the list.
    if failed > 0 {
        log::warn!(
            "cache: batch download completed: {}/{} succeeded, {} failed",
            succeeded,
            total,
            failed
        );
        for err in &errors {
            log::warn!("cache: download error: {}", err);
        }
    } else {
        log::info!(
            "cache: batch download completed: all {} symbols succeeded",
            total
        );
    }

    BatchDownloadResult {
        total,
        succeeded,
        failed,
        errors,
    }
}

/// Remove data files (parquet and csv) older than `max_age_days` from all
/// region directories under the cache. Returns the count of files removed.
pub fn evict_stale_data(max_age_days: u32) -> usize {
    let cache_dir = get_cache_dir();
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(max_age_days as u64 * 86_400);

    let mut removed = 0usize;

    // Walk each region sub-directory.
    let regions = match std::fs::read_dir(&cache_dir) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    for region_entry in regions.flatten() {
        if !region_entry.path().is_dir() {
            continue;
        }

        if let Ok(files) = std::fs::read_dir(region_entry.path()) {
            for file_entry in files.flatten() {
                let path = file_entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext != "parquet" && ext != "csv" {
                    continue;
                }

                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        if modified < cutoff {
                            if std::fs::remove_file(&path).is_ok() {
                                removed += 1;
                                log::info!("Evicted stale file: {:?}", path);
                            }
                        }
                    }
                }
            }
        }
    }

    if removed > 0 {
        // Also clear the in-memory cache since files were removed.
        data::clear_bars_cache();
        log::info!("Evicted {} stale data files (>{} days old)", removed, max_age_days);
    }

    removed
}

// ── IPC commands ────────────────────────────────────────────────────

/// IPC command: get cache statistics for display in the UI.
#[tauri::command]
pub fn get_cache_status(region: Option<String>) -> CacheStats {
    let region = region.as_deref().unwrap_or("us");
    get_cache_stats(region)
}

/// IPC command: ensure bundled sample data is copied to the local cache.
///
/// Safe to call multiple times -- skips files that already exist.
#[tauri::command]
pub fn ensure_local_data(app: tauri::AppHandle) -> Result<usize, String> {
    ensure_sample_data(&app)
}

/// IPC command: download OHLCV data for a batch of symbols concurrently.
///
/// Uses up to 20 parallel HTTP connections for maximum throughput.
/// Each symbol uses incremental updates when existing data is available.
#[tauri::command]
pub async fn download_batch(
    symbols: Vec<String>,
    region: Option<String>,
) -> Result<BatchDownloadResult, String> {
    let region = region.unwrap_or_else(|| "us".to_string());
    Ok(download_symbols_batch(&symbols, &region).await)
}

/// IPC command: clear the in-memory OHLCV data cache.
///
/// Useful when the user manually manages data files or wants to free memory.
#[tauri::command]
pub fn clear_data_cache() -> String {
    let before = data::cache_len();
    data::clear_bars_cache();
    format!("Cleared {} cached entries", before)
}

// ── Internal helpers ────────────────────────────────────────────────

/// Resolve the path to the bundled sample data directory.
///
/// Mirrors the logic from `demo.rs` -- checks Tauri resource dir,
/// adjacent to executable, and development source tree.
fn find_sample_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    // 1. Tauri resource resolver (production builds).
    if let Ok(resource_path) = app.path().resource_dir() {
        let candidate = resource_path.join("resources").join("sample_data");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        let candidate2 = resource_path.join("sample_data");
        if candidate2.is_dir() {
            return Ok(candidate2);
        }
    }

    // 2. Adjacent to executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("resources").join("sample_data");
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }

    // 3. Development fallback -- source tree.
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("sample_data");
    if dev_path.is_dir() {
        return Ok(dev_path);
    }

    Err("Could not locate bundled sample data directory.".to_string())
}

/// Convert a SystemTime to an ISO-8601 date string (YYYY-MM-DD).
fn system_time_to_iso(t: std::time::SystemTime) -> String {
    let duration = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let dt = chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default();
    dt.format("%Y-%m-%d").to_string()
}

/// Extract a JSON array of strings from a value by key.
fn json_str_array(val: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    val.get(key)?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
}

/// Extract a JSON array of f64 from a value by key.
fn json_f64_array(val: &serde_json::Value, key: &str) -> Option<Vec<f64>> {
    val.get(key)?
        .as_array()?
        .iter()
        .map(|v| v.as_f64())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cache_stats_empty() {
        let stats = get_cache_stats("nonexistent_region_test");
        assert_eq!(stats.symbol_count, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert!(stats.oldest_file_date.is_none());
        assert!(stats.newest_file_date.is_none());
    }

    #[test]
    fn test_evict_stale_data_empty() {
        // Should return 0 when nothing to evict.
        let removed = evict_stale_data(9999);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_system_time_to_iso() {
        use std::time::{Duration, UNIX_EPOCH};
        // 2025-01-01 00:00:00 UTC = 1735689600 seconds since epoch
        let t = UNIX_EPOCH + Duration::from_secs(1_735_689_600);
        let iso = system_time_to_iso(t);
        assert_eq!(iso, "2025-01-01");
    }

    #[test]
    fn test_json_str_array() {
        let val = serde_json::json!({ "dates": ["2025-01-01", "2025-01-02"] });
        let arr = json_str_array(&val, "dates").unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "2025-01-01");
    }

    #[test]
    fn test_json_f64_array() {
        let val = serde_json::json!({ "prices": [100.0, 101.5, 99.8] });
        let arr = json_f64_array(&val, "prices").unwrap();
        assert_eq!(arr.len(), 3);
        assert!((arr[1] - 101.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_json_array_missing() {
        let val = serde_json::json!({ "other": 42 });
        assert!(json_str_array(&val, "dates").is_none());
        assert!(json_f64_array(&val, "prices").is_none());
    }

    #[test]
    fn test_batch_download_result_default() {
        let result = BatchDownloadResult {
            total: 0,
            succeeded: 0,
            failed: 0,
            errors: vec![],
        };
        assert_eq!(result.total, 0);
    }
}
