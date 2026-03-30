#!/usr/bin/env python3
"""
Aura Alpha Compute Worker — Sidecar for Tauri Desktop
======================================================

Invoked by the Rust job_executor via subprocess:
    python3 compute_worker.py --job-type backtest --params '{"job_id": "...", ...}'

Reads job params from --params (JSON string), executes the computation,
prints a single JSON line to stdout, then exits.

Exit codes:
    0 = success (stdout contains JSON result)
    1 = execution error (stdout contains JSON with "status": "failed")
    2 = bad arguments / missing dependencies

Supported job types:
    backtest       — Run strategy backtest on OHLCV data
    scan           — Signal detection on symbol(s)
    ml_inference   — ML model prediction
    feature_extraction — Compute features for ML pipeline

Dependencies: numpy, polars (for parquet reading)
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import traceback
from pathlib import Path
from typing import Any, Dict, List, Optional

# ============================================================================
# Data loading
# ============================================================================

def _get_cache_dir() -> Path:
    """Resolve the OHLCV data cache directory."""
    env_dir = os.environ.get("AURA_CACHE_DIR")
    if env_dir:
        return Path(env_dir)
    return Path.home() / ".aura-worker" / "data"


def _load_bars(symbol: str, region: str, cache_dir: Path) -> Optional[dict]:
    """Load OHLCV bars from cached parquet. Returns dict or None."""
    try:
        import numpy as np
        import polars as pl
    except ImportError:
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


# ============================================================================
# Indicators
# ============================================================================

def _compute_atr(highs, lows, closes, period: int = 14):
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


def _compute_ema(data, period):
    import numpy as np
    n = len(data)
    ema = np.full(n, np.nan)
    if n <= period:
        return ema
    alpha = 2.0 / (period + 1)
    ema[period - 1] = np.mean(data[:period])
    for i in range(period, n):
        ema[i] = data[i] * alpha + ema[i - 1] * (1 - alpha)
    return ema


def _compute_rsi(closes, period=14):
    import numpy as np
    n = len(closes)
    rsi = np.full(n, 50.0)
    if n <= period + 1:
        return rsi
    deltas = np.diff(closes)
    gains = np.where(deltas > 0, deltas, 0.0)
    losses_arr = np.where(deltas < 0, -deltas, 0.0)
    avg_gain = np.mean(gains[:period])
    avg_loss = np.mean(losses_arr[:period])
    for i in range(period, len(deltas)):
        avg_gain = (avg_gain * (period - 1) + gains[i]) / period
        avg_loss = (avg_loss * (period - 1) + losses_arr[i]) / period
        rs = avg_gain / (avg_loss + 1e-10)
        rsi[i + 1] = 100.0 - (100.0 / (1.0 + rs))
    return rsi


def _compute_bbands(closes, period=20, std_mult=2.0):
    import numpy as np
    n = len(closes)
    upper = np.full(n, np.nan)
    middle = np.full(n, np.nan)
    lower = np.full(n, np.nan)
    for i in range(period - 1, n):
        window = closes[i - period + 1:i + 1]
        m = np.mean(window)
        s = np.std(window)
        middle[i] = m
        upper[i] = m + std_mult * s
        lower[i] = m - std_mult * s
    return upper, middle, lower


def _compute_sma(data, period):
    import numpy as np
    n = len(data)
    sma = np.full(n, np.nan)
    for i in range(period - 1, n):
        sma[i] = np.mean(data[i - period + 1:i + 1])
    return sma


def _compute_obv(closes, volumes):
    import numpy as np
    n = len(closes)
    obv = np.zeros(n)
    for i in range(1, n):
        if closes[i] > closes[i - 1]:
            obv[i] = obv[i - 1] + volumes[i]
        elif closes[i] < closes[i - 1]:
            obv[i] = obv[i - 1] - volumes[i]
        else:
            obv[i] = obv[i - 1]
    return obv


# ============================================================================
# Entry logic (matches EC2 behavior -- 50% condition threshold)
# ============================================================================

def _check_entry(i, entry_logic, indicators, params, direction):
    import numpy as np
    ind = indicators
    conditions_met = 0
    conditions_total = len(entry_logic) if entry_logic else 1

    for cond in entry_logic:
        try:
            if cond == "ema_cross_up":
                ef, es = ind["ema_fast"], ind["ema_slow"]
                if not np.isnan(ef[i]) and not np.isnan(es[i]) and i > 0:
                    if ef[i] > es[i] and ef[i - 1] <= es[i - 1]:
                        conditions_met += 1
            elif cond == "ema_cross_down":
                ef, es = ind["ema_fast"], ind["ema_slow"]
                if not np.isnan(ef[i]) and not np.isnan(es[i]) and i > 0:
                    if ef[i] < es[i] and ef[i - 1] >= es[i - 1]:
                        conditions_met += 1
            elif cond == "rsi_above_threshold":
                thr = params.get("rsi_entry_threshold", params.get("rsi_entry", 55))
                if ind["rsi"][i] > thr:
                    conditions_met += 1
            elif cond == "rsi_oversold":
                thr = params.get("rsi_oversold", 30)
                if ind["rsi"][i] < thr:
                    conditions_met += 1
            elif cond == "rsi_above_floor":
                thr = params.get("rsi_floor", 40)
                if ind["rsi"][i] > thr:
                    conditions_met += 1
            elif cond in ("volume_surge", "volume_spike"):
                vm = params.get("volume_multiplier", params.get("volume_spike_mult", 1.5))
                vs = ind["vol_sma"]
                vols = ind["volumes"]
                if not np.isnan(vs[i]) and vs[i] > 0 and vols[i] > vs[i] * vm:
                    conditions_met += 1
            elif cond == "price_above_high":
                lb = params.get("lookback_period", 20)
                if i >= lb and ind["closes"][i] > np.max(ind["highs"][i - lb:i]):
                    conditions_met += 1
            elif cond == "volume_breakout":
                vm = params.get("volume_multiplier", 2.0)
                vs = ind["vol_sma"]
                vols = ind["volumes"]
                if not np.isnan(vs[i]) and vs[i] > 0 and vols[i] > vs[i] * vm:
                    conditions_met += 1
            elif cond == "consolidation_check":
                cd = params.get("consolidation_days", 10)
                rp = params.get("consolidation_range_pct", 0.05)
                if i >= cd:
                    hi = np.max(ind["highs"][i - cd:i])
                    lo = np.min(ind["lows"][i - cd:i])
                    if lo > 0 and (hi - lo) / lo < rp:
                        conditions_met += 1
            elif cond in ("squeeze_fire", "band_expansion"):
                bb_u = ind.get("bb_upper")
                bb_m = ind.get("bb_middle")
                bb_l = ind.get("bb_lower")
                if bb_u is not None and not np.isnan(bb_u[i]) and not np.isnan(bb_l[i]):
                    squeeze = params.get("squeeze_threshold", 0.02)
                    width = (bb_u[i] - bb_l[i]) / (bb_m[i] + 1e-10)
                    if i > 0:
                        prev_width = (bb_u[i-1] - bb_l[i-1]) / (bb_m[i-1] + 1e-10) if not np.isnan(bb_u[i-1]) else width
                        if prev_width < squeeze and width > squeeze:
                            conditions_met += 1
            elif cond == "direction_filter":
                ef = ind.get("ema_fast")
                if ef is not None and not np.isnan(ef[i]):
                    if (direction == "long" and ind["closes"][i] > ef[i]) or \
                       (direction != "long" and ind["closes"][i] < ef[i]):
                        conditions_met += 1
            elif cond == "zscore_extreme":
                lb = params.get("spread_lookback", 30)
                ez = params.get("entry_zscore", 2.0)
                if i >= lb:
                    window = ind["closes"][i - lb:i]
                    m, s = np.mean(window), np.std(window)
                    if s > 0:
                        z = (ind["closes"][i] - m) / s
                        if abs(z) >= ez:
                            conditions_met += 1
            elif cond in ("correlation_stable",):
                conditions_met += 1
            elif cond in ("top_sector_rank", "momentum_positive"):
                lb = params.get("ranking_period", 30)
                if i >= lb and ind["closes"][i] > ind["closes"][i - lb]:
                    conditions_met += 1
            elif cond == "gap_detection":
                gt = params.get("gap_threshold_pct", 0.03)
                if i > 0 and abs(ind["closes"][i] / ind["closes"][i - 1] - 1) > gt:
                    conditions_met += 1
            elif cond == "event_window":
                conditions_met += 1
            elif cond == "obv_rising":
                obv = ind.get("obv")
                if obv is not None and i > 5:
                    if obv[i] > obv[i - 5]:
                        conditions_met += 1
            elif cond == "price_below_lower_band":
                bb_l = ind.get("bb_lower")
                if bb_l is not None and not np.isnan(bb_l[i]):
                    if ind["closes"][i] < bb_l[i]:
                        conditions_met += 1
            elif cond == "distance_from_sma":
                sma = ind.get("sma")
                md = params.get("min_distance_from_sma", 0.02)
                if sma is not None and not np.isnan(sma[i]) and sma[i] > 0:
                    dist = abs(ind["closes"][i] - sma[i]) / sma[i]
                    if dist >= md:
                        conditions_met += 1
            elif cond == "price_above_vwap":
                lb = params.get("volume_sma_period", 20)
                if i >= lb:
                    w = ind["closes"][i - lb:i] * ind["volumes"][i - lb:i]
                    vwap = np.sum(w) / (np.sum(ind["volumes"][i - lb:i]) + 1e-10)
                    if ind["closes"][i] > vwap:
                        conditions_met += 1
            elif cond in ("bollinger_squeeze", "rsi_confirmation", "volume_confirm"):
                conditions_met += 1
            else:
                conditions_met += 1
        except Exception:
            pass

    return conditions_met >= max(1, conditions_total * 0.5)


# ============================================================================
# Trade simulation
# ============================================================================

def _simulate_trades(
    closes, highs, lows, dates: List[str],
    params: dict, direction: str = "long",
    date_start: str = "", date_end: str = "",
    entry_logic: Optional[List[str]] = None,
) -> List[dict]:
    """Simulate trades using strategy-specific entry logic and ATR-based exits."""
    import numpy as np

    n = len(closes)
    if n < 50:
        return []

    stop_atr = params.get("stop_loss_atr_mult", 2.0)
    tp_atr = params.get("take_profit_atr_mult", 4.0)
    trail_pct = params.get("trailing_stop_pct", 0.05)
    if trail_pct > 1:
        trail_pct = trail_pct / 100.0
    max_hold = params.get("max_hold_days", 30)
    atr_period = params.get("atr_period", 14)
    ema_fast_period = params.get("ema_fast", 9)
    ema_slow_period = params.get("ema_slow", 21)
    rsi_period = params.get("rsi_period", 14)
    vol_sma_period = params.get("volume_sma_period", 20)
    bbands_period = params.get("bbands_period", 20)
    bbands_std = params.get("bbands_std", 2.0)

    if not entry_logic:
        entry_logic = ["ema_cross_up", "rsi_above_threshold", "volume_surge"]

    volumes = np.zeros(n)
    if "_volumes" in params:
        volumes = np.array(params["_volumes"], dtype=float)

    indicators = {
        "closes": closes,
        "highs": highs,
        "lows": lows,
        "volumes": volumes,
        "atr": _compute_atr(highs, lows, closes, atr_period),
        "ema_fast": _compute_ema(closes, ema_fast_period),
        "ema_slow": _compute_ema(closes, ema_slow_period),
        "rsi": _compute_rsi(closes, rsi_period),
        "vol_sma": _compute_sma(volumes, vol_sma_period) if volumes.any() else np.full(n, np.nan),
        "sma": _compute_sma(closes, bbands_period),
        "obv": _compute_obv(closes, volumes) if volumes.any() else np.zeros(n),
    }

    needs_bb = any(c in str(entry_logic) for c in ["squeeze", "band", "bollinger", "lower_band"])
    if needs_bb:
        bb_u, bb_m, bb_l = _compute_bbands(closes, bbands_period, bbands_std)
        indicators["bb_upper"] = bb_u
        indicators["bb_middle"] = bb_m
        indicators["bb_lower"] = bb_l

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

    min_lookback = max(ema_slow_period, atr_period, rsi_period, vol_sma_period, bbands_period) + 5
    start_idx = max(start_idx, min_lookback)

    trades: List[dict] = []
    in_trade = False
    entry_price = 0.0
    entry_idx = 0
    stop_price = 0.0
    tp_price = 0.0
    trail_high = 0.0
    trail_low = float("inf")

    atr = indicators["atr"]

    for i in range(start_idx, min(end_idx, n)):
        if np.isnan(atr[i]):
            continue

        if not in_trade:
            if _check_entry(i, entry_logic, indicators, params, direction):
                entry_price = closes[i]
                entry_idx = i
                if direction == "long":
                    stop_price = entry_price - atr[i] * stop_atr
                    tp_price = entry_price + atr[i] * tp_atr
                    trail_high = entry_price
                else:
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
                pnl_pct = ((exit_price - entry_price) / entry_price if direction == "long"
                           else (entry_price - exit_price) / entry_price)
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


# ============================================================================
# Metrics
# ============================================================================

def _compute_metrics(trades: List[dict]) -> dict:
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


# ============================================================================
# Job executors
# ============================================================================

def execute_backtest(job: dict) -> dict:
    """Run a research backtest job against OHLCV data."""
    import random

    cache_dir = _get_cache_dir()
    params = job.get("params", {})

    # Support both flat params and nested job structure from coordinator
    job_id = job.get("job_id", "unknown")
    strategy_family = params.get("strategy_family", job.get("strategy_family", "unknown"))
    parameter_set = params.get("parameter_set", job.get("parameter_set", params))
    symbol_universe = params.get("symbol_universe", job.get("symbol_universe", []))
    date_window = params.get("date_window", job.get("date_window", ":"))
    backtest_config = params.get("backtest_config", job.get("backtest_config", {}))

    direction = backtest_config.get("direction", params.get("direction", "long"))
    region = backtest_config.get("region", params.get("region", "us"))
    date_parts = date_window.split(":")
    date_start = date_parts[0] if len(date_parts) > 0 else ""
    date_end = date_parts[1] if len(date_parts) > 1 else ""

    # Extract entry_logic
    entry_logic = params.get("entry_logic")
    if not entry_logic:
        payload = job.get("payload", {})
        if isinstance(payload, dict):
            candidate = payload.get("candidate", {})
            entry_logic = candidate.get("entry_logic")
    if not entry_logic:
        entry_logic = parameter_set.get("entry_logic") if isinstance(parameter_set, dict) else None

    # Auto-sample symbols from cache if universe is empty
    if not symbol_universe:
        data_dir = cache_dir / region
        if data_dir.exists():
            available = [f.stem for f in data_dir.glob("*.parquet")]
            sample_size = min(15, len(available))
            symbol_universe = random.sample(available, sample_size) if available else []

    # Handle symbol as single string (from Rust flat params)
    symbol = params.get("symbol")
    if symbol and not symbol_universe:
        symbol_universe = [symbol]

    all_trades: List[dict] = []
    symbols_tested = 0
    symbols_skipped = 0

    for sym in symbol_universe:
        bars = _load_bars(sym, region, cache_dir)
        if bars is None or len(bars["closes"]) < 50:
            symbols_skipped += 1
            continue

        bt_params = dict(parameter_set) if isinstance(parameter_set, dict) else {}
        bt_params["_volumes"] = bars["volumes"].tolist()

        trades = _simulate_trades(
            closes=bars["closes"], highs=bars["highs"],
            lows=bars["lows"], dates=bars["dates"],
            params=bt_params, direction=direction,
            date_start=date_start, date_end=date_end,
            entry_logic=entry_logic,
        )
        all_trades.extend(trades)
        symbols_tested += 1

    metrics = _compute_metrics(all_trades)
    metrics["symbols_tested"] = symbols_tested
    metrics["symbols_skipped"] = symbols_skipped
    metrics["strategy_family"] = strategy_family
    metrics["date_window"] = date_window
    if isinstance(parameter_set, dict):
        metrics["mutation_id"] = parameter_set.get("_mutation_id", "unknown")
        metrics["parameter_set"] = {
            k: v for k, v in parameter_set.items() if not k.startswith("_")
        }

    return {
        "status": "completed",
        "job_type": "backtest",
        "metrics": metrics,
    }


def execute_scan(job: dict) -> dict:
    """Run signal detection on symbol(s).

    Uses the same indicator + entry logic as backtests but only checks
    the latest bar for signal generation.
    """
    import numpy as np

    cache_dir = _get_cache_dir()
    params = job.get("params", {})
    symbols = params.get("symbols", [])
    symbol = params.get("symbol")
    if symbol and not symbols:
        symbols = [symbol]
    region = params.get("region", "us")
    scan_type = params.get("scan_type", "momentum")

    # Map scan_type to entry logic
    scan_logic_map = {
        "momentum": ["ema_cross_up", "rsi_above_threshold", "volume_surge"],
        "reversal": ["rsi_oversold", "price_below_lower_band", "volume_spike"],
        "breakout": ["price_above_high", "volume_breakout", "band_expansion"],
        "volume": ["volume_surge", "obv_rising", "volume_breakout"],
    }
    entry_logic = params.get("entry_logic", scan_logic_map.get(scan_type, scan_logic_map["momentum"]))

    signals = []
    for sym in symbols:
        bars = _load_bars(sym, region, cache_dir)
        if bars is None or len(bars["closes"]) < 50:
            continue

        closes = bars["closes"]
        highs = bars["highs"]
        lows = bars["lows"]
        volumes = bars["volumes"]
        n = len(closes)

        # Compute indicators for the last bar
        indicators = {
            "closes": closes,
            "highs": highs,
            "lows": lows,
            "volumes": volumes,
            "atr": _compute_atr(highs, lows, closes),
            "ema_fast": _compute_ema(closes, params.get("ema_fast", 9)),
            "ema_slow": _compute_ema(closes, params.get("ema_slow", 21)),
            "rsi": _compute_rsi(closes),
            "vol_sma": _compute_sma(volumes, 20) if volumes.any() else np.full(n, np.nan),
            "sma": _compute_sma(closes, 20),
            "obv": _compute_obv(closes, volumes) if volumes.any() else np.zeros(n),
        }

        needs_bb = any(c in str(entry_logic) for c in ["squeeze", "band", "bollinger", "lower_band"])
        if needs_bb:
            bb_u, bb_m, bb_l = _compute_bbands(closes)
            indicators["bb_upper"] = bb_u
            indicators["bb_middle"] = bb_m
            indicators["bb_lower"] = bb_l

        # Check last bar for signal
        last_idx = n - 1
        if not np.isnan(indicators["atr"][last_idx]):
            if _check_entry(last_idx, entry_logic, indicators, params, "long"):
                rsi_val = float(indicators["rsi"][last_idx]) if not np.isnan(indicators["rsi"][last_idx]) else 50.0
                signals.append({
                    "symbol": sym,
                    "signal": "buy",
                    "scan_type": scan_type,
                    "price": round(float(closes[last_idx]), 4),
                    "rsi": round(rsi_val, 2),
                    "date": bars["dates"][-1],
                    "confidence": min(0.95, 0.5 + rsi_val / 200),
                })

    return {
        "status": "completed",
        "job_type": "scan",
        "results": signals,
        "symbols_scanned": len(symbols),
        "signals_found": len(signals),
    }


def execute_ml_inference(job: dict) -> dict:
    """Run ML model inference.

    Uses a simple ensemble of technical indicators as a lightweight model
    when no trained model file is available.
    """
    import numpy as np

    cache_dir = _get_cache_dir()
    params = job.get("params", {})
    symbols = params.get("symbols", [])
    symbol = params.get("symbol")
    if symbol and not symbols:
        symbols = [symbol]
    model_name = params.get("model", "ensemble_technical")
    region = params.get("region", "us")

    predictions = []
    for sym in symbols:
        bars = _load_bars(sym, region, cache_dir)
        if bars is None or len(bars["closes"]) < 50:
            continue

        closes = bars["closes"]
        highs = bars["highs"]
        lows = bars["lows"]
        volumes = bars["volumes"]
        n = len(closes)

        # Compute features
        rsi = _compute_rsi(closes)
        ema_fast = _compute_ema(closes, 9)
        ema_slow = _compute_ema(closes, 21)
        atr = _compute_atr(highs, lows, closes)

        last = n - 1
        if np.isnan(rsi[last]) or np.isnan(ema_fast[last]) or np.isnan(ema_slow[last]):
            continue

        # Simple ensemble prediction: momentum + mean-reversion + volatility
        # Score from -1.0 (strong sell) to +1.0 (strong buy)
        momentum_score = 0.0
        if ema_fast[last] > ema_slow[last]:
            momentum_score = 0.3
        elif ema_fast[last] < ema_slow[last]:
            momentum_score = -0.3

        rsi_score = 0.0
        if rsi[last] < 30:
            rsi_score = 0.4  # oversold = buy signal
        elif rsi[last] > 70:
            rsi_score = -0.4  # overbought = sell signal
        else:
            rsi_score = (50 - rsi[last]) / 100  # slight mean-reversion bias

        # Trend strength
        returns_5d = (closes[last] - closes[max(0, last - 5)]) / closes[max(0, last - 5)]
        trend_score = max(-0.3, min(0.3, returns_5d * 5))

        prediction = round(momentum_score + rsi_score + trend_score, 4)
        confidence = round(min(0.95, abs(prediction) + 0.3), 4)

        predictions.append({
            "symbol": sym,
            "prediction": prediction,
            "direction": "long" if prediction > 0 else "short",
            "confidence": confidence,
            "model": model_name,
            "features": {
                "rsi": round(float(rsi[last]), 2),
                "ema_fast": round(float(ema_fast[last]), 4),
                "ema_slow": round(float(ema_slow[last]), 4),
                "returns_5d": round(float(returns_5d), 6),
            },
        })

    return {
        "status": "completed",
        "job_type": "ml_inference",
        "predictions": predictions,
        "model": model_name,
        "symbols_processed": len(predictions),
    }


def execute_feature_extraction(job: dict) -> dict:
    """Extract technical features for ML pipeline."""
    import numpy as np

    cache_dir = _get_cache_dir()
    params = job.get("params", {})
    symbols = params.get("symbols", [])
    symbol = params.get("symbol")
    if symbol and not symbols:
        symbols = [symbol]
    region = params.get("region", "us")
    dataset = params.get("dataset", "default")

    features_list = []
    for sym in symbols:
        bars = _load_bars(sym, region, cache_dir)
        if bars is None or len(bars["closes"]) < 50:
            continue

        closes = bars["closes"]
        highs = bars["highs"]
        lows = bars["lows"]
        volumes = bars["volumes"]
        n = len(closes)

        rsi = _compute_rsi(closes)
        ema9 = _compute_ema(closes, 9)
        ema21 = _compute_ema(closes, 21)
        atr = _compute_atr(highs, lows, closes)
        bb_u, bb_m, bb_l = _compute_bbands(closes)

        last = n - 1
        if np.isnan(rsi[last]) or np.isnan(ema9[last]):
            continue

        features_list.append({
            "symbol": sym,
            "date": bars["dates"][-1],
            "features": {
                "rsi_14": round(float(rsi[last]), 4),
                "ema_9": round(float(ema9[last]), 4),
                "ema_21": round(float(ema21[last]), 4),
                "atr_14": round(float(atr[last]), 4) if not np.isnan(atr[last]) else 0.0,
                "bb_width": round(float((bb_u[last] - bb_l[last]) / (bb_m[last] + 1e-10)), 4) if not np.isnan(bb_u[last]) else 0.0,
                "returns_1d": round(float((closes[last] - closes[last - 1]) / closes[last - 1]), 6) if last > 0 else 0.0,
                "returns_5d": round(float((closes[last] - closes[max(0, last - 5)]) / closes[max(0, last - 5)]), 6),
                "returns_20d": round(float((closes[last] - closes[max(0, last - 20)]) / closes[max(0, last - 20)]), 6),
                "volume_ratio": round(float(volumes[last] / (np.mean(volumes[max(0, last - 20):last]) + 1e-10)), 4) if volumes.any() else 1.0,
                "close": round(float(closes[last]), 4),
            },
        })

    return {
        "status": "completed",
        "job_type": "feature_extraction",
        "features_count": len(features_list),
        "dataset": dataset,
        "features": features_list,
    }


# ============================================================================
# Main dispatch
# ============================================================================

def main():
    parser = argparse.ArgumentParser(description="Aura Alpha Compute Worker Sidecar")
    parser.add_argument("--job-type", required=True,
                        choices=["backtest", "scan", "ml_inference", "feature_extraction",
                                 "health_check", "ping"],
                        help="Type of job to execute")
    parser.add_argument("--params", required=True,
                        help="JSON string with job parameters")
    parser.add_argument("--cache-dir", default="",
                        help="Override OHLCV cache directory")
    args = parser.parse_args()

    # Set cache dir override if provided
    if args.cache_dir:
        os.environ["AURA_CACHE_DIR"] = args.cache_dir

    try:
        job = json.loads(args.params)
    except json.JSONDecodeError as e:
        result = {"status": "failed", "error": f"Invalid JSON params: {e}"}
        print(json.dumps(result))
        sys.exit(1)

    t0 = time.time()

    try:
        if args.job_type == "backtest":
            result = execute_backtest(job)
        elif args.job_type == "scan":
            result = execute_scan(job)
        elif args.job_type == "ml_inference":
            result = execute_ml_inference(job)
        elif args.job_type == "feature_extraction":
            result = execute_feature_extraction(job)
        elif args.job_type == "health_check":
            import platform
            result = {
                "status": "completed",
                "job_type": "health_check",
                "platform": platform.system(),
                "arch": platform.machine(),
                "python_version": platform.python_version(),
                "has_numpy": _check_dep("numpy"),
                "has_polars": _check_dep("polars"),
            }
        elif args.job_type == "ping":
            result = {"status": "completed", "job_type": "ping", "pong": True}
        else:
            result = {"status": "failed", "error": f"Unknown job type: {args.job_type}"}

        result["compute_seconds"] = round(time.time() - t0, 3)
        print(json.dumps(result))
        sys.exit(0)

    except Exception as e:
        result = {
            "status": "failed",
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
            "compute_seconds": round(time.time() - t0, 3),
        }
        print(json.dumps(result))
        sys.exit(1)


def _check_dep(name: str) -> bool:
    try:
        __import__(name)
        return True
    except ImportError:
        return False


if __name__ == "__main__":
    main()
