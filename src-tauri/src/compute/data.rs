//! Data loading — reads OHLCV bars from Parquet and CSV files.
//!
//! Optimized pipeline:
//! - **In-memory LRU cache**: Recently loaded symbols are kept in memory so
//!   repeated scanner/backtest runs (every 60s) avoid disk I/O entirely.
//!   Uses the `lru` crate for O(1) get/put with automatic eviction.
//! - **Parquet-first**: Parquet files are loaded via Polars `scan_parquet` for
//!   predicate pushdown and column pruning. CSV is a fallback for sample data.
//! - **Prefetch**: `prefetch_symbols()` loads a batch of symbols into the LRU
//!   cache in parallel (rayon) before compute jobs begin.
//! - **Memory-mapped reads**: Polars' `scan_parquet` uses memory-mapped I/O
//!   internally -- the OS page cache handles hot files efficiently.
//! - **Incremental updates**: `get_last_cached_date()` + `append_bars_to_parquet()`
//!   allow downloading only new bars instead of full history re-downloads.
//! - **Snappy compression**: All Parquet writes use snappy for smaller footprint.

use super::types::OhlcvBars;
use lru::LruCache;
use polars::prelude::*;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

// ── Global in-memory LRU cache ─────────────────────────────────────

/// Maximum number of symbols to keep in the in-memory LRU cache.
/// At ~12KB per symbol (250 bars * 6 columns * 8 bytes), 2000 entries ~ 24 MB.
const LRU_CAPACITY: usize = 2000;

/// Cached entry: the parsed bars plus the file's last-modified timestamp
/// so we can detect stale data without re-parsing.
struct CachedBars {
    bars: OhlcvBars,
    file_modified: SystemTime,
}

/// Global in-memory LRU cache keyed by canonical file path.
/// The `lru::LruCache` gives us O(1) get/put with automatic eviction of the
/// least recently used entry when capacity is exceeded -- no manual eviction.
static BARS_CACHE: std::sync::LazyLock<Mutex<LruCache<PathBuf, CachedBars>>> =
    std::sync::LazyLock::new(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(LRU_CAPACITY).unwrap(),
        ))
    });

/// Resolve the cache directory for market data.
///
/// Priority: `AURA_CACHE_DIR` env var, then `~/.aura-worker/data`.
pub fn get_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AURA_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aura-worker")
        .join("data")
}

// ── Primary load function (LRU cache + disk) ───────────────────────

/// Load OHLCV bars for a symbol, checking the in-memory LRU cache first.
///
/// Lookup order:
/// 1. In-memory LRU cache (zero I/O). If the file's mtime hasn't changed,
///    return the cached `OhlcvBars` instantly.
/// 2. Parquet file at `{cache_dir}/{region}/{SYMBOL}.parquet`.
/// 3. CSV file at `{cache_dir}/{region}/{SYMBOL}.csv` (fallback for sample data).
///
/// On a cache miss from disk, the result is inserted into the LRU cache so
/// subsequent calls are instant. The LRU evicts the oldest entry automatically
/// when capacity is exceeded.
pub fn load_bars(symbol: &str, region: &str, cache_dir: &Path) -> Option<OhlcvBars> {
    let sym_upper = symbol.to_uppercase();
    let region_dir = cache_dir.join(region);

    // Try Parquet first (primary format), then CSV (sample data fallback).
    let parquet_path = region_dir.join(format!("{}.parquet", sym_upper));
    if parquet_path.exists() {
        return load_bars_cached(&parquet_path);
    }

    let csv_path = region_dir.join(format!("{}.csv", sym_upper));
    if csv_path.exists() {
        return load_bars_from_csv(&csv_path);
    }

    None
}

/// Check the in-memory LRU cache first; fall back to disk parse on miss.
/// Validates cache entries against the file's mtime to detect stale data.
fn load_bars_cached(path: &Path) -> Option<OhlcvBars> {
    if !path.exists() {
        return None;
    }

    let file_modified = std::fs::metadata(path).ok()?.modified().ok()?;

    // Fast path: check LRU cache under lock.
    {
        let mut cache = BARS_CACHE.lock().ok()?;
        if let Some(entry) = cache.get(path) {
            if entry.file_modified == file_modified {
                // Cache hit -- LruCache::get() automatically promotes to most-recent.
                return Some(entry.bars.clone());
            }
            // File changed on disk -- remove stale entry.
            cache.pop(path);
        }
    }

    // Slow path: parse from disk.
    let bars = load_bars_from_parquet_uncached(path)?;

    // Insert into LRU cache. LruCache handles eviction automatically.
    {
        if let Ok(mut cache) = BARS_CACHE.lock() {
            cache.put(
                path.to_path_buf(),
                CachedBars {
                    bars: bars.clone(),
                    file_modified,
                },
            );
        }
    }

    Some(bars)
}

