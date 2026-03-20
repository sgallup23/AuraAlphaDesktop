#!/usr/bin/env python3
"""
Aura Alpha Grid Worker  --  Distributable compute node
========================================================

Connects to the Aura Alpha coordinator via HTTPS, pulls research/backtest
jobs, executes them locally, and reports results back. Designed to run on
any Windows, macOS, or Linux machine with Python 3.10+.

Hub-and-spoke model:
  - Coordinator (hub): https://auraalpha.cc/api/cluster/contributor/*
  - This worker (spoke): fetches jobs, runs compute, returns results

Token is auto-provisioned on first run. No manual setup required beyond
having Python installed.

Usage:
    python worker.py
    python worker.py --coordinator-url https://auraalpha.cc --max-parallel 4
    python worker.py --verbose
"""
from __future__ import annotations

import argparse
import json
import logging
import os
import platform
import signal
import ssl
import sys
import time
import traceback
import urllib.error
import urllib.request
from concurrent.futures import ProcessPoolExecutor, as_completed
from concurrent.futures import TimeoutError as FuturesTimeout
from dataclasses import dataclass, field
from pathlib import Path
from threading import Event, Thread
from typing import Any, Dict, List, Optional, Tuple

__version__ = "1.0.0"

# ============================================================================
# Logging
# ============================================================================

log = logging.getLogger("grid-worker")

# ============================================================================
# Configuration
# ============================================================================

POLL_INTERVAL = 5          # seconds between idle dequeue attempts
HEARTBEAT_INTERVAL = 30    # seconds between heartbeats
JOB_TIMEOUT = 600          # seconds per job before timeout
MAX_RETRIES = 3            # HTTP retry attempts
BACKOFF_BASE = 1           # base seconds for exponential backoff
THROTTLE_CHECK_INTERVAL = 10  # seconds between system load checks


def _auto_cpu_count() -> int:
    try:
        return os.cpu_count() or 1
    except Exception:
        return 1


def _auto_ram_gb() -> float:
    """Detect system RAM in GB using platform-specific methods (no psutil required)."""
    # Try psutil first (if user installed it)
    try:
        import psutil
        return round(psutil.virtual_memory().total / (1024 ** 3), 1)
    except ImportError:
        pass

    try:
        if platform.system() == "Linux":
            with open("/proc/meminfo") as f:
                for line in f:
                    if line.startswith("MemTotal:"):
                        return round(int(line.split()[1]) / (1024 ** 2), 1)
        elif platform.system() == "Darwin":
            import subprocess
            out = subprocess.check_output(
                ["sysctl", "-n", "hw.memsize"], timeout=5
            ).decode().strip()
            return round(int(out) / (1024 ** 3), 1)
        elif platform.system() == "Windows":
            import ctypes

            class MEMORYSTATUSEX(ctypes.Structure):
                _fields_ = [
                    ("dwLength", ctypes.c_ulong),
                    ("dwMemoryLoad", ctypes.c_ulong),
                    ("ullTotalPhys", ctypes.c_ulonglong),
                    ("ullAvailPhys", ctypes.c_ulonglong),
                    ("ullTotalPageFile", ctypes.c_ulonglong),
                    ("ullAvailPageFile", ctypes.c_ulonglong),
                    ("ullTotalVirtual", ctypes.c_ulonglong),
                    ("ullAvailVirtual", ctypes.c_ulonglong),
                    ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
                ]

            stat = MEMORYSTATUSEX()
            stat.dwLength = ctypes.sizeof(stat)
            ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(stat))
            return round(stat.ullTotalPhys / (1024 ** 3), 1)
    except Exception:
        pass

    return 8.0  # safe default


def _auto_max_parallel() -> int:
    cpus = _auto_cpu_count()
    return max(1, cpus - 1)


@dataclass
class WorkerConfig:
    """All configuration for the grid worker."""
    coordinator_url: str = "https://auraalpha.cc"
    max_parallel: int = 0       # 0 = auto-detect
    batch_size: int = 5
    cache_dir: Path = field(default_factory=lambda: Path.home() / ".aura-worker" / "data")
    log_dir: Path = field(default_factory=lambda: Path.home() / ".aura-worker" / "logs")
    heartbeat_interval: int = HEARTBEAT_INTERVAL
    job_timeout: int = JOB_TIMEOUT
    cpu_count: int = field(default_factory=_auto_cpu_count)
    ram_gb: float = field(default_factory=_auto_ram_gb)
    verbose: bool = False

    def __post_init__(self):
        if self.max_parallel <= 0:
            self.max_parallel = _auto_max_parallel()
        if isinstance(self.cache_dir, str):
            self.cache_dir = Path(self.cache_dir)
        if isinstance(self.log_dir, str):
            self.log_dir = Path(self.log_dir)

    def ensure_dirs(self):
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.log_dir.mkdir(parents=True, exist_ok=True)


# ============================================================================
# Token persistence  (auto-provision on first run)
# ============================================================================

def _token_file_path() -> Path:
    d = Path.home() / ".aura-worker"
    d.mkdir(parents=True, exist_ok=True)
    return d / "grid_token.json"


def _load_stored_token() -> Optional[dict]:
    path = _token_file_path()
    if not path.exists():
        return None
    try:
        data = json.loads(path.read_text())
        if data.get("token") and data.get("worker_id"):
            return data
    except Exception:
        pass
    return None


def _save_token(data: dict):
    path = _token_file_path()
    path.write_text(json.dumps(data, indent=2))
    log.info("Token saved to %s", path)


