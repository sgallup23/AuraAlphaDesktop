#!/usr/bin/env python3
"""
Aura Alpha — Embedded Local API Server (Standalone Mode)
=========================================================
Lightweight FastAPI server that runs inside the Tauri desktop app.
Provides offline-first API endpoints for paper trading, backtesting,
and signal generation without requiring connection to auraalpha.cc.

Starts automatically with the desktop app on localhost:8020.
"""
import asyncio
import json
import logging
import os
import sqlite3
import sys
import time
from datetime import datetime, date
from pathlib import Path
from typing import Optional

import uvicorn
from fastapi import FastAPI, Query
from fastapi.middleware.cors import CORSMiddleware

# ── Paths ─────────────────────────────────────────────────────────────
APP_DIR = Path(__file__).resolve().parent
RESOURCES = APP_DIR.parent.parent / "resources"
DATA_DIR = Path.home() / ".aura-worker" / "data"
DB_PATH = Path.home() / ".aura-worker" / "local.db"
STATE_DIR = Path.home() / ".aura-worker" / "state"

for d in [DATA_DIR, STATE_DIR]:
    d.mkdir(parents=True, exist_ok=True)

logging.basicConfig(level=logging.INFO, format="%(asctime)s [LOCAL-API] %(message)s")
log = logging.getLogger("local-api")

# ── Free tier strategy limit ──────────────────────────────────────────
FREE_STRATEGIES = [
    "breakout", "bullish_pullback", "the_5_day_bounce", "bearish_engulfing",
    "ascending_triangle", "avwap_bounce", "bear_flag", "nice_chart",
    "double_bottom", "trend_play", "gap_and_go", "snap_back_long",
    "slippery_slope", "topping_formation", "failed_bounce",
]
FREE_MARKET = "us"
SIGNAL_DELAY_MINUTES = 10  # slight delay on offline signals

# ── Database ──────────────────────────────────────────────────────────
def _get_db():
    conn = sqlite3.connect(str(DB_PATH), timeout=10)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    return conn

