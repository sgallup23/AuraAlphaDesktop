"""
Data fetcher: ensures OHLCV parquets are available locally before backtests run.
Downloads missing data from the coordinator API, with yfinance fallback.
"""
from __future__ import annotations

import logging
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import List, Set, Tuple

from .api_client import CoordinatorClient

log = logging.getLogger("standalone.data_fetcher")


class DataFetcher:
    """Pre-fetches OHLCV parquet data for symbols needed by research jobs."""

    def __init__(self, client: CoordinatorClient, cache_dir: Path):
        self.client = client
        self.cache_dir = cache_dir

    def _parquet_path(self, symbol: str, region: str) -> Path:
        """Canonical local path for a symbol's parquet file."""
        return self.cache_dir / region / f"{symbol}.parquet"

    def _download_symbol(self, symbol: str, region: str) -> bool:
        """Download a single symbol's parquet, with yfinance fallback.

        Returns True if the file is now available locally.
        """
        dest = self._parquet_path(symbol, region)

        # Already cached
        if dest.exists() and dest.stat().st_size > 0:
            return True

        # Try coordinator API first
        ok = self.client.download_data(region, symbol, dest)
        if ok and dest.exists() and dest.stat().st_size > 0:
            log.debug("Downloaded %s/%s from coordinator", region, symbol)
            return True

        # Fallback: yfinance
        try:
            import yfinance as yf

            ticker = symbol
            # Crypto symbols need special yfinance format
            if region == "crypto":
                if not ticker.endswith("-USD"):
                    ticker = f"{ticker}-USD"

            log.debug("Trying yfinance fallback for %s (ticker=%s)", symbol, ticker)
            df = yf.download(ticker, period="max", progress=False, auto_adjust=True)

            if df is not None and len(df) >= 50:
                try:
                    import polars as pl

                    # Convert pandas → polars and standardize column names
                    df = df.reset_index()
                    rename_map = {}
                    for col in df.columns:
                        lower = col.lower() if isinstance(col, str) else str(col).lower()
                        rename_map[col] = lower
                    df = df.rename(columns=rename_map)

                    pldf = pl.from_pandas(df)
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    pldf.write_parquet(dest)
                    log.info("Saved %s/%s via yfinance (%d rows)", region, symbol, len(pldf))
                    return True
                except Exception as e:
                    log.warning("Failed to save yfinance data for %s: %s", symbol, e)
        except ImportError:
            pass  # yfinance not available
        except Exception as e:
            log.debug("yfinance fallback failed for %s: %s", symbol, e)

        return False

    def ensure_data(self, symbols: List[str], region: str) -> Tuple[int, int]:
        """Ensure all symbols have cached parquets. Downloads missing ones.

        Uses up to 4 concurrent download threads.
        Returns (available_count, missing_count).
        """
        # Determine which symbols need downloading
        needed: List[str] = []
        for sym in symbols:
            p = self._parquet_path(sym, region)
            if not (p.exists() and p.stat().st_size > 0):
                needed.append(sym)

        if not needed:
            return len(symbols), 0

        log.info(
            "Fetching data: %d/%d symbols need download for region=%s",
            len(needed), len(symbols), region,
        )

        # Download concurrently (max 4 threads)
        succeeded: Set[str] = set()
        max_workers = min(4, len(needed))

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            future_to_sym = {
                executor.submit(self._download_symbol, sym, region): sym
                for sym in needed
            }
            for future in as_completed(future_to_sym):
                sym = future_to_sym[future]
                try:
                    if future.result():
                        succeeded.add(sym)
                except Exception as e:
                    log.warning("Download error for %s: %s", sym, e)

        available = len(symbols) - len(needed) + len(succeeded)
        missing = len(needed) - len(succeeded)

        if missing > 0:
            log.warning(
                "Data fetch complete: %d available, %d still missing",
                available, missing,
            )
        else:
            log.info("Data fetch complete: all %d symbols available", available)

        return available, missing