def _get_worker_id() -> str:
    hostname = platform.node() or "unknown"
    return f"grid-{hostname.lower().replace(' ', '-')}"


def _auto_provision(base_url: str) -> dict:
    """Call the zero-auth auto-provision endpoint to get a unique token.

    POST /api/cluster/contributor/auto-provision
    Body: {hostname, cpus, ram, os}
    Returns: {token, worker_id}
    """
    url = f"{base_url.rstrip('/')}/api/cluster/contributor/auto-provision"
    body = json.dumps({
        "hostname": platform.node() or "unknown",
        "cpus": _auto_cpu_count(),
        "ram": _auto_ram_gb(),
        "os": f"{platform.system()} {platform.release()}",
    }).encode("utf-8")

    req = urllib.request.Request(
        url, data=body, method="POST",
        headers={"Content-Type": "application/json"},
    )
    try:
        ctx = ssl.create_default_context()
        with urllib.request.urlopen(req, timeout=15, context=ctx) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            if data.get("token") and data.get("worker_id"):
                log.info("Auto-provisioned worker_id=%s", data["worker_id"])
                return data
            raise ValueError(f"Unexpected auto-provision response: {data}")
    except Exception as e:
        log.warning("Auto-provision failed: %s", e)
        raise


def resolve_token(coordinator_url: str, cli_token: Optional[str] = None) -> Tuple[str, str]:
    """Resolve token and worker_id from CLI arg, env var, stored file, or auto-provision.

    Returns (token, worker_id).
    """
    # 1. Explicit CLI flag
    if cli_token:
        log.info("Using CLI-provided token")
        return cli_token, _get_worker_id()

    # 2. Environment variable
    env_token = os.getenv("GRID_WORKER_TOKEN") or os.getenv("AURA_TOKEN")
    if env_token:
        log.info("Using token from environment variable")
        return env_token, _get_worker_id()

    # 3. Stored auto-provisioned token
    stored = _load_stored_token()
    if stored:
        log.info("Loaded stored token for worker_id=%s", stored["worker_id"])
        return stored["token"], stored["worker_id"]

    # 4. Auto-provision from coordinator
    log.info("No token found -- auto-provisioning from coordinator...")
    provisioned = _auto_provision(coordinator_url)
    _save_token(provisioned)
    return provisioned["token"], provisioned["worker_id"]


# ============================================================================
# HTTP client  (stdlib only -- no requests dependency)
# ============================================================================

def _http_request(
    method: str,
    url: str,
    headers: dict,
    body: Optional[dict] = None,
    timeout: int = 10,
    stream: bool = False,
) -> Tuple[int, Any]:
    """Make an HTTP request with retry logic. Returns (status_code, parsed_body).

    Returns (0, {}) on network/timeout errors after all retries exhausted.
    """
    last_exc: Optional[Exception] = None

    for attempt in range(MAX_RETRIES):
        try:
            data = json.dumps(body).encode("utf-8") if body is not None else None
            req = urllib.request.Request(url, data=data, method=method, headers=headers)
            ctx = ssl.create_default_context()
            resp = urllib.request.urlopen(req, timeout=timeout, context=ctx)

            if stream:
                return (resp.status, resp)

            resp_body = resp.read().decode("utf-8")
            try:
                return (resp.status, json.loads(resp_body))
            except json.JSONDecodeError:
                return (resp.status, {"raw": resp_body})
        except urllib.error.HTTPError as e:
            # Don't retry 4xx (except 429)
            if 400 <= e.code < 500 and e.code != 429:
                body_text = ""
                try:
                    body_text = e.read().decode("utf-8")[:500]
                except Exception:
                    pass
                return (e.code, {"error": body_text})
            last_exc = e
        except Exception as e:
            last_exc = e

        if attempt < MAX_RETRIES - 1:
            wait = BACKOFF_BASE * (2 ** attempt)
            log.debug("Request %s %s failed (attempt %d/%d), retrying in %ds: %s",
                      method, url, attempt + 1, MAX_RETRIES, wait, last_exc)
            time.sleep(wait)

    return (0, {"error": str(last_exc) if last_exc else "unknown"})


