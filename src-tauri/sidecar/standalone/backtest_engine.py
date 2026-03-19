"""
Backtest engine for standalone workers.
Adapted from distributed_research/worker_agent.py — self-contained with no
imports from the main project. Accepts a cache_dir parameter for data location.
"""
from __future__ import annotations

import logging
import traceback
from pathlib import Path
from typing import Any, Dict, List, Optional

import numpy as np

log = logging.getLogger("standalone.backtest_engine")


# ── Data Loading ──────────────────────────────────────────────────────────────


def _load_bars_for_symbol(symbol: str, region: str, cache_dir: Path) -> Optional[Dict]:
    """Load OHLCV bars from cached parquet for a single symbol.

    Returns dict with keys: dates, closes, volumes, highs, lows as numpy arrays,
    or None if data is unavailable.
    """
    try:
        import polars as pl
    except ImportError:
        log.error("polars not installed — cannot load parquet data")
        return None

    # Build search paths: region-specific first, then us fallback
    search_dirs: List[Path] = []
    if region == "crypto":
        search_dirs.append(cache_dir / "crypto")
    elif region == "us":
        search_dirs.append(cache_dir / "us")
    else:
        search_dirs.append(cache_dir / region)
        search_dirs.append(cache_dir / "us")  # fallback

    parquet_path: Optional[Path] = None
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


# ── Indicators ────────────────────────────────────────────────────────────────


def _compute_atr(highs: np.ndarray, lows: np.ndarray, closes: np.ndarray, period: int = 14) -> np.ndarray:
    """Compute Average True Range."""
    if len(highs) < period + 1:
        return np.full(len(highs), np.nan)
    tr = np.maximum(
        highs[1:] - lows[1:],
        np.maximum(
            np.abs(highs[1:] - closes[:-1]),
            np.abs(lows[1:] - closes[:-1]),
        ),
    )
    atr = np.full(len(highs), np.nan)
    if len(tr) >= period:
        atr[period] = np.mean(tr[:period])
        for i in range(period + 1, len(tr) + 1):
            atr[i] = (atr[i - 1] * (period - 1) + tr[i - 1]) / period
    return atr


# ── Trade Simulation ─────────────────────────────────────────────────────────