/// Load OHLCV bars from a Parquet file at an arbitrary path (no caching).
///
/// Uses Polars `scan_parquet` which leverages memory-mapped I/O internally.
fn load_bars_from_parquet_uncached(path: &Path) -> Option<OhlcvBars> {
    let df = LazyFrame::scan_parquet(path, Default::default())
        .ok()?
        .collect()
        .ok()?;

    dataframe_to_bars(&df)
}

/// Load OHLCV bars from a CSV file.
///
/// Expected columns: date, open, high, low, close, volume.
/// (Column names are case-insensitive; the loader tries common variants.)
pub fn load_bars_from_csv(path: &Path) -> Option<OhlcvBars> {
    if !path.exists() {
        return None;
    }

    let df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some(path.to_path_buf()))
        .ok()?
        .finish()
        .ok()?;

    dataframe_to_bars(&df)
}

// ── LRU cache management ───────────────────────────────────────────

/// Invalidate a single symbol in the LRU cache (e.g., after download/append).
pub fn invalidate_cache_entry(symbol: &str, region: &str) {
    let cache_dir = get_cache_dir();
    let path = cache_dir
        .join(region)
        .join(format!("{}.parquet", symbol.to_uppercase()));
    if let Ok(mut cache) = BARS_CACHE.lock() {
        cache.pop(&path);
    }
}

/// Clear the entire in-memory LRU cache. Called when data is re-downloaded
/// or during shutdown to free memory.
pub fn clear_bars_cache() {
    if let Ok(mut cache) = BARS_CACHE.lock() {
        let count = cache.len();
        cache.clear();
        if count > 0 {
            log::info!("data: cleared {count} entries from in-memory LRU cache");
        }
    }
}

/// Return the number of entries currently in the LRU cache.
pub fn cache_len() -> usize {
    BARS_CACHE.lock().map(|c| c.len()).unwrap_or(0)
}

// ── Prefetch (parallel bulk load into LRU cache) ──────────────────