class CoordinatorClient:
    """HTTP client for the coordinator API at /api/cluster/contributor/*."""

    def __init__(self, base_url: str, token: str, worker_id: str):
        self.base_url = base_url.rstrip("/")
        self.api = f"{self.base_url}/api/cluster/contributor"
        self.token = token
        self.worker_id = worker_id
        self.headers = {
            "Content-Type": "application/json",
            "X-Worker-Token": self.token,
            "X-Contributor-Token": self.token,
            "X-Worker-Id": self.worker_id,
        }

    def _url(self, path: str) -> str:
        return f"{self.api}/{path.lstrip('/')}"

    def register(self, capabilities: dict) -> bool:
        caps = dict(capabilities)
        caps["worker_id"] = self.worker_id
        status, data = _http_request("POST", self._url("register"), self.headers, caps)
        if status == 200:
            log.info("Registered as %s (%d CPUs, %.1f GB RAM)",
                     self.worker_id,
                     caps.get("cpu_count", caps.get("cpus", 0)),
                     caps.get("ram_gb", 0))
            return True
        log.warning("Registration failed: %d %s", status, str(data)[:200])
        return False

    def heartbeat(self, active_job_ids: Optional[List[str]] = None) -> bool:
        status, _ = _http_request("POST", self._url("heartbeat"), self.headers, {
            "worker_id": self.worker_id,
            "status": "online",
            "job_ids": active_job_ids or [],
        }, timeout=5)
        return status == 200

    def dequeue(self, count: int = 5) -> list:
        status, data = _http_request("POST", self._url("dequeue"), self.headers, {
            "worker_id": self.worker_id,
            "count": count,
            "max_jobs": count,
        }, timeout=10)
        if status == 200:
            return data.get("jobs", [])
        return []

    def complete(self, job_id: str, metrics: Optional[dict] = None,
                 result: Optional[dict] = None, duration: float = 0) -> bool:
        payload = {
            "worker_id": self.worker_id,
            "job_id": job_id,
            "status": "completed",
            "metrics": metrics or {},
            "result": json.dumps(result or metrics or {}),
            "duration_sec": round(duration, 2),
        }
        status, _ = _http_request("POST", self._url("complete"), self.headers, payload)
        return status == 200

    def fail(self, job_id: str, error: str, duration: float = 0) -> bool:
        payload = {
            "worker_id": self.worker_id,
            "job_id": job_id,
            "status": "failed",
            "error": error[:2000],
            "duration_sec": round(duration, 2),
        }
        status, _ = _http_request("POST", self._url("complete"), self.headers, payload)
        return status == 200

    def download_data(self, region: str, symbol: str, dest_path: Path) -> bool:
        """Download a parquet file from the coordinator. Returns True on success."""
        try:
            status, resp = _http_request(
                "GET", self._url(f"data/{region}/{symbol}"),
                self.headers, stream=True, timeout=300,
            )
            if status != 200 or not hasattr(resp, "read"):
                return False

            dest_path.parent.mkdir(parents=True, exist_ok=True)
            with open(dest_path, "wb") as f:
                while True:
                    chunk = resp.read(65536)
                    if not chunk:
                        break
                    f.write(chunk)
            return dest_path.exists() and dest_path.stat().st_size > 0
        except Exception as e:
            log.debug("Failed to download %s/%s: %s", region, symbol, e)
            return False


# ============================================================================
# Adaptive Throttle  (yields CPU to games, apps, etc.)
# ============================================================================

