"""
Signal Store — Local SQLite storage for generated signals
==========================================================
"""
import json
import sqlite3
import time
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

DB_PATH = Path.home() / ".aura-worker" / "local.db"


def _get_db():
    conn = sqlite3.connect(str(DB_PATH), timeout=10)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    return conn


def store_signals(signals: list) -> int:
    """Store generated signals. Returns count stored."""
    conn = _get_db()
    stored = 0
    for sig in signals:
        try:
            conn.execute("""
                INSERT INTO cached_signals
                (symbol, strategy, direction, confidence, region, generated_at, price)
                VALUES (?, ?, ?, ?, ?, ?, ?)
            """, (
                sig.get("symbol", ""),
                sig.get("strategy", ""),
                sig.get("direction", sig.get("side", "long")),
                sig.get("confidence", 0),
                sig.get("region", "us"),
                sig.get("generated_at", datetime.utcnow().isoformat()),
                sig.get("price", sig.get("entry_price", 0)),
            ))
            stored += 1
        except Exception:
            pass
    conn.commit()
    conn.close()
    return stored


def get_latest_signals(limit: int = 50, strategy: str = None,
                       min_confidence: float = 0) -> list:
    """Get latest cached signals."""
    conn = _get_db()
    query = "SELECT * FROM cached_signals WHERE confidence >= ?"
    params = [min_confidence]
    if strategy:
        query += " AND strategy = ?"
        params.append(strategy)
    query += " ORDER BY confidence DESC LIMIT ?"
    params.append(limit)
    rows = conn.execute(query, params).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def clear_old_signals(hours: int = 24) -> int:
    """Remove signals older than N hours."""
    conn = _get_db()
    cutoff = datetime.utcnow().timestamp() - (hours * 3600)
    cutoff_iso = datetime.utcfromtimestamp(cutoff).isoformat()
    cur = conn.execute("DELETE FROM cached_signals WHERE generated_at < ?", (cutoff_iso,))
    conn.commit()
    removed = cur.rowcount
    conn.close()
    return removed


def signal_count() -> int:
    conn = _get_db()
    row = conn.execute("SELECT COUNT(*) as cnt FROM cached_signals").fetchone()
    conn.close()
    return row["cnt"] if row else 0
