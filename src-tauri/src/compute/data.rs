//! Data loading — reads OHLCV bars from Parquet and CSV files.
//!
//! Parquet support uses Polars. CSV loading uses the Rust stdlib
//! for zero extra dependencies on the CSV path.

use super::types::OhlcvBars;
use polars::prelude::*;
use std::path::{Path, PathBuf};

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

/// Load OHLCV bars for a symbol from a Parquet file in the cache.
///
/// Expected path: `{cache_dir}/{region}/{symbol}.parquet`
/// The Parquet file must contain columns: date, open, high, low, close, volume.
pub fn load_bars(symbol: &str, region: &str, cache_dir: &Path) -> Option<OhlcvBars> {
    let path = cache_dir
        .join(region)
        .join(format!("{}.parquet", symbol.to_uppercase()));
    load_bars_from_parquet(&path)
}

/// Load OHLCV bars from a Parquet file at an arbitrary path.
fn load_bars_from_parquet(path: &Path) -> Option<OhlcvBars> {
    if !path.exists() {
        return None;
    }

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
}
