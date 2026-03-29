"""
Cache Manager — Local parquet-based OHLCV data cache
=====================================================
Stores market data locally for instant offline access.
Handles: fetch → cache → partial updates → expiry.
"""
import logging
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional

log = logging.getLogger("cache-mgr")

CACHE_DIR = Path.home() / ".aura-worker" / "data"
CACHE_DIR.mkdir(parents=True, exist_ok=True)

# Cache expiry: data older than this is refreshed
CACHE_TTL_HOURS = 12  # weekday data refreshes twice daily
CACHE_TTL_WEEKEND_HOURS = 48  # weekend data stays longer

# Preload symbols for instant first-run experience
PRELOAD_SYMBOLS = ["SPY", "QQQ", "AAPL", "TSLA", "NVDA", "MSFT", "AMD", "META",
                   "BTC-USD", "ETH-USD", "SOL-USD", "AMZN", "GOOGL", "NFLX", "JPM"]


def _cache_path(symbol: str, timeframe: str = "1d", region: str = "us") -> Path:
    """Parquet file path for a cached symbol."""
    safe_sym = symbol.replace("/", "_").replace(":", "_").upper()
    return CACHE_DIR / region / f"{safe_sym}_{timeframe}.parquet"


def is_cached(symbol: str, timeframe: str = "1d", region: str = "us") -> bool:
    """Check if symbol has fresh cached data."""
    path = _cache_path(symbol, timeframe, region)
    if not path.exists():
        return False
    # Check age
    age_hours = (time.time() - path.stat().st_mtime) / 3600
    is_weekend = datetime.now().weekday() >= 5
    ttl = CACHE_TTL_WEEKEND_HOURS if is_weekend else CACHE_TTL_HOURS
    return age_hours < ttl


def read_cache(symbol: str, timeframe: str = "1d", region: str = "us",
               bars: int = 500) -> Optional[list]:
    """Read cached OHLCV data. Returns list of bar dicts or None."""
    path = _cache_path(symbol, timeframe, region)
    if not path.exists():
        return None
    try:
        import polars as pl
        df = pl.read_parquet(str(path))
        data = df.tail(bars).to_dicts()
        return data
    except Exception as e:
        log.warning("Cache read failed for %s: %s", symbol, e)
        return None


def write_cache(symbol: str, bars: list, timeframe: str = "1d",
                region: str = "us") -> bool:
    """Write OHLCV bars to parquet cache."""
    if not bars:
        return False
    try:
        import polars as pl
        path = _cache_path(symbol, timeframe, region)
        path.parent.mkdir(parents=True, exist_ok=True)
        pl.DataFrame(bars).write_parquet(str(path))
        return True
    except Exception as e:
        log.warning("Cache write failed for %s: %s", symbol, e)
        return False


def append_cache(symbol: str, new_bars: list, timeframe: str = "1d",
                 region: str = "us") -> bool:
    """Append new bars to existing cache (partial update)."""
    existing = read_cache(symbol, timeframe, region, bars=99999)
    if existing is None:
        return write_cache(symbol, new_bars, timeframe, region)

    # Deduplicate by date
    existing_dates = {b.get("date", b.get("timestamp", "")) for b in existing}
    merged = existing + [b for b in new_bars
                         if b.get("date", b.get("timestamp", "")) not in existing_dates]
    # Sort by date
    merged.sort(key=lambda b: b.get("date", b.get("timestamp", "")))
    return write_cache(symbol, merged, timeframe, region)


def cache_stats() -> dict:
    """Return cache statistics."""
    total_files = 0
    total_bytes = 0
    regions = {}
    for path in CACHE_DIR.rglob("*.parquet"):
        total_files += 1
        total_bytes += path.stat().st_size
        region = path.parent.name
        regions[region] = regions.get(region, 0) + 1

    return {
        "total_files": total_files,
        "total_mb": round(total_bytes / (1024 * 1024), 1),
        "regions": regions,
        "cache_dir": str(CACHE_DIR),
    }


def clear_expired() -> int:
    """Remove expired cache files. Returns count removed."""
    removed = 0
    now = time.time()
    for path in CACHE_DIR.rglob("*.parquet"):
        age_hours = (now - path.stat().st_mtime) / 3600
        if age_hours > 168:  # 7 days
            path.unlink()
            removed += 1
    return removed