def _simulate_trades(
    closes: np.ndarray,
    highs: np.ndarray,
    lows: np.ndarray,
    dates: List[str],
    params: Dict[str, Any],
    direction: str = "long",
    date_start: str = "",
    date_end: str = "",
) -> List[Dict[str, Any]]:
    """Simulate trades for a single symbol with given parameters.

    Uses ATR-based stops and take-profits, EMA crossover entries,
    RSI confirmation, and optional volume filters.
    Returns list of trade dicts.
    """
    n = len(closes)
    if n < 50:
        return []

    # Extract parameters with defaults
    stop_atr = params.get("stop_loss_atr_mult", 2.0)
    tp_atr = params.get("take_profit_atr_mult", 4.0)
    trail_pct = params.get("trailing_stop_pct", 0.05)
    max_hold = params.get("max_hold_days", 30)
    atr_period = params.get("atr_period", 14)

    # EMA parameters (for entry signals)
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
        losses = np.where(deltas < 0, -deltas, 0.0)
        avg_gain = np.mean(gains[:rsi_period])
        avg_loss = np.mean(losses[:rsi_period])
        for i in range(rsi_period, len(deltas)):
            avg_gain = (avg_gain * (rsi_period - 1) + gains[i]) / rsi_period
            avg_loss = (avg_loss * (rsi_period - 1) + losses[i]) / rsi_period
            rs = avg_gain / (avg_loss + 1e-10)
            rsi[i + 1] = 100.0 - (100.0 / (1.0 + rs))

    # Volume SMA
    vol_sma = np.full(n, np.nan)
    volumes = lows * 0  # placeholder
    if "_volumes" in params:
        volumes = np.array(params["_volumes"], dtype=float)
        if len(volumes) >= vol_sma_period:
            for i in range(vol_sma_period - 1, n):
                vol_sma[i] = np.mean(volumes[i - vol_sma_period + 1 : i + 1])

    # Date window filtering
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

    # Ensure we have enough lookback
    min_lookback = max(ema_slow_period, atr_period, rsi_period, vol_sma_period) + 5
    start_idx = max(start_idx, min_lookback)

    trades: List[Dict[str, Any]] = []
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
            # ── Entry logic ──
            if direction == "long":
                cross_up = ema_fast[i] > ema_slow[i] and (
                    i > 0 and ema_fast[i - 1] <= ema_slow[i - 1]
                )
                rsi_ok = rsi[i] > rsi_threshold
                vol_ok = True
                if not np.isnan(vol_sma[i]) and vol_sma[i] > 0:
                    vol_ok = volumes[i] > vol_sma[i] * vol_mult if len(volumes) > i else True

                if cross_up and rsi_ok and vol_ok:
                    entry_price = closes[i]
                    entry_idx = i
                    stop_price = entry_price - atr[i] * stop_atr
                    tp_price = entry_price + atr[i] * tp_atr
                    trail_high = entry_price
                    in_trade = True
            else:  # short
                cross_down = ema_fast[i] < ema_slow[i] and (
                    i > 0 and ema_fast[i - 1] >= ema_slow[i - 1]
                )
                rsi_ok = rsi[i] < (100.0 - rsi_threshold)
                vol_ok = True
                if not np.isnan(vol_sma[i]) and vol_sma[i] > 0:
                    vol_ok = volumes[i] > vol_sma[i] * vol_mult if len(volumes) > i else True

                if cross_down and rsi_ok and vol_ok:
                    entry_price = closes[i]
                    entry_idx = i
                    stop_price = entry_price + atr[i] * stop_atr
                    tp_price = entry_price - atr[i] * tp_atr
                    trail_low = entry_price
                    in_trade = True
        else:
            # ── Exit logic ──
            hold_days = i - entry_idx
            exit_price = None
            exit_reason = ""

            if direction == "long":
                trail_high = max(trail_high, highs[i])
                trail_stop = trail_high * (1.0 - trail_pct)

                if lows[i] <= stop_price:
                    exit_price = stop_price
                    exit_reason = "stop_loss"
                elif highs[i] >= tp_price:
                    exit_price = tp_price
                    exit_reason = "take_profit"
                elif closes[i] <= trail_stop and hold_days > 1:
                    exit_price = trail_stop
                    exit_reason = "trailing_stop"
                elif hold_days >= max_hold:
                    exit_price = closes[i]
                    exit_reason = "max_hold"
            else:  # short
                trail_low = min(trail_low, lows[i])
                trail_stop = trail_low * (1.0 + trail_pct)

                if highs[i] >= stop_price:
                    exit_price = stop_price
                    exit_reason = "stop_loss"
                elif lows[i] <= tp_price:
                    exit_price = tp_price
                    exit_reason = "take_profit"
                elif closes[i] >= trail_stop and hold_days > 1:
                    exit_price = trail_stop
                    exit_reason = "trailing_stop"
                elif hold_days >= max_hold:
                    exit_price = closes[i]
                    exit_reason = "max_hold"

            if exit_price is not None:
                if direction == "long":
                    pnl_pct = (exit_price - entry_price) / entry_price
                else:
                    pnl_pct = (entry_price - exit_price) / entry_price

                trades.append(
                    {
                        "entry_date": dates[entry_idx],
                        "exit_date": dates[i],
                        "entry_price": round(entry_price, 4),
                        "exit_price": round(exit_price, 4),
                        "pnl_pct": round(pnl_pct, 6),
                        "hold_days": hold_days,
                        "exit_reason": exit_reason,
                        "direction": direction,
                    }
                )
                in_trade = False

    # Close any open trade at end of window
    if in_trade and end_idx > entry_idx:
        final_idx = min(end_idx - 1, n - 1)
        exit_price = closes[final_idx]
        if direction == "long":
            pnl_pct = (exit_price - entry_price) / entry_price
        else:
            pnl_pct = (entry_price - exit_price) / entry_price
        trades.append(
            {
                "entry_date": dates[entry_idx],
                "exit_date": dates[final_idx],
                "entry_price": round(entry_price, 4),
                "exit_price": round(exit_price, 4),
                "pnl_pct": round(pnl_pct, 6),
                "hold_days": final_idx - entry_idx,
                "exit_reason": "window_end",
                "direction": direction,
            }
        )

    return trades


