"""
Strategy Runner — Runs limited strategy set against local OHLCV data
=====================================================================
Standalone mode: 15 free strategies, US market only.
Generates buy/sell signals with confidence scores.

This module wraps the strategy detection logic. In production,
the detection code would be compiled via PyInstaller.
For now, it implements a simplified version that produces
realistic signals from OHLCV price action.
"""
import logging
import math
from datetime import datetime
from typing import Optional

log = logging.getLogger("strategy-runner")

# Free strategies available in standalone mode
FREE_STRATEGIES = {
    "breakout": {"direction": "long", "lookback": 20, "threshold": 1.02},
    "bullish_pullback": {"direction": "long", "lookback": 10, "threshold": 0.97},
    "the_5_day_bounce": {"direction": "long", "lookback": 5, "threshold": 0.95},
    "bearish_engulfing": {"direction": "short", "lookback": 2, "threshold": 1.0},
    "ascending_triangle": {"direction": "long", "lookback": 15, "threshold": 1.01},
    "avwap_bounce": {"direction": "long", "lookback": 10, "threshold": 0.98},
    "bear_flag": {"direction": "short", "lookback": 10, "threshold": 0.99},
    "nice_chart": {"direction": "long", "lookback": 20, "threshold": 1.0},
    "double_bottom": {"direction": "long", "lookback": 20, "threshold": 0.96},
    "trend_play": {"direction": "long", "lookback": 10, "threshold": 1.01},
    "gap_and_go": {"direction": "long", "lookback": 1, "threshold": 1.02},
    "snap_back_long": {"direction": "long", "lookback": 5, "threshold": 0.94},
    "slippery_slope": {"direction": "short", "lookback": 10, "threshold": 0.98},
    "topping_formation": {"direction": "short", "lookback": 15, "threshold": 0.99},
    "failed_bounce": {"direction": "short", "lookback": 5, "threshold": 0.97},
}


def _compute_sma(closes: list, period: int) -> float:
    if len(closes) < period:
        return closes[-1] if closes else 0
    return sum(closes[-period:]) / period


def _compute_atr(bars: list, period: int = 14) -> float:
    if len(bars) < period + 1:
        return 0
    trs = []
    for i in range(1, len(bars)):
        h = bars[i]["high"]
        l = bars[i]["low"]
        pc = bars[i - 1]["close"]
        tr = max(h - l, abs(h - pc), abs(l - pc))
        trs.append(tr)
    return sum(trs[-period:]) / period if trs else 0


def _detect_breakout(bars: list, params: dict) -> Optional[dict]:
    if len(bars) < params["lookback"] + 1:
        return None
    lookback = params["lookback"]
    recent_high = max(b["high"] for b in bars[-lookback - 1:-1])
    current = bars[-1]["close"]
    if current > recent_high * params["threshold"]:
        conf = min(0.5 + (current / recent_high - 1) * 10, 0.95)
        return {"confidence": round(conf, 2), "direction": "long"}
    return None


def _detect_pullback(bars: list, params: dict) -> Optional[dict]:
    if len(bars) < 20:
        return None
    sma20 = _compute_sma([b["close"] for b in bars], 20)
    current = bars[-1]["close"]
    prev = bars[-2]["close"]
    # Price pulled back below SMA but bouncing
    if prev < sma20 and current > prev and current < sma20 * 1.02:
        conf = min(0.5 + abs(current - prev) / current * 20, 0.9)
        return {"confidence": round(conf, 2), "direction": params["direction"]}
    return None


def _detect_engulfing(bars: list, params: dict) -> Optional[dict]:
    if len(bars) < 3:
        return None
    prev = bars[-2]
    curr = bars[-1]
    # Bearish engulfing
    if params["direction"] == "short":
        if prev["close"] > prev["open"] and curr["close"] < curr["open"]:
            if curr["open"] > prev["close"] and curr["close"] < prev["open"]:
                body = abs(curr["close"] - curr["open"])
                conf = min(0.55 + body / curr["open"] * 10, 0.85)
                return {"confidence": round(conf, 2), "direction": "short"}
    return None


def _detect_generic(bars: list, params: dict) -> Optional[dict]:
    """Generic momentum/mean-reversion detector for remaining strategies."""
    if len(bars) < params["lookback"] + 1:
        return None
    lookback = params["lookback"]
    closes = [b["close"] for b in bars]
    sma = _compute_sma(closes, lookback)
    current = closes[-1]
    atr = _compute_atr(bars)
    if atr == 0:
        return None

    if params["direction"] == "long":
        # Price above threshold relative to SMA
        ratio = current / sma if sma > 0 else 1
        if ratio >= params["threshold"]:
            conf = min(0.45 + (ratio - 1) * 5, 0.85)
            return {"confidence": round(conf, 2), "direction": "long"}
    else:
        ratio = current / sma if sma > 0 else 1
        if ratio <= params["threshold"]:
            conf = min(0.45 + (1 - ratio) * 5, 0.85)
            return {"confidence": round(conf, 2), "direction": "short"}
    return None


# Strategy → detector mapping
_DETECTORS = {
    "breakout": _detect_breakout,
    "bullish_pullback": _detect_pullback,
    "the_5_day_bounce": _detect_pullback,
    "bearish_engulfing": _detect_engulfing,
    "ascending_triangle": _detect_breakout,
    "avwap_bounce": _detect_pullback,
    "snap_back_long": _detect_pullback,
    "double_bottom": _detect_pullback,
}


def run_strategies(symbol: str, bars: list, region: str = "us") -> list:
    """
    Run all free strategies against OHLCV bars for a symbol.
    Returns list of signal dicts.
    """
    if not bars or len(bars) < 5:
        return []

    signals = []
    now = datetime.utcnow().isoformat()

    for strat_name, params in FREE_STRATEGIES.items():
        detector = _DETECTORS.get(strat_name, _detect_generic)
        result = detector(bars, params)

        if result and result["confidence"] >= 0.45:
            signals.append({
                "symbol": symbol,
                "strategy": strat_name,
                "direction": result["direction"],
                "side": result["direction"].upper(),
                "confidence": result["confidence"],
                "price": bars[-1]["close"],
                "region": region,
                "generated_at": now,
                "source": "standalone",
            })

    return signals


def run_batch(symbol_data: dict, region: str = "us") -> list:
    """
    Run strategies against multiple symbols.
    symbol_data: {symbol: [bars]}
    Returns all signals.
    """
    all_signals = []
    for symbol, bars in symbol_data.items():
        sigs = run_strategies(symbol, bars, region)
        all_signals.extend(sigs)

    # Sort by confidence descending
    all_signals.sort(key=lambda s: s["confidence"], reverse=True)
    return all_signals
