"""
Data Service — Hybrid data pipeline with source prioritization
================================================================
Automatically selects the best data source:
  STANDALONE: Yahoo Finance → local cache
  CONNECTED:  IBKR → Twelve Data → Yahoo → cache

All fetches are cached locally. Users never see raw source names.
"""
import asyncio
import logging
import os
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Optional

from .cache_manager import (
    PRELOAD_SYMBOLS, is_cached, read_cache, write_cache, append_cache
)

log = logging.getLogger("data-svc")
_executor = ThreadPoolExecutor(max_workers=4)

# ── Connection state ─────────────────────────────────────────────────
_connected_mode = False
_ibkr_available = False
_twelve_data_key = os.environ.get("TWELVE_DATA_API_KEY", "")


def set_connected(connected: bool, ibkr: bool = False):
    """Switch between standalone and connected mode."""
    global _connected_mode, _ibkr_available
    _connected_mode = connected
    _ibkr_available = ibkr
    log.info("Data mode: %s (IBKR=%s)", "CONNECTED" if connected else "STANDALONE", ibkr)


def is_connected() -> bool:
    return _connected_mode


# ── Yahoo Finance fetcher ────────────────────────────────────────────
def _fetch_yahoo(symbol: str, period: str = "2y", interval: str = "1d") -> list:
    """Fetch OHLCV from Yahoo Finance. Runs in thread pool."""
    try:
        import yfinance as yf
        ticker = yf.Ticker(symbol)
        hist = ticker.history(period=period, interval=interval)
        if hist.empty:
            return []

        bars = []
        for idx, row in hist.iterrows():
            bars.append({
                "date": idx.strftime("%Y-%m-%d") if interval == "1d" else idx.isoformat(),
                "open": round(float(row["Open"]), 4),
                "high": round(float(row["High"]), 4),
                "low": round(float(row["Low"]), 4),
                "close": round(float(row["Close"]), 4),
                "volume": int(row["Volume"]),
            })
        return bars
    except Exception as e:
        log.warning("Yahoo fetch failed for %s: %s", symbol, e)
        return []


def _fetch_twelve_data(symbol: str, bars: int = 500) -> list:
    """Fetch from Twelve Data API (connected mode only)."""
    if not _twelve_data_key:
        return []
    try:
        import urllib.request
        import json
        url = (f"https://api.twelvedata.com/time_series?"
               f"symbol={symbol}&interval=1day&outputsize={bars}"
               f"&apikey={_twelve_data_key}")
        with urllib.request.urlopen(url, timeout=10) as resp:
            data = json.loads(resp.read())
        values = data.get("values", [])
        return [{
            "date": v["datetime"],
            "open": float(v["open"]),
            "high": float(v["high"]),
            "low": float(v["low"]),
            "close": float(v["close"]),
            "volume": int(v["volume"]),
        } for v in reversed(values)]
    except Exception as e:
        log.warning("TwelveData fetch failed for %s: %s", symbol, e)
        return []


# ── Main data fetch function ─────────────────────────────────────────
async def get_ohlcv(symbol: str, bars: int = 500, region: str = "us",
                    force_refresh: bool = False) -> dict:
    """
    Get OHLCV data with automatic source selection.
    Returns: {"bars": [...], "symbol": str, "source": str}

    Priority:
      1. Fresh local cache (instant)
      2. IBKR real-time (connected mode)
      3. Twelve Data (connected mode)
      4. Yahoo Finance (always available)
      5. Stale cache (last resort)
    """
    # 1. Check cache first (instant, no network)
    if not force_refresh and is_cached(symbol, "1d", region):
        cached = read_cache(symbol, "1d", region, bars)
        if cached:
            return {"bars": cached, "symbol": symbol, "source": "cache", "cached": True}

    # 2. Connected mode: try premium sources first
    if _connected_mode:
        # IBKR would go here (real-time streaming)
        # For now, try Twelve Data
        if _twelve_data_key:
            data = await asyncio.get_event_loop().run_in_executor(
                _executor, _fetch_twelve_data, symbol, bars
            )
            if data:
                write_cache(symbol, data, "1d", region)
                return {"bars": data[-bars:], "symbol": symbol, "source": "twelvedata"}

    # 3. Yahoo Finance (always available, free)
    data = await asyncio.get_event_loop().run_in_executor(
        _executor, _fetch_yahoo, symbol
    )
    if data:
        write_cache(symbol, data, "1d", region)
        return {"bars": data[-bars:], "symbol": symbol, "source": "yahoo"}

    # 4. Stale cache (last resort — better than nothing)
    stale = read_cache(symbol, "1d", region, bars)
    if stale:
        return {"bars": stale, "symbol": symbol, "source": "cache_stale", "stale": True}

    # 5. Nothing available
    return {"bars": [], "symbol": symbol, "source": "none", "error": "No data available"}


# ── Batch fetch (for preloading) ──────────────────────────────────────
async def batch_fetch(symbols: list, region: str = "us") -> dict:
    """Fetch multiple symbols concurrently."""
    results = {}
    tasks = [get_ohlcv(sym, region=region) for sym in symbols]
    completed = await asyncio.gather(*tasks, return_exceptions=True)
    for sym, result in zip(symbols, completed):
        if isinstance(result, Exception):
            results[sym] = {"bars": [], "error": str(result)}
        else:
            results[sym] = result
    return results


# ── Preload (first-run experience) ────────────────────────────────────
async def preload():
    """Preload essential symbols for instant first-run experience."""
    # Only preload symbols not already cached
    to_fetch = [s for s in PRELOAD_SYMBOLS if not is_cached(s)]
    if not to_fetch:
        log.info("Preload: all %d symbols already cached", len(PRELOAD_SYMBOLS))
        return {"status": "cached", "symbols": len(PRELOAD_SYMBOLS)}

    log.info("Preloading %d symbols (out of %d)...", len(to_fetch), len(PRELOAD_SYMBOLS))
    results = await batch_fetch(to_fetch)
    success = sum(1 for r in results.values() if r.get("bars"))
    log.info("Preload complete: %d/%d symbols cached", success, len(to_fetch))
    return {"status": "done", "fetched": success, "total": len(to_fetch)}


# ── Partial update (append latest bars) ───────────────────────────────
async def update_symbol(symbol: str, region: str = "us") -> dict:
    """Fetch latest bars and append to cache (incremental update)."""
    data = await asyncio.get_event_loop().run_in_executor(
        _executor, _fetch_yahoo, symbol, "5d", "1d"
    )
    if data:
        append_cache(symbol, data, "1d", region)
        return {"updated": True, "new_bars": len(data)}
    return {"updated": False}