class AdaptiveThrottle:
    """Monitors system load and recommends how many parallel workers to run."""

    HEAVY_LOAD = 60   # drop to 25% capacity
    MEDIUM_LOAD = 40  # drop to 50%
    LIGHT_LOAD = 20   # drop to 75%
    RAM_CRITICAL = 25  # % available -- go minimal

    def __init__(self, max_parallel: int):
        self.max_parallel = max_parallel
        self.current = max_parallel
        self._last_check = 0.0
        self.is_throttled = False

    def _get_metrics(self) -> dict:
        try:
            import psutil
            cpu = psutil.cpu_percent(interval=0.5)
            mem = psutil.virtual_memory()
            return {"cpu_pct": cpu, "ram_avail_pct": mem.available / mem.total * 100}
        except ImportError:
            pass

        # Linux fallback
        try:
            with open("/proc/stat") as f:
                p1 = f.readline().split()
            time.sleep(0.3)
            with open("/proc/stat") as f:
                p2 = f.readline().split()

            idle1, total1 = int(p1[4]), sum(int(x) for x in p1[1:])
            idle2, total2 = int(p2[4]), sum(int(x) for x in p2[1:])
            dt = total2 - total1
            cpu = ((dt - (idle2 - idle1)) / max(dt, 1)) * 100

            meminfo = {}
            with open("/proc/meminfo") as f:
                for line in f:
                    parts = line.split()
                    meminfo[parts[0].rstrip(":")] = int(parts[1])
            total_kb = meminfo.get("MemTotal", 1)
            avail_kb = meminfo.get("MemAvailable", meminfo.get("MemFree", 0))
            return {"cpu_pct": cpu, "ram_avail_pct": avail_kb / total_kb * 100}
        except Exception:
            return {"cpu_pct": 0, "ram_avail_pct": 100}

    def recommended_workers(self) -> int:
        now = time.time()
        if now - self._last_check < THROTTLE_CHECK_INTERVAL:
            return self.current

        self._last_check = now
        m = self._get_metrics()

        # Estimate our CPU contribution and subtract it
        our_est = (self.current / max(_auto_cpu_count(), 1)) * 100
        other_cpu = max(0, m["cpu_pct"] - our_est)

        if m["ram_avail_pct"] < self.RAM_CRITICAL:
            target = max(1, 2)
        elif other_cpu >= self.HEAVY_LOAD:
            target = max(1, self.max_parallel // 4)
        elif other_cpu >= self.MEDIUM_LOAD:
            target = max(1, self.max_parallel // 2)
        elif other_cpu >= self.LIGHT_LOAD:
            target = max(2, int(self.max_parallel * 0.75))
        else:
            target = self.max_parallel

        prev = self.current
        # Drop fast, ramp slowly
        if target < self.current:
            self.current = target
        elif target > self.current:
            self.current = min(target, self.current + 2)

        self.is_throttled = self.current < self.max_parallel

        if self.current != prev:
            log.info("Throttle: %d -> %d workers (other CPU ~%.0f%%, RAM avail %.0f%%)",
                     prev, self.current, other_cpu, m["ram_avail_pct"])

        return self.current


# ============================================================================
# Data Fetcher  (downloads OHLCV from coordinator before backtests)
# ============================================================================

class DataFetcher:
    """Pre-fetches OHLCV parquet data for symbols needed by research jobs."""

    def __init__(self, client: CoordinatorClient, cache_dir: Path):
        self.client = client
        self.cache_dir = cache_dir

    def _path(self, symbol: str, region: str) -> Path:
        return self.cache_dir / region / f"{symbol}.parquet"

    def ensure_data(self, symbols: List[str], region: str) -> Tuple[int, int]:
        """Download any missing parquets. Returns (available, missing)."""
        needed = [s for s in symbols
                  if not (self._path(s, region).exists()
                          and self._path(s, region).stat().st_size > 0)]

        if not needed:
            return len(symbols), 0

        log.info("Fetching data: %d/%d symbols need download for region=%s",
                 len(needed), len(symbols), region)

        succeeded = 0
        for sym in needed:
            if self.client.download_data(region, sym, self._path(sym, region)):
                succeeded += 1

        available = len(symbols) - len(needed) + succeeded
        missing = len(needed) - succeeded
        if missing > 0:
            log.warning("Data fetch: %d available, %d still missing", available, missing)
        else:
            log.info("Data fetch complete: all %d symbols available", available)
        return available, missing


# ============================================================================
# Backtest Engine  (generic compute -- NO proprietary strategy code)
# ============================================================================
# This engine only processes parameters sent by the coordinator. It does NOT
# contain any strategy definitions, signal generation logic, or trading
# algorithms. It is a parameter-driven simulation framework.

def _load_bars(symbol: str, region: str, cache_dir: Path) -> Optional[dict]:
    """Load OHLCV bars from cached parquet. Returns dict or None."""
    try:
        import numpy as np
    except ImportError:
        log.error("numpy not installed -- cannot run backtests")
        return None
    try:
        import polars as pl
    except ImportError:
        log.error("polars not installed -- cannot load parquet data")
        return None

    search_dirs = [cache_dir / region]
    if region != "us":
        search_dirs.append(cache_dir / "us")

    parquet_path = None
    for d in search_dirs:
        candidate = d / f"{symbol}.parquet"
        if candidate.exists():
            parquet_path = candidate
            break

    if parquet_path is None:
        return None

    try:
        df = pl.read_parquet(parquet_path).sort("date")
        if df.is_empty() or "close" not in df.columns:
            return None
        return {
            "dates": [str(d)[:10] for d in df["date"].to_list()],
            "closes": df["close"].to_numpy().astype(float),
            "volumes": df["volume"].to_numpy().astype(float),
            "highs": df["high"].to_numpy().astype(float),
            "lows": df["low"].to_numpy().astype(float),
        }
    except Exception:
        return None


def _compute_atr(highs, lows, closes, period: int = 14):
    """Compute Average True Range."""
    import numpy as np
    n = len(highs)
    if n < period + 1:
        return np.full(n, np.nan)
    tr = np.maximum(
        highs[1:] - lows[1:],
        np.maximum(np.abs(highs[1:] - closes[:-1]), np.abs(lows[1:] - closes[:-1])),
    )
    atr = np.full(n, np.nan)
    if len(tr) >= period:
        atr[period] = np.mean(tr[:period])
        for i in range(period + 1, len(tr) + 1):
            atr[i] = (atr[i - 1] * (period - 1) + tr[i - 1]) / period
    return atr


def _simulate_trades(
    closes, highs, lows, dates: List[str],
    params: dict, direction: str = "long",
    date_start: str = "", date_end: str = "",
) -> List[dict]:
    """Simulate trades for a single symbol with coordinator-provided parameters.

    Uses ATR-based stops/take-profits, EMA crossover entries, RSI confirmation.
    This is a generic compute kernel -- all strategy-specific parameters come from
    the coordinator via the job payload.
    """
    import numpy as np

    n = len(closes)
    if n < 50:
        return []

    # Extract parameters with safe defaults
    stop_atr = params.get("stop_loss_atr_mult", 2.0)
    tp_atr = params.get("take_profit_atr_mult", 4.0)
    trail_pct = params.get("trailing_stop_pct", 0.05)
    max_hold = params.get("max_hold_days", 30)
    atr_period = params.get("atr_period", 14)
    ema_fast_period = params.get("ema_fast", 9)
    ema_slow_period = params.get("ema_slow", 21)
    rsi_period = params.get("rsi_period", 14)
    rsi_threshold = params.get("rsi_entry_threshold", 50.0)
    vol_mult = params.get("volume_multiplier", 1.5)
    vol_sma_period = params.get("volume_sma_period", 20)

    # Compute indicators
    atr = _compute_atr(highs, lows, closes, atr_period)

    # EMA fast/slow
    ema_fast = np.full(n, np.nan)
    ema_slow = np.full(n, np.nan)
    if n > ema_fast_period:
        alpha_f = 2.0 / (ema_fast_period + 1)
        ema_fast[ema_fast_period - 1] = np.mean(closes[:ema_fast_period])
        for i in range(ema_fast_period, n):
            ema_fast[i] = closes[i] * alpha_f + ema_fast[i - 1] * (1 - alpha_f)
    if n > ema_slow_period:
        alpha_s = 2.0 / (ema_slow_period + 1)
        ema_slow[ema_slow_period - 1] = np.mean(closes[:ema_slow_period])
        for i in range(ema_slow_period, n):
            ema_slow[i] = closes[i] * alpha_s + ema_slow[i - 1] * (1 - alpha_s)

    # RSI
    rsi = np.full(n, 50.0)
    if n > rsi_period + 1:
        deltas = np.diff(closes)
        gains = np.where(deltas > 0, deltas, 0.0)
        losses_arr = np.where(deltas < 0, -deltas, 0.0)
        avg_gain = np.mean(gains[:rsi_period])
        avg_loss = np.mean(losses_arr[:rsi_period])
        for i in range(rsi_period, len(deltas)):
            avg_gain = (avg_gain * (rsi_period - 1) + gains[i]) / rsi_period
            avg_loss = (avg_loss * (rsi_period - 1) + losses_arr[i]) / rsi_period
            rs = avg_gain / (avg_loss + 1e-10)
            rsi[i + 1] = 100.0 - (100.0 / (1.0 + rs))

    # Volume SMA
    vol_sma = np.full(n, np.nan)
    volumes = np.zeros(n)
    if "_volumes" in params:
        volumes = np.array(params["_volumes"], dtype=float)
        if len(volumes) >= vol_sma_period:
            for i in range(vol_sma_period - 1, n):
                vol_sma[i] = np.mean(volumes[i - vol_sma_period + 1 : i + 1])

    # Date window
    start_idx = 0
    end_idx = n
    if date_start:
        for i, d in enumerate(dates):
            if d >= date_start:
                start_idx = i
                break
    if date_end:
        for i in range(n - 1, -1, -1):
            if dates[i] <= date_end:
                end_idx = i + 1
                break

    min_lookback = max(ema_slow_period, atr_period, rsi_period, vol_sma_period) + 5
    start_idx = max(start_idx, min_lookback)

    trades: List[dict] = []
    in_trade = False
    entry_price = 0.0
    entry_idx = 0
    stop_price = 0.0
    tp_price = 0.0
    trail_high = 0.0
    trail_low = float("inf")

    for i in range(start_idx, min(end_idx, n)):
        if np.isnan(atr[i]) or np.isnan(ema_fast[i]) or np.isnan(ema_slow[i]):
            continue

        if not in_trade:
            if direction == "long":
                cross_up = ema_fast[i] > ema_slow[i] and (
                    i > 0 and ema_fast[i - 1] <= ema_slow[i - 1]
                )
                rsi_ok = rsi[i] > rsi_threshold
                vol_ok = True
                if not np.isnan(vol_sma[i]) and vol_sma[i] > 0 and len(volumes) > i:
                    vol_ok = volumes[i] > vol_sma[i] * vol_mult

                if cross_up and rsi_ok and vol_ok:
                    entry_price = closes[i]
                    entry_idx = i
                    stop_price = entry_price - atr[i] * stop_atr
                    tp_price = entry_price + atr[i] * tp_atr
                    trail_high = entry_price
                    in_trade = True
            else:
                cross_down = ema_fast[i] < ema_slow[i] and (
                    i > 0 and ema_fast[i - 1] >= ema_slow[i - 1]
                )
                rsi_ok = rsi[i] < (100.0 - rsi_threshold)
                vol_ok = True
                if not np.isnan(vol_sma[i]) and vol_sma[i] > 0 and len(volumes) > i:
                    vol_ok = volumes[i] > vol_sma[i] * vol_mult

                if cross_down and rsi_ok and vol_ok:
                    entry_price = closes[i]
                    entry_idx = i
                    stop_price = entry_price + atr[i] * stop_atr
                    tp_price = entry_price - atr[i] * tp_atr
                    trail_low = entry_price
                    in_trade = True
        else:
            hold_days = i - entry_idx
            exit_price = None
            exit_reason = ""

            if direction == "long":
                trail_high = max(trail_high, highs[i])
                trail_stop = trail_high * (1.0 - trail_pct)
                if lows[i] <= stop_price:
                    exit_price, exit_reason = stop_price, "stop_loss"
                elif highs[i] >= tp_price:
                    exit_price, exit_reason = tp_price, "take_profit"
                elif closes[i] <= trail_stop and hold_days > 1:
                    exit_price, exit_reason = trail_stop, "trailing_stop"
                elif hold_days >= max_hold:
                    exit_price, exit_reason = closes[i], "max_hold"
            else:
                trail_low = min(trail_low, lows[i])
                trail_stop = trail_low * (1.0 + trail_pct)
                if highs[i] >= stop_price:
                    exit_price, exit_reason = stop_price, "stop_loss"
                elif lows[i] <= tp_price:
                    exit_price, exit_reason = tp_price, "take_profit"
                elif closes[i] >= trail_stop and hold_days > 1:
                    exit_price, exit_reason = trail_stop, "trailing_stop"
                elif hold_days >= max_hold:
                    exit_price, exit_reason = closes[i], "max_hold"

            if exit_price is not None:
                if direction == "long":
                    pnl_pct = (exit_price - entry_price) / entry_price
                else:
                    pnl_pct = (entry_price - exit_price) / entry_price
                trades.append({
                    "entry_date": dates[entry_idx],
                    "exit_date": dates[i],
                    "entry_price": round(entry_price, 4),
                    "exit_price": round(exit_price, 4),
                    "pnl_pct": round(pnl_pct, 6),
                    "hold_days": hold_days,
                    "exit_reason": exit_reason,
                    "direction": direction,
                })
                in_trade = False

    # Close any open trade at end of window
    if in_trade and end_idx > entry_idx:
        final_idx = min(end_idx - 1, n - 1)
        exit_price = closes[final_idx]
        pnl_pct = ((exit_price - entry_price) / entry_price if direction == "long"
                    else (entry_price - exit_price) / entry_price)
        trades.append({
            "entry_date": dates[entry_idx],
            "exit_date": dates[final_idx],
            "entry_price": round(entry_price, 4),
            "exit_price": round(exit_price, 4),
            "pnl_pct": round(pnl_pct, 6),
            "hold_days": final_idx - entry_idx,
            "exit_reason": "window_end",
            "direction": direction,
        })

    return trades


def _compute_metrics(trades: List[dict]) -> dict:
    """Compute performance metrics from a list of trades."""
    import numpy as np

    if not trades:
        return {
            "num_trades": 0, "sharpe": 0.0, "sortino": 0.0,
            "profit_factor": 0.0, "win_rate": 0.0, "avg_return": 0.0,
            "max_drawdown": 0.0, "total_return": 0.0, "avg_hold_days": 0.0,
        }

    returns = [t["pnl_pct"] for t in trades]
    n = len(returns)
    wins = sum(1 for r in returns if r > 0)

    avg_ret = float(np.mean(returns))
    std_ret = float(np.std(returns, ddof=1)) if n > 1 else 1e-9
    downside = (float(np.std([r for r in returns if r < 0], ddof=1))
                if any(r < 0 for r in returns) else 1e-9)

    sharpe = avg_ret / (std_ret + 1e-9)
    sortino = avg_ret / (downside + 1e-9)

    gross_profit = sum(r for r in returns if r > 0)
    gross_loss = abs(sum(r for r in returns if r < 0))
    profit_factor = gross_profit / (gross_loss + 1e-9)

    cum = np.cumprod(1 + np.array(returns))
    peak = np.maximum.accumulate(cum)
    dd = (cum - peak) / peak
    max_dd = float(np.min(dd)) if len(dd) > 0 else 0.0
    total_ret = float(cum[-1] - 1) if len(cum) > 0 else 0.0
    avg_hold = float(np.mean([t["hold_days"] for t in trades]))

    exit_reasons: Dict[str, int] = {}
    for t in trades:
        r = t.get("exit_reason", "unknown")
        exit_reasons[r] = exit_reasons.get(r, 0) + 1

    return {
        "num_trades": n,
        "sharpe": round(sharpe, 4),
        "sortino": round(sortino, 4),
        "profit_factor": round(profit_factor, 4),
        "win_rate": round(wins / n * 100, 1),
        "avg_return": round(avg_ret, 6),
        "max_drawdown": round(max_dd, 6),
        "total_return": round(total_ret, 6),
        "avg_hold_days": round(avg_hold, 1),
        "gross_profit": round(gross_profit, 6),
        "gross_loss": round(gross_loss, 6),
        "wins": wins,
        "losses": n - wins,
        "exit_reasons": exit_reasons,
    }


def run_backtest_job(job_dict: dict, cache_dir: Path) -> dict:
    """Execute a research_backtest job. Must be picklable for ProcessPoolExecutor.

    Accepts a job dict from the coordinator and runs the backtest across all
    symbols in the job's universe.
    """
    try:
        job_id = job_dict.get("job_id", "unknown")
        strategy_family = job_dict.get("strategy_family", "unknown")
        parameter_set = job_dict.get("parameter_set", {})
        symbol_universe = job_dict.get("symbol_universe", [])
        date_window = job_dict.get("date_window", ":")
        backtest_config = job_dict.get("backtest_config", {})

        direction = backtest_config.get("direction", "long")
        region = backtest_config.get("region", "us")
        date_parts = date_window.split(":")
        date_start = date_parts[0] if len(date_parts) > 0 else ""
        date_end = date_parts[1] if len(date_parts) > 1 else ""

        all_trades: List[dict] = []
        symbols_tested = 0
        symbols_skipped = 0

        for symbol in symbol_universe:
            bars = _load_bars(symbol, region, cache_dir)
            if bars is None or len(bars["closes"]) < 50:
                symbols_skipped += 1
                continue

            params = dict(parameter_set)
            params["_volumes"] = bars["volumes"].tolist()

            trades = _simulate_trades(
                closes=bars["closes"], highs=bars["highs"],
                lows=bars["lows"], dates=bars["dates"],
                params=params, direction=direction,
                date_start=date_start, date_end=date_end,
            )
            all_trades.extend(trades)
            symbols_tested += 1

        metrics = _compute_metrics(all_trades)
        metrics["symbols_tested"] = symbols_tested
        metrics["symbols_skipped"] = symbols_skipped
        metrics["strategy_family"] = strategy_family
        metrics["date_window"] = date_window
        metrics["mutation_id"] = parameter_set.get("_mutation_id", "unknown")
        metrics["parameter_set"] = {
            k: v for k, v in parameter_set.items() if not k.startswith("_")
        }

        return {"job_id": job_id, "status": "completed", "metrics": metrics}

    except Exception as e:
        return {
            "job_id": job_dict.get("job_id", "unknown"),
            "status": "failed",
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
        }


# ============================================================================
# Job Router  (dispatches by job_type -- NO proprietary code paths)
# ============================================================================

def route_job(job_dict: dict, cache_dir: Path) -> dict:
    """Route a job to the correct executor based on job_type.

    Only supports job types that can run without proprietary code:
      - research_backtest: parameter-driven backtest (primary workload)

    All other job types are returned as 'skipped' -- the coordinator will
    re-route them to a full node.
    """
    job_type = job_dict.get("job_type", "research_backtest")
    job_id = job_dict.get("job_id", "unknown")

    t0 = time.time()

    try:
        if job_type == "research_backtest":
            result = run_backtest_job(job_dict, cache_dir)
        else:
            # Grid workers only run parameter-driven backtests.
            # Signal gen, ML train, walk-forward, etc. require the full codebase.
            result = {
                "job_id": job_id,
                "status": "failed",
                "error": f"Job type '{job_type}' not supported on grid workers. "
                         f"Supported: research_backtest",
            }

        result["job_id"] = job_id
        result["execution_time"] = round(time.time() - t0, 2)
        return result

    except Exception as e:
        return {
            "job_id": job_id,
            "status": "failed",
            "error": f"{type(e).__name__}: {e}",
            "execution_time": round(time.time() - t0, 2),
        }


# ============================================================================
# Main Worker Loop
# ============================================================================

class GridWorker:
    """SETI@home-style research worker: pulls jobs over HTTPS, computes, reports."""

    def __init__(self, config: WorkerConfig, token: str, worker_id: str):
        self.config = config
        config.ensure_dirs()

        self.client = CoordinatorClient(
            base_url=config.coordinator_url,
            token=token,
            worker_id=worker_id,
        )
        self.fetcher = DataFetcher(client=self.client, cache_dir=config.cache_dir)
        self.throttle = AdaptiveThrottle(max_parallel=config.max_parallel)
        self.worker_id = worker_id

        self._shutdown = Event()
        self._active_job_ids: List[str] = []
        self.stats = {"completed": 0, "failed": 0, "started_at": 0.0}

    def _capabilities(self) -> dict:
        return {
            "hostname": platform.node() or "unknown",
            "cpu_count": self.config.cpu_count,
            "cpus": self.config.cpu_count,
            "ram_gb": self.config.ram_gb,
            "max_parallel": self.config.max_parallel,
            "os": f"{platform.system()} {platform.release()}",
            "python_version": platform.python_version(),
            "worker_version": __version__,
            "worker_type": "grid_contributor",
            "supported_job_types": ["research_backtest"],
        }

    def _throughput(self) -> float:
        elapsed = time.time() - self.stats["started_at"]
        if elapsed < 10:
            return 0.0
        return self.stats["completed"] / (elapsed / 60.0)

    def _heartbeat_loop(self):
        while not self._shutdown.is_set():
            try:
                self.client.heartbeat(list(self._active_job_ids))
            except Exception:
                pass
            self._shutdown.wait(timeout=self.config.heartbeat_interval)

    def _prefetch_data(self, jobs: List[dict]):
        region_symbols: Dict[str, List[str]] = {}
        for job in jobs:
            bc = job.get("backtest_config", {})
            region = bc.get("region", "us")
            symbols = job.get("symbol_universe", [])
            region_symbols.setdefault(region, []).extend(symbols)

        for region, syms in region_symbols.items():
            unique = list(dict.fromkeys(syms))
            self.fetcher.ensure_data(unique, region)

    def _execute_batch(self, jobs: List[dict]) -> List[dict]:
        self._prefetch_data(jobs)
        self._active_job_ids = [j.get("job_id", "?") for j in jobs]

        results: List[dict] = []
        max_workers = min(self.throttle.recommended_workers(), len(jobs))
        cache_dir = self.config.cache_dir

        if max_workers <= 1 or len(jobs) == 1:
            for job in jobs:
                t0 = time.time()
                result = route_job(job, cache_dir)
                result["execution_time"] = round(time.time() - t0, 2)
                result["worker_id"] = self.worker_id
                results.append(result)
            self._active_job_ids = []
            return results

        try:
            with ProcessPoolExecutor(max_workers=max_workers) as executor:
                future_to_job = {}
                for job in jobs:
                    fut = executor.submit(route_job, job, cache_dir)
                    future_to_job[fut] = job.get("job_id", "?")

                timeout = self.config.job_timeout * len(jobs)
                for future in as_completed(future_to_job, timeout=timeout):
                    job_id = future_to_job[future]
                    try:
                        result = future.result(timeout=self.config.job_timeout)
                        result["worker_id"] = self.worker_id
                        results.append(result)
                    except FuturesTimeout:
                        results.append({
                            "job_id": job_id, "status": "failed",
                            "error": f"Timed out after {self.config.job_timeout}s",
                            "worker_id": self.worker_id,
                        })
                    except Exception as e:
                        results.append({
                            "job_id": job_id, "status": "failed",
                            "error": str(e), "worker_id": self.worker_id,
                        })
        except Exception as e:
            log.error("Batch execution error: %s", e)
            completed_ids = {r.get("job_id") for r in results}
            for job in jobs:
                jid = job.get("job_id", "?")
                if jid not in completed_ids:
                    results.append({
                        "job_id": jid, "status": "failed",
                        "error": f"Batch error: {e}", "worker_id": self.worker_id,
                    })
        finally:
            self._active_job_ids = []

        return results

    def _report_results(self, results: List[dict]):
        for result in results:
            job_id = result.get("job_id", "unknown")
            duration = result.get("execution_time", 0)
            try:
                if result.get("status") == "completed":
                    self.client.complete(job_id, metrics=result.get("metrics", {}),
                                         duration=duration)
                    self.stats["completed"] += 1
                else:
                    self.client.fail(job_id, result.get("error", "unknown"),
                                     duration=duration)
                    self.stats["failed"] += 1
            except Exception as e:
                log.error("Failed to report result for job %s: %s", job_id, e)
                self.stats["failed"] += 1

    def run(self):
        """Main loop: register -> dequeue -> execute -> report -> repeat."""
        self.stats["started_at"] = time.time()

        log.info("=" * 70)
        log.info("Aura Alpha Grid Worker v%s starting", __version__)
        log.info("Worker ID: %s", self.worker_id)
        log.info("Coordinator: %s", self.config.coordinator_url)
        log.info("CPUs: %d | RAM: %.1f GB | Parallel: %d | Batch: %d",
                 self.config.cpu_count, self.config.ram_gb,
                 self.config.max_parallel, self.config.batch_size)
        log.info("Cache: %s", self.config.cache_dir)
        log.info("Adaptive throttle: ON (yields to games, apps, heavy processes)")
        log.info("=" * 70)

        # Graceful shutdown
        def _signal_handler(signum, frame):
            log.info("Received signal %d, shutting down gracefully...", signum)
            self._shutdown.set()

        signal.signal(signal.SIGINT, _signal_handler)
        signal.signal(signal.SIGTERM, _signal_handler)

        # Register with coordinator (retry up to 5 times)
        for attempt in range(5):
            if self.client.register(self._capabilities()):
                break
            log.warning("Registration attempt %d/5 failed, retrying in %ds...",
                        attempt + 1, 3 * (attempt + 1))
            time.sleep(3 * (attempt + 1))
        else:
            log.error("Failed to register after 5 attempts. Check your network and coordinator URL.")
            return

        # Start heartbeat daemon thread
        hb_thread = Thread(target=self._heartbeat_loop, daemon=True, name="heartbeat")
        hb_thread.start()

        # Main dequeue-execute loop with exponential backoff
        idle_backoff = 1
        max_backoff = 30

        while not self._shutdown.is_set():
            try:
                # Throttle check
                recommended = self.throttle.recommended_workers()
                batch_count = self.config.batch_size
                if recommended < self.config.max_parallel:
                    ratio = recommended / max(self.config.max_parallel, 1)
                    batch_count = max(1, int(self.config.batch_size * ratio))
                    if ratio <= 0.25:
                        self._shutdown.wait(timeout=5)
                        if self._shutdown.is_set():
                            break

                # Dequeue
                jobs = self.client.dequeue(count=batch_count)
                if not jobs:
                    log.debug("No jobs available, sleeping %ds", idle_backoff)
                    self._shutdown.wait(timeout=idle_backoff)
                    idle_backoff = min(idle_backoff * 2, max_backoff)
                    continue

                idle_backoff = 1
                throttle_tag = (f" [throttled {recommended}/{self.config.max_parallel}]"
                                if self.throttle.is_throttled else "")
                log.info("Dequeued %d jobs%s", len(jobs), throttle_tag)

                # Execute
                batch_start = time.time()
                results = self._execute_batch(jobs)
                batch_elapsed = time.time() - batch_start

                # Report
                self._report_results(results)

                completed = sum(1 for r in results if r.get("status") == "completed")
                failed = len(results) - completed
                log.info("Batch done: %d completed, %d failed in %.1fs (%.1f jobs/min)%s",
                         completed, failed, batch_elapsed, self._throughput(), throttle_tag)

            except KeyboardInterrupt:
                log.info("Worker interrupted.")
                break
            except Exception as e:
                log.error("Worker loop error: %s", e)
                log.debug(traceback.format_exc())
                self._shutdown.wait(timeout=5)

        self._shutdown.set()
        log.info("=" * 70)
        log.info("Worker %s stopped. Completed: %d | Failed: %d | Throughput: %.1f/min",
                 self.worker_id, self.stats["completed"],
                 self.stats["failed"], self._throughput())
        log.info("=" * 70)


# ============================================================================
# CLI Entry Point
# ============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Aura Alpha Grid Worker -- distributed compute node",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  python worker.py
  python worker.py --max-parallel 4
  python worker.py --coordinator-url https://auraalpha.cc --verbose
  python worker.py --token MY_TOKEN

Token resolution order:
  1. --token CLI flag
  2. GRID_WORKER_TOKEN or AURA_TOKEN env var
  3. Stored token from previous auto-provision (~/.aura-worker/grid_token.json)
  4. Auto-provision from coordinator (zero setup required)
""",
    )
    parser.add_argument("--coordinator-url", type=str, default="",
                        help="Coordinator URL (default: from .env or https://auraalpha.cc)")
    parser.add_argument("--token", type=str, default="",
                        help="Worker token (default: auto-provisioned)")
    parser.add_argument("--max-parallel", type=int, default=0,
                        help="Max parallel jobs (0 = auto from CPU count)")
    parser.add_argument("--batch-size", type=int, default=0,
                        help="Jobs to pull per batch (default: 5)")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Enable debug logging")

    args = parser.parse_args()

    # Logging
    level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    # Load .env if present (simple key=value parser, no dependency)
    env_path = Path(__file__).resolve().parent / ".env"
    if env_path.exists():
        try:
            for line in env_path.read_text().splitlines():
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    key, _, val = line.partition("=")
                    key = key.strip()
                    val = val.strip().strip('"').strip("'")
                    if key and val and key not in os.environ:
                        os.environ[key] = val
        except Exception:
            pass

    # Build config from defaults + env
    coordinator_url = (args.coordinator_url
                       or os.getenv("COORDINATOR_URL")
                       or os.getenv("AURA_COORDINATOR_URL")
                       or "https://auraalpha.cc")
    max_parallel = args.max_parallel or int(os.getenv("MAX_PARALLEL", "0"))
    batch_size = args.batch_size or int(os.getenv("BATCH_SIZE", "5"))

    config = WorkerConfig(
        coordinator_url=coordinator_url,
        max_parallel=max_parallel,
        batch_size=batch_size,
        verbose=args.verbose,
    )

    # Resolve token
    try:
        token, worker_id = resolve_token(coordinator_url, args.token or None)
    except Exception as e:
        log.error("Cannot obtain worker token: %s", e)
        log.error("Check your network connection and coordinator URL.")
        sys.exit(1)

    # Run
    worker = GridWorker(config, token, worker_id)
    worker.run()


if __name__ == "__main__":
    main()