/// Pre-load a batch of symbols into the in-memory LRU cache in parallel.
///
/// Called by the worker before processing a batch of jobs: symbols are loaded
/// from disk concurrently via rayon, then inserted into the shared LRU cache.
/// Subsequent `load_bars()` calls for these symbols hit the cache instantly.
///
/// Skips symbols already in the cache to avoid redundant I/O.
/// Returns the number of symbols successfully prefetched from disk.
pub fn prefetch_symbols(symbols: &[String], region: &str, cache_dir: &Path) -> usize {
    use rayon::prelude::*;

    if symbols.is_empty() {
        return 0;
    }

    // Determine which symbols need loading (not already cached).
    let to_load: Vec<PathBuf> = {
        let cache = match BARS_CACHE.lock() {
            Ok(c) => c,
            Err(_) => return 0,
        };
        symbols
            .iter()
            .filter_map(|sym| {
                let path = cache_dir
                    .join(region)
                    .join(format!("{}.parquet", sym.to_uppercase()));
                if path.exists() && !cache.contains(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect()
    };

    if to_load.is_empty() {
        return 0;
    }

    // Load from disk in parallel with rayon.
    let loaded: Vec<(PathBuf, CachedBars)> = to_load
        .par_iter()
        .filter_map(|path| {
            let file_modified = std::fs::metadata(path).ok()?.modified().ok()?;
            let bars = load_bars_from_parquet_uncached(path)?;
            Some((
                path.clone(),
                CachedBars {
                    bars,
                    file_modified,
                },
            ))
        })
        .collect();

    let count = loaded.len();

    // Insert all into the LRU cache under a single lock acquisition.
    if let Ok(mut cache) = BARS_CACHE.lock() {
        for (path, entry) in loaded {
            cache.put(path, entry);
        }
    }

    if count > 0 {
        log::info!(
            "data: prefetched {} symbols for region '{}' into LRU cache",
            count,
            region
        );
    }

    count
}

// ── Incremental update helpers ─────────────────────────────────────

/// Get the last date in a cached Parquet file for incremental updates.
///
/// If the cache has data up to "2026-04-07", only bars from "2026-04-08"
/// onward need to be downloaded. Returns `None` if the file doesn't exist.
pub fn get_last_cached_date(symbol: &str, region: &str, cache_dir: &Path) -> Option<String> {
    let path = cache_dir
        .join(region)
        .join(format!("{}.parquet", symbol.to_uppercase()));

    if !path.exists() {
        return None;
    }

    // First check the in-memory cache.
    if let Ok(cache) = BARS_CACHE.lock() {
        if let Some(entry) = cache.peek(&path) {
            return entry.bars.dates.last().cloned();
        }
    }

    // Fall back to reading from disk.
    let df = LazyFrame::scan_parquet(&path, Default::default())
        .ok()?
        .collect()
        .ok()?;

    let dates = extract_date_column(&df)?;
    dates.last().cloned()
}

/// Append new bars to an existing Parquet file (incremental update).
///
/// If the file doesn't exist, writes the new bars as a fresh file.
/// If it does exist, reads existing data, appends new bars (deduped by date),
/// sorts by date, and rewrites with snappy compression.
///
/// Also invalidates the LRU cache entry so next `load_bars()` picks up new data.
/// Returns the total number of bars in the file after the append.
pub fn append_bars_to_parquet(
    symbol: &str,
    region: &str,
    cache_dir: &Path,
    new_bars: &OhlcvBars,
) -> Result<usize, String> {
    let dest_dir = cache_dir.join(region);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let path = dest_dir.join(format!("{}.parquet", symbol.to_uppercase()));

    // Build DataFrame from new bars.
    let new_df = bars_to_dataframe(new_bars)?;

    // If existing file, merge and deduplicate.
    let final_df = if path.exists() {
        let existing_df = LazyFrame::scan_parquet(&path, Default::default())
            .map_err(|e| format!("Failed to scan existing parquet: {}", e))?
            .collect()
            .map_err(|e| format!("Failed to collect existing parquet: {}", e))?;

        // Vertical concat, then deduplicate by date (keep last = new data wins).
        concat(
            [existing_df.clone().lazy(), new_df.clone().lazy()],
            UnionArgs::default(),
        )
        .map_err(|e| format!("Concat error: {}", e))?
        .unique(Some(vec!["date".into()]), UniqueKeepStrategy::Last)
        .sort(["date"], Default::default())
        .collect()
        .map_err(|e| format!("Dedup/sort error: {}", e))?
    } else {
        new_df
    };

    let total_bars = final_df.height();

    // Write with snappy compression for smaller footprint.
    write_parquet_snappy(&path, &mut final_df.clone())?;

    // Invalidate LRU cache entry so next load picks up new data.
    invalidate_cache_entry(symbol, region);

    log::info!(
        "data: appended bars for {} ({} total bars)",
        symbol.to_uppercase(),
        total_bars
    );

    Ok(total_bars)
}

// ── Parquet write helper with snappy compression ──────────────────

/// Write a DataFrame to Parquet with snappy compression.
///
/// All Parquet writes go through this function to ensure consistent
/// compression settings across the codebase.
pub fn write_parquet_snappy(path: &Path, df: &mut DataFrame) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("Failed to create file {:?}: {}", path, e))?;

    ParquetWriter::new(file)
        .with_compression(ParquetCompression::Snappy)
        .finish(df)
        .map_err(|e| format!("Parquet write error: {}", e))?;

    Ok(())
}

// ── Convert OhlcvBars to/from DataFrame ────────────────────────────

/// Convert OhlcvBars into a Polars DataFrame (for writing to Parquet).
pub fn bars_to_dataframe(bars: &OhlcvBars) -> Result<DataFrame, String> {
    DataFrame::new(vec![
        Column::new("date".into(), &bars.dates),
        Column::new("open".into(), &bars.opens),
        Column::new("high".into(), &bars.highs),
        Column::new("low".into(), &bars.lows),
        Column::new("close".into(), &bars.closes),
        Column::new("volume".into(), &bars.volumes),
    ])
    .map_err(|e| format!("DataFrame error: {}", e))
}

/// List symbols that have cached Parquet data for a region.
///
/// Returns sorted symbol names (e.g. `["AAPL", "MSFT", ...]`).
pub fn list_available_symbols(region: &str, cache_dir: &Path) -> Vec<String> {
    let dir = cache_dir.join(region);
    let mut symbols = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("parquet") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    symbols.push(stem.to_uppercase());
                }
            }
        }
    }

    symbols.sort();
    symbols
}