def _init_db():
    conn = _get_db()
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS paper_positions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL,
            side TEXT NOT NULL,
            qty REAL NOT NULL,
            entry_price REAL NOT NULL,
            entry_date TEXT NOT NULL,
            strategy TEXT,
            current_price REAL DEFAULT 0,
            unrealized_pnl REAL DEFAULT 0,
            status TEXT DEFAULT 'open',
            exit_price REAL,
            exit_date TEXT,
            realized_pnl REAL
        );
        CREATE TABLE IF NOT EXISTS paper_trades (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL,
            side TEXT NOT NULL,
            qty REAL NOT NULL,
            price REAL NOT NULL,
            strategy TEXT,
            ts TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS equity_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            equity REAL NOT NULL,
            cash REAL NOT NULL,
            positions_value REAL DEFAULT 0,
            ts TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS cached_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL,
            strategy TEXT NOT NULL,
            direction TEXT NOT NULL,
            confidence REAL NOT NULL,
            region TEXT DEFAULT 'us',
            generated_at TEXT NOT NULL,
            price REAL DEFAULT 0
        );
    """)
    conn.commit()
    conn.close()

_init_db()

# ── Load bundled resources ────────────────────────────────────────────
def _load_strategies():
    f = RESOURCES / "strategies" / "strategies.json"
    if f.exists():
        return json.loads(f.read_text())
    return []

def _load_sweep_best(market="us"):
    f = RESOURCES / "sweep_best" / f"{market}.json"
    if f.exists():
        return json.loads(f.read_text())
    return {}

STRATEGIES = _load_strategies()

# ── Paper trading state ───────────────────────────────────────────────
PAPER_STATE = {
    "starting_capital": 100_000.0,
    "cash": 100_000.0,
    "equity": 100_000.0,
    "positions_count": 0,
    "trades_today": 0,
    "day_pnl": 0.0,
}

# ── FastAPI App ───────────────────────────────────────────────────────
app = FastAPI(title="Aura Alpha Local API", version="1.0.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# ── Health ────────────────────────────────────────────────────────────
@app.get("/api/health")
async def health():
    return {
        "status": "ok",
        "mode": "standalone",
        "version": "1.0.0",
        "port": 8020,
        "timestamp": datetime.utcnow().isoformat(),
    }

@app.get("/api/health/score")
async def health_score():
    return {
        "score": 100,
        "status": "healthy",
        "mode": "standalone",
        "components": {
            "local_api": {"score": 100, "weight": 0.5, "detail": "Running"},
            "paper_engine": {"score": 100, "weight": 0.3, "detail": "Active"},
            "data_cache": {"score": 100, "weight": 0.2, "detail": "Ready"},
        },
    }

# ── Strategies ────────────────────────────────────────────────────────
@app.get("/api/strategies")
async def list_strategies():
    free = [s for s in STRATEGIES if s.get("key", s.get("name", "")) in FREE_STRATEGIES]
    locked_count = len(STRATEGIES) - len(free)
    return {
        "strategies": free,
        "total": len(STRATEGIES),
        "available": len(free),
        "locked": locked_count,
        "upgrade_message": f"Unlock {locked_count} more strategies with Aura Alpha Pro" if locked_count > 0 else None,
    }

# ── OHLCV Charts ──────────────────────────────────────────────────────
@app.get("/api/charts/ohlcv/{symbol}")
async def get_ohlcv(symbol: str, bars: int = Query(500), region: str = Query("us")):
    # Check local cache first
    parquet_path = DATA_DIR / region / f"{symbol}.parquet"
    if parquet_path.exists():
        try:
            import polars as pl
            df = pl.read_parquet(str(parquet_path))
            data = df.tail(bars).to_dicts()
            return {"bars": data, "symbol": symbol, "source": "cache"}
        except Exception:
            pass

    # Fetch from yfinance
    try:
        import yfinance as yf
        ticker = yf.Ticker(symbol)
        hist = ticker.history(period="2y")
        if hist.empty:
            return {"bars": [], "symbol": symbol, "error": "No data"}

        bars_data = []
        for idx, row in hist.iterrows():
            bars_data.append({
                "date": idx.strftime("%Y-%m-%d"),
                "open": round(row["Open"], 4),
                "high": round(row["High"], 4),
                "low": round(row["Low"], 4),
                "close": round(row["Close"], 4),
                "volume": int(row["Volume"]),
            })

        # Cache as parquet for next time
        try:
            import polars as pl
            parquet_path.parent.mkdir(parents=True, exist_ok=True)
            pl.DataFrame(bars_data).write_parquet(str(parquet_path))
        except Exception:
            pass

        return {"bars": bars_data[-bars:], "symbol": symbol, "source": "yfinance"}
    except Exception as e:
        return {"bars": [], "symbol": symbol, "error": str(e)}

# ── Positions (Paper Trading) ─────────────────────────────────────────
@app.get("/api/positions")
async def get_positions():
    conn = _get_db()
    rows = conn.execute("SELECT * FROM paper_positions WHERE status = 'open'").fetchall()
    conn.close()
    positions = [dict(r) for r in rows]
    return {"positions": positions, "count": len(positions)}

@app.get("/api/positions/orders")
async def get_orders():
    return {"orders": []}

# ── P&L Snapshot ──────────────────────────────────────────────────────
@app.get("/api/pnl/snapshot")
async def pnl_snapshot():
    return {
        "timestamp": time.time(),
        "bots": {
            "paper": {
                "equity": PAPER_STATE["equity"],
                "day_pnl": PAPER_STATE["day_pnl"],
                "week_pnl": 0,
                "month_pnl": 0,
                "unrealized_pnl": 0,
                "positions": PAPER_STATE["positions_count"],
                "total_trades": PAPER_STATE["trades_today"],
                "status": "OK",
                "cash_available": [{"note": "Paper Trading Mode"}],
            }
        },
        "total_pnl": PAPER_STATE["day_pnl"],
        "total_equity": PAPER_STATE["equity"],
    }

# ── Telemetry ─────────────────────────────────────────────────────────
@app.get("/api/telemetry/latest")
async def telemetry():
    return {
        "bots": {
            "paper": {
                "status": "OK",
                "last_heartbeat": datetime.utcnow().isoformat(),
                "payload": {
                    "mode": "PAPER",
                    "equity": {
                        "current": PAPER_STATE["equity"],
                        "starting": PAPER_STATE["starting_capital"],
                        "day_pnl": PAPER_STATE["day_pnl"],
                    },
                    "positions": [],
                },
            }
        }
    }

# ── Control Health ────────────────────────────────────────────────────
@app.get("/api/control/health")
async def control_health():
    return {
        "can_trade": True,
        "reason": "Paper trading mode — standalone",
        "bots": {"paper": {"status": "OK"}},
        "per_bot_health": {"paper": True},
        "kill": {"enabled": False},
    }

# ── System Health ─────────────────────────────────────────────────────
@app.get("/api/system/health")
async def system_health():
    return {
        "api": "ok",
        "gateway": {"connected": False, "note": "Standalone mode — no broker connected"},
        "bots": {},
    }

# ── Signals (with delay for free tier) ────────────────────────────────
@app.get("/api/signals/latest")
@app.get("/api/scanners/signals/live")
async def latest_signals(promoted_only: bool = False):
    conn = _get_db()
    rows = conn.execute(
        "SELECT * FROM cached_signals ORDER BY confidence DESC LIMIT 50"
    ).fetchall()
    conn.close()
    signals = [dict(r) for r in rows]
    return {"signals": signals, "count": len(signals), "mode": "standalone"}

# ── Scanners ──────────────────────────────────────────────────────────
@app.get("/api/scanners")
async def scanners():
    return [
        {"key": "momentum", "name": "Momentum Scanner", "description": "High momentum stocks"},
        {"key": "reversals", "name": "Reversal Scanner", "description": "Mean reversion setups"},
    ]

@app.get("/api/scanners/{key}")
async def scanner_results(key: str):
    return {"scanner": key, "results": [], "note": "Connect to cloud for live scanner data"}

# ── Backtests ─────────────────────────────────────────────────────────
@app.get("/api/backtest/results")
@app.get("/api/backtest/strategies")
async def backtest_results(region: str = Query("us")):
    sweep = _load_sweep_best(region)
    results = []
    for strat, data in sweep.items():
        if strat in FREE_STRATEGIES:
            results.append({
                "strategy": strat,
                "pf": data.get("pf", 0),
                "win_rate": data.get("win_rate", 0),
                "trade_count": data.get("trade_count", 0),
                "avg_return": data.get("avg_return", 0),
            })
    return {"strategies": results, "region": region}

# ── Watchlists ────────────────────────────────────────────────────────
@app.get("/api/watchlists")
async def watchlists():
    return [
        {"id": 1, "name": "Default", "symbols": ["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA", "NVDA", "META", "AMD", "NFLX", "SPY"]},
    ]

# ── Alerts ────────────────────────────────────────────────────────────
@app.get("/api/alerts")
async def alerts():
    return []

@app.get("/api/alerts/triggered")
async def triggered_alerts():
    return []

# ── Intelligence ──────────────────────────────────────────────────────
@app.get("/api/intelligence/status")
async def intelligence_status():
    return {
        "jobs_by_status": {"completed": 0, "queued": 0},
        "workers_by_status": {},
        "mode": "standalone",
        "upgrade_message": "Connect to cloud to access distributed research grid (128,000+ jobs completed)",
    }

# ── Layla Performance ─────────────────────────────────────────────────
@app.get("/api/layla/performance")
async def layla_performance():
    return {
        "status": "showcase",
        "initial_capital": 10000,
        "current_equity": 13448.10,
        "total_pnl": 3448.10,
        "total_pnl_pct": 34.48,
        "total_trades": 477,
        "win_rate": 60.4,
        "strategies_active": 19,
        "regions_active": 3,
        "strategy_breakdown": [
            {"strategy": "bullish_pullback", "pnl": 240.53, "trades": 88, "win_rate": 69.3},
            {"strategy": "bearish_engulfing", "pnl": 117.43, "trades": 73, "win_rate": 60.3},
            {"strategy": "the_5_day_bounce", "pnl": 108.25, "trades": 53, "win_rate": 58.5},
            {"strategy": "ascending_triangle", "pnl": 81.18, "trades": 46, "win_rate": 60.9},
            {"strategy": "double_bottom", "pnl": 75.36, "trades": 18, "win_rate": 83.3},
        ],
        "equity_curve": [],
    }

# ── Upgrade triggers ──────────────────────────────────────────────────
@app.get("/api/upgrade/check")
async def upgrade_check(feature: str = ""):
    locked_features = {
        "strategies": "Unlock all 83 strategies across 19 global markets",
        "live_trading": "Connect your broker for live automated execution",
        "global_markets": "Trade Europe, Asia, Forex, Futures — 19 markets",
        "grid_compute": "Contribute compute power and earn research credits",
        "ml_training": "Custom ML model training on your strategy data",
        "optimizer": "Per-market strategy optimization with walk-forward validation",
    }
    msg = locked_features.get(feature, "Upgrade to Aura Alpha Pro for full system access")
    return {
        "locked": True,
        "feature": feature,
        "message": msg,
        "upgrade_url": "https://auraalpha.cc/register",
        "plans": [
            {"name": "Trader", "price": 49, "features": ["2 live bots", "US markets", "All strategies"]},
            {"name": "Pro", "price": 149, "features": ["3 live bots", "All 19 markets", "ML + Grid"]},
        ],
    }

# ── Catch-all for unimplemented endpoints ─────────────────────────────
@app.get("/api/{path:path}")
async def catch_all(path: str):
    return {
        "mode": "standalone",
        "endpoint": f"/api/{path}",
        "message": "Connect to Aura Alpha cloud for full functionality",
        "upgrade_url": "https://auraalpha.cc/register",
    }

# ── Signal generation endpoint ────────────────────────────────────────
@app.post("/api/signals/generate")
@app.get("/api/signals/generate")
async def generate_signals(symbols: str = Query("SPY,QQQ,AAPL,TSLA,NVDA")):
    """Generate signals for given symbols using local data + strategy engine."""
    from .data_service import get_ohlcv
    from .strategy_runner import run_batch
    from .signal_store import store_signals

    sym_list = [s.strip() for s in symbols.split(",") if s.strip()]
    symbol_data = {}
    for sym in sym_list[:20]:  # cap at 20
        result = await get_ohlcv(sym, bars=100)
        if result.get("bars"):
            symbol_data[sym] = result["bars"]

    signals = run_batch(symbol_data, region="us")
    stored = store_signals(signals)
    return {"signals": signals, "generated": len(signals), "stored": stored}

# ── Paper trading endpoints ──────────────────────────────────────────
@app.get("/api/paper/positions")
async def paper_positions():
    from .paper_engine import get_positions
    return {"positions": get_positions()}

@app.get("/api/paper/trades")
async def paper_trades(limit: int = Query(50)):
    from .paper_engine import get_trades
    return {"trades": get_trades(limit)}

@app.get("/api/paper/equity")
async def paper_equity():
    from .paper_engine import get_account, get_equity_curve
    return {"account": get_account(), "curve": get_equity_curve()}

@app.post("/api/paper/execute")
async def paper_execute():
    """Execute latest signals through paper trading engine."""
    from .signal_store import get_latest_signals
    from .paper_engine import process_signals
    signals = get_latest_signals(limit=20, min_confidence=0.50)
    result = process_signals(signals)
    return result

@app.post("/api/paper/close/{position_id}")
async def paper_close(position_id: int, price: float = Query(0)):
    from .paper_engine import close_position
    return close_position(position_id, price)

@app.post("/api/paper/reset")
async def paper_reset():
    from .paper_engine import reset_account
    return reset_account()

@app.get("/api/paper/summary")
async def paper_summary():
    from .paper_engine import get_account, get_positions
    acct = get_account()
    positions = get_positions()
    return {
        "equity": acct.get("equity", DEFAULT_CAPITAL),
        "cash": acct.get("cash", DEFAULT_CAPITAL),
        "total_pnl": acct.get("total_pnl", 0),
        "day_pnl": acct.get("day_pnl", 0),
        "total_trades": acct.get("total_trades", 0),
        "open_positions": len(positions),
        "starting_capital": acct.get("starting_capital", DEFAULT_CAPITAL),
    }

# ── Market status ────────────────────────────────────────────────────
@app.get("/api/market/status")
async def market_status():
    from .data_service import is_connected
    from .cache_manager import cache_stats
    return {
        "mode": "connected" if is_connected() else "standalone",
        "cache": cache_stats(),
    }

# ── Preload / first-run ──────────────────────────────────────────────
@app.post("/api/system/preload")
@app.get("/api/system/preload")
async def trigger_preload():
    from .data_service import preload
    return await preload()

# ── Background refresh loop ──────────────────────────────────────────
_refresh_running = False

async def _refresh_loop():
    """Background loop: refresh data, generate signals, update paper engine."""
    global _refresh_running
    _refresh_running = True
    from .data_service import get_ohlcv, preload
    from .strategy_runner import run_batch
    from .signal_store import store_signals, clear_old_signals
    from .paper_engine import update_prices

    # First-run preload
    log.info("First-run preload starting...")
    await preload()
    log.info("Preload complete — generating initial signals...")

    # Generate initial signals
    watchlist = ["SPY", "QQQ", "AAPL", "TSLA", "NVDA", "MSFT", "AMD", "META", "AMZN", "GOOGL"]
    symbol_data = {}
    for sym in watchlist:
        result = await get_ohlcv(sym, bars=100)
        if result.get("bars"):
            symbol_data[sym] = result["bars"]

    if symbol_data:
        signals = run_batch(symbol_data)
        store_signals(signals)
        log.info("Initial signals generated: %d", len(signals))

    # Continuous refresh loop
    cycle = 0
    while _refresh_running:
        try:
            await asyncio.sleep(300)  # 5 minutes
            cycle += 1

            # Refresh prices for watchlist
            price_map = {}
            for sym in watchlist:
                result = await get_ohlcv(sym, bars=5, force_refresh=(cycle % 6 == 0))
                bars = result.get("bars", [])
                if bars:
                    price_map[sym] = bars[-1]["close"]

            # Update paper positions with latest prices
            if price_map:
                update_prices(price_map)

            # Re-generate signals every 3 cycles (15 min)
            if cycle % 3 == 0:
                symbol_data = {}
                for sym in watchlist:
                    result = await get_ohlcv(sym, bars=100)
                    if result.get("bars"):
                        symbol_data[sym] = result["bars"]
                if symbol_data:
                    signals = run_batch(symbol_data)
                    store_signals(signals)

            # Clean old signals daily
            if cycle % 288 == 0:  # ~24 hours
                clear_old_signals(hours=48)

        except Exception as e:
            log.warning("Refresh loop error: %s", e)
            await asyncio.sleep(30)


@app.on_event("startup")
async def on_startup():
    """Start background refresh loop on API startup."""
    asyncio.create_task(_refresh_loop())
    log.info("Background refresh loop started")


# ── Main ──────────────────────────────────────────────────────────────
def main():
    log.info("=" * 60)
    log.info("Aura Alpha Local API — Standalone Mode")
    log.info("=" * 60)
    log.info(f"  Data:       {DATA_DIR}")
    log.info(f"  Database:   {DB_PATH}")
    log.info(f"  Resources:  {RESOURCES}")
    log.info(f"  Strategies: {len(FREE_STRATEGIES)} (free tier)")
    log.info(f"  Port:       8020")
    log.info("=" * 60)
    uvicorn.run(app, host="127.0.0.1", port=8020, log_level="warning")

if __name__ == "__main__":
    main()