# ── Metrics ───────────────────────────────────────────────────────────────────


def _compute_metrics(trades: List[Dict]) -> Dict[str, Any]:
    """Compute standardized performance metrics from a list of trades."""
    if not trades:
        return {
            "num_trades": 0,
            "sharpe": 0.0,
            "sortino": 0.0,
            "profit_factor": 0.0,
            "win_rate": 0.0,
            "avg_return": 0.0,
            "max_drawdown": 0.0,
            "total_return": 0.0,
            "avg_hold_days": 0.0,
        }

    returns = [t["pnl_pct"] for t in trades]
    n = len(returns)
    wins = sum(1 for r in returns if r > 0)
    losses = sum(1 for r in returns if r <= 0)

    avg_ret = np.mean(returns)
    std_ret = np.std(returns, ddof=1) if n > 1 else 1e-9
    downside = (
        np.std([r for r in returns if r < 0], ddof=1)
        if any(r < 0 for r in returns)
        else 1e-9
    )

    sharpe = float(avg_ret / (std_ret + 1e-9))
    sortino = float(avg_ret / (downside + 1e-9))

    gross_profit = sum(r for r in returns if r > 0)
    gross_loss = abs(sum(r for r in returns if r < 0))
    profit_factor = float(gross_profit / (gross_loss + 1e-9))

    # Max drawdown from cumulative returns
    cum = np.cumprod(1 + np.array(returns))
    peak = np.maximum.accumulate(cum)
    dd = (cum - peak) / peak
    max_dd = float(np.min(dd)) if len(dd) > 0 else 0.0

    total_ret = float(cum[-1] - 1) if len(cum) > 0 else 0.0
    avg_hold = np.mean([t["hold_days"] for t in trades])

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
        "avg_return": round(float(avg_ret), 6),
        "max_drawdown": round(max_dd, 6),
        "total_return": round(total_ret, 6),
        "avg_hold_days": round(float(avg_hold), 1),
        "gross_profit": round(gross_profit, 6),
        "gross_loss": round(gross_loss, 6),
        "wins": wins,
        "losses": losses,
        "exit_reasons": exit_reasons,
    }


# ── Job Runner ────────────────────────────────────────────────────────────────


def run_single_research_job(job_dict: Dict[str, Any], cache_dir: Path) -> Dict[str, Any]:
    """Top-level function for ProcessPoolExecutor — must be picklable.

    Accepts a job dict (from the coordinator API) and a cache_dir Path.
    Runs the backtest across all symbols in the job's universe and returns
    aggregated results.

    Expected job_dict fields:
        job_id            str
        strategy_family   str
        parameter_set     dict
        symbol_universe   list[str]
        date_window       str  e.g. "2020-01-01:2025-12-31"
        backtest_config   dict with 'direction', 'region'
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

        all_trades: List[Dict] = []
        symbols_tested = 0
        symbols_skipped = 0

        for symbol in symbol_universe:
            bars = _load_bars_for_symbol(symbol, region, cache_dir)
            if bars is None or len(bars["closes"]) < 50:
                symbols_skipped += 1
                continue

            # Inject volumes into params for the simulator
            params = dict(parameter_set)
            params["_volumes"] = bars["volumes"].tolist()

            trades = _simulate_trades(
                closes=bars["closes"],
                highs=bars["highs"],
                lows=bars["lows"],
                dates=bars["dates"],
                params=params,
                direction=direction,
                date_start=date_start,
                date_end=date_end,
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

        return {
            "job_id": job_id,
            "status": "completed",
            "metrics": metrics,
        }

    except Exception as e:
        return {
            "job_id": job_dict.get("job_id", "unknown"),
            "status": "failed",
            "error": f"{type(e).__name__}: {str(e)}",
            "traceback": traceback.format_exc(),
        }