// ── Internal helpers ─────────────────────────────────────────────────

/// Convert a Polars DataFrame with OHLCV columns into our `OhlcvBars` type.
///
/// Tries common column name variants (lowercase, Title Case, UPPER).
fn dataframe_to_bars(df: &DataFrame) -> Option<OhlcvBars> {
    let dates = extract_date_column(df)?;
    let opens = extract_f64_column(df, &["open", "Open", "OPEN"])?;
    let highs = extract_f64_column(df, &["high", "High", "HIGH"])?;
    let lows = extract_f64_column(df, &["low", "Low", "LOW"])?;
    let closes = extract_f64_column(df, &["close", "Close", "CLOSE"])?;
    let volumes = extract_f64_column(df, &["volume", "Volume", "VOLUME"])?;

    Some(OhlcvBars {
        dates,
        opens,
        highs,
        lows,
        closes,
        volumes,
    })
}

/// Extract a date/datetime column as ISO-8601 strings.
fn extract_date_column(df: &DataFrame) -> Option<Vec<String>> {
    let names = ["date", "Date", "DATE", "datetime", "Datetime", "timestamp"];
    for name in &names {
        if let Ok(col) = df.column(name) {
            let series = col.as_materialized_series();
            // Try to get string representation for each element.
            let dates: Vec<String> = (0..series.len())
                .map(|i| {
                    let val = series.get(i).ok();
                    match val {
                        Some(av) => format!("{}", av),
                        None => String::new(),
                    }
                })
                .collect();
            return Some(dates);
        }
    }
    None
}

/// Extract a numeric column as `Vec<f64>`, trying multiple name variants.
fn extract_f64_column(df: &DataFrame, names: &[&str]) -> Option<Vec<f64>> {
    for name in names {
        if let Ok(col) = df.column(name) {
            let series = col.as_materialized_series();
            let casted = series.cast(&DataType::Float64).ok()?;
            let ca = casted.f64().ok()?;
            let values: Vec<f64> = ca.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect();
            return Some(values);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cache_dir_default() {
        // Without AURA_CACHE_DIR set, should end in .aura-worker/data.
        std::env::remove_var("AURA_CACHE_DIR");
        let dir = get_cache_dir();
        assert!(dir.ends_with(".aura-worker/data") || dir.ends_with(".aura-worker\\data"));
    }

    #[test]
    fn test_get_cache_dir_env() {
        std::env::set_var("AURA_CACHE_DIR", "/tmp/test-aura-cache");
        let dir = get_cache_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-aura-cache"));
        std::env::remove_var("AURA_CACHE_DIR");
    }

    #[test]
    fn test_list_available_symbols_empty() {
        let symbols = list_available_symbols("us", Path::new("/tmp/nonexistent-aura-test"));
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_load_bars_missing_file() {
        let result = load_bars("NOSYMBOL", "us", Path::new("/tmp/nonexistent-aura-test"));
        assert!(result.is_none());
    }

    #[test]
    fn test_load_csv_missing_file() {
        let result = load_bars_from_csv(Path::new("/tmp/nonexistent.csv"));
        assert!(result.is_none());
    }

    #[test]
    fn test_lru_cache_operations() {
        // Clear cache first.
        clear_bars_cache();
        assert_eq!(cache_len(), 0);

        // Invalidating a nonexistent key should not panic.
        invalidate_cache_entry("TEST", "us");
    }

    #[test]
    fn test_prefetch_empty() {
        let count = prefetch_symbols(&[], "us", Path::new("/tmp/nonexistent-aura-test"));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_prefetch_nonexistent() {
        let syms = vec!["NOSYMBOL1".to_string(), "NOSYMBOL2".to_string()];
        let count = prefetch_symbols(&syms, "us", Path::new("/tmp/nonexistent-aura-test"));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_bars_to_dataframe() {
        let bars = OhlcvBars {
            dates: vec!["2025-01-01".to_string(), "2025-01-02".to_string()],
            opens: vec![100.0, 101.0],
            highs: vec![102.0, 103.0],
            lows: vec![99.0, 100.0],
            closes: vec![101.0, 102.0],
            volumes: vec![1000.0, 1100.0],
        };
        let df = bars_to_dataframe(&bars);
        assert!(df.is_ok());
        let df = df.unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 6);
    }
}
