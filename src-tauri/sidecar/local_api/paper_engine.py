"""
Paper Trading Engine — Fully local simulated trading
======================================================
Consumes signals, simulates fills, tracks positions/P&L.
All state persisted in local SQLite. No server required.
"""
import logging
import sqlite3
import time
import uuid
from datetime import datetime, date
from pathlib import Path
from typing import Optional

log = logging.getLogger("paper-engine")
DB_PATH = Path.home() / ".aura-worker" / "local.db"

# Paper trading config
DEFAULT_CAPITAL = 100_000.0
MAX_POSITIONS = 10
POSITION_SIZE_PCT = 0.05  # 5% of equity per position
MIN_CONFIDENCE = 0.50


def _get_db():
    conn = sqlite3.connect(str(DB_PATH), timeout=10)
    conn.row_factory = sqlite3.Row
    return conn


def _ensure_tables():
    conn = _get_db()
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS paper_account (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            starting_capital REAL NOT NULL,
            cash REAL NOT NULL,
            equity REAL NOT NULL,
            day_pnl REAL DEFAULT 0,
            total_pnl REAL DEFAULT 0,
            total_trades INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
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
            pnl REAL DEFAULT 0,
            ts TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS equity_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            equity REAL NOT NULL,
            cash REAL NOT NULL,
            positions_value REAL DEFAULT 0,
            day_pnl REAL DEFAULT 0,
            ts TEXT NOT NULL
        );
    """)
    # Initialize account if not exists
    row = conn.execute("SELECT * FROM paper_account WHERE id = 1").fetchone()
    if not row:
        now = datetime.utcnow().isoformat()
        conn.execute("""
            INSERT INTO paper_account (id, starting_capital, cash, equity, created_at, updated_at)
            VALUES (1, ?, ?, ?, ?, ?)
        """, (DEFAULT_CAPITAL, DEFAULT_CAPITAL, DEFAULT_CAPITAL, now, now))
        conn.commit()
    conn.close()

_ensure_tables()


def get_account() -> dict:
    conn = _get_db()
    row = conn.execute("SELECT * FROM paper_account WHERE id = 1").fetchone()
    conn.close()
    return dict(row) if row else {}


def get_positions(status: str = "open") -> list:
    conn = _get_db()
    rows = conn.execute("SELECT * FROM paper_positions WHERE status = ?", (status,)).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def get_trades(limit: int = 50) -> list:
    conn = _get_db()
    rows = conn.execute("SELECT * FROM paper_trades ORDER BY ts DESC LIMIT ?", (limit,)).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def get_equity_curve(limit: int = 200) -> list:
    conn = _get_db()
    rows = conn.execute("SELECT * FROM equity_snapshots ORDER BY ts DESC LIMIT ?", (limit,)).fetchall()
    conn.close()
    return [dict(r) for r in reversed(rows)]


def process_signals(signals: list) -> dict:
    """
    Process new signals: open positions for high-confidence signals.
    Returns summary of actions taken.
    """
    conn = _get_db()
    acct = dict(conn.execute("SELECT * FROM paper_account WHERE id = 1").fetchone())
    open_positions = conn.execute("SELECT COUNT(*) as cnt FROM paper_positions WHERE status = 'open'").fetchone()["cnt"]
    existing_symbols = {r["symbol"] for r in conn.execute("SELECT symbol FROM paper_positions WHERE status = 'open'").fetchall()}

    opened = 0
    skipped = 0
    now = datetime.utcnow().isoformat()

    for sig in signals:
        if sig.get("confidence", 0) < MIN_CONFIDENCE:
            skipped += 1
            continue
        if open_positions + opened >= MAX_POSITIONS:
            break
        symbol = sig["symbol"]
        if symbol in existing_symbols:
            skipped += 1
            continue

        price = sig.get("price", 0)
        if price <= 0:
            continue

        # Calculate position size
        position_value = acct["equity"] * POSITION_SIZE_PCT
        qty = int(position_value / price)
        if qty <= 0:
            continue
        cost = qty * price
        if cost > acct["cash"]:
            continue

        # Open position
        conn.execute("""
            INSERT INTO paper_positions (symbol, side, qty, entry_price, entry_date, strategy, current_price, status)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'open')
        """, (symbol, sig.get("direction", "long"), qty, price, now, sig.get("strategy", ""), price))

        # Record trade
        conn.execute("""
            INSERT INTO paper_trades (symbol, side, qty, price, strategy, ts)
            VALUES (?, ?, ?, ?, ?, ?)
        """, (symbol, "BUY", qty, price, sig.get("strategy", ""), now))

        # Update cash
        acct["cash"] -= cost
        acct["total_trades"] = acct.get("total_trades", 0) + 1
        existing_symbols.add(symbol)
        opened += 1

    # Update account
    conn.execute("UPDATE paper_account SET cash = ?, total_trades = ?, updated_at = ? WHERE id = 1",
                 (acct["cash"], acct["total_trades"], now))
    conn.commit()
    conn.close()

    return {"opened": opened, "skipped": skipped, "total_signals": len(signals)}


def update_prices(price_map: dict) -> dict:
    """
    Update current prices for all open positions.
    price_map: {symbol: current_price}
    Returns updated P&L summary.
    """
    conn = _get_db()
    positions = conn.execute("SELECT * FROM paper_positions WHERE status = 'open'").fetchall()
    acct = dict(conn.execute("SELECT * FROM paper_account WHERE id = 1").fetchone())

    total_unrealized = 0
    positions_value = 0

    for pos in positions:
        symbol = pos["symbol"]
        current = price_map.get(symbol, pos["current_price"])
        if current <= 0:
            continue

        qty = pos["qty"]
        entry = pos["entry_price"]
        side = pos["side"]

        if side == "long":
            pnl = (current - entry) * qty
        else:
            pnl = (entry - current) * qty

        positions_value += abs(qty * current)
        total_unrealized += pnl

        conn.execute("UPDATE paper_positions SET current_price = ?, unrealized_pnl = ? WHERE id = ?",
                     (current, round(pnl, 2), pos["id"]))

    # Update account equity
    equity = acct["cash"] + positions_value
    day_pnl = equity - acct.get("starting_capital", DEFAULT_CAPITAL)  # simplified
    total_pnl = equity - acct["starting_capital"]
    now = datetime.utcnow().isoformat()

    conn.execute("""
        UPDATE paper_account SET equity = ?, day_pnl = ?, total_pnl = ?, updated_at = ? WHERE id = 1
    """, (round(equity, 2), round(day_pnl, 2), round(total_pnl, 2), now))

    # Snapshot
    conn.execute("""
        INSERT INTO equity_snapshots (equity, cash, positions_value, day_pnl, ts)
        VALUES (?, ?, ?, ?, ?)
    """, (round(equity, 2), round(acct["cash"], 2), round(positions_value, 2), round(day_pnl, 2), now))

    conn.commit()
    conn.close()

    return {
        "equity": round(equity, 2),
        "cash": round(acct["cash"], 2),
        "positions_value": round(positions_value, 2),
        "unrealized_pnl": round(total_unrealized, 2),
        "total_pnl": round(total_pnl, 2),
        "open_positions": len(positions),
    }


def close_position(position_id: int, exit_price: float) -> dict:
    """Close a specific position."""
    conn = _get_db()
    pos = conn.execute("SELECT * FROM paper_positions WHERE id = ? AND status = 'open'", (position_id,)).fetchone()
    if not pos:
        conn.close()
        return {"error": "Position not found"}

    pos = dict(pos)
    qty = pos["qty"]
    entry = pos["entry_price"]
    side = pos["side"]

    if side == "long":
        pnl = (exit_price - entry) * qty
    else:
        pnl = (entry - exit_price) * qty

    now = datetime.utcnow().isoformat()
    proceeds = qty * exit_price

    conn.execute("""
        UPDATE paper_positions SET status = 'closed', exit_price = ?, exit_date = ?, realized_pnl = ? WHERE id = ?
    """, (exit_price, now, round(pnl, 2), position_id))

    conn.execute("""
        INSERT INTO paper_trades (symbol, side, qty, price, strategy, pnl, ts)
        VALUES (?, ?, ?, ?, ?, ?, ?)
    """, (pos["symbol"], "SELL" if side == "long" else "BUY_COVER", qty, exit_price, pos["strategy"], round(pnl, 2), now))

    # Return cash
    conn.execute("UPDATE paper_account SET cash = cash + ?, total_trades = total_trades + 1, updated_at = ? WHERE id = 1",
                 (proceeds, now))

    conn.commit()
    conn.close()

    return {"closed": True, "symbol": pos["symbol"], "pnl": round(pnl, 2)}


def reset_account():
    """Reset paper trading account to starting state."""
    conn = _get_db()
    now = datetime.utcnow().isoformat()
    conn.execute("DELETE FROM paper_positions")
    conn.execute("DELETE FROM paper_trades")
    conn.execute("DELETE FROM equity_snapshots")
    conn.execute("""
        UPDATE paper_account SET cash = ?, equity = ?, day_pnl = 0, total_pnl = 0, total_trades = 0, updated_at = ? WHERE id = 1
    """, (DEFAULT_CAPITAL, DEFAULT_CAPITAL, now))
    conn.commit()
    conn.close()
    return {"reset": True, "capital": DEFAULT_CAPITAL}
