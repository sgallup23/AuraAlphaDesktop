"""
Job Router — Dispatches grid jobs to the right executor by job_type.
=====================================================================
Each job type has its own execution path. The worker calls
    route_job(job_dict, cache_dir) → result_dict
and gets back a standardized result.

Supported job types:
  - research_backtest: Strategy candidate backtest (primary)
  - signal_gen: Generate trading signals for a symbol chunk
  - ml_train: Train ML model for one strategy
  - walk_forward: Walk-forward OOS validation
  - optimization: Parameter optimization sweep
  - alpha_factory: Generate + backtest strategy candidates
  - ohlcv_refresh: Update OHLCV cache (requires IBKR, skip if unavailable)
"""
from __future__ import annotations

import json
import logging
import subprocess
import sys
import time
import traceback
from pathlib import Path
from typing import Any, Dict, Optional

log = logging.getLogger("standalone.job_router")

# Project base — workers running from prodesk
BASE = None
for candidate in [
    Path.home() / "TRADING_DESK" / "prodesk",
    Path.home() / "prodesk",
    Path(__file__).resolve().parent.parent.parent,
]:
    if (candidate / "phase2").exists():
        BASE = candidate
        break


def route_job(job_dict: Dict[str, Any], cache_dir: Path) -> Dict[str, Any]:
    """Route a job to the correct executor based on job_type.

    Args:
        job_dict: Full job dict from dequeue (with job_id, job_type, payload, etc.)
        cache_dir: Local cache directory for OHLCV data

    Returns:
        {"job_id": str, "status": "completed"|"failed", "metrics"|"error": ...}
    """
    job_type = job_dict.get("job_type", "research_backtest")
    job_id = job_dict.get("job_id", "unknown")
    payload = job_dict.get("payload", {})

    # If payload is a string (JSON), parse it
    if isinstance(payload, str):
        try:
            payload = json.loads(payload)
        except (json.JSONDecodeError, TypeError):
            payload = {}

    t0 = time.time()

    try:
        if job_type == "research_backtest":
            result = _run_research_backtest(job_dict, cache_dir)
        elif job_type == "signal_gen":
            result = _run_signal_gen(payload, cache_dir)
        elif job_type == "ml_train":
            result = _run_ml_train(payload)
        elif job_type == "walk_forward":
            result = _run_walk_forward(payload, cache_dir)
        elif job_type == "optimization":
            result = _run_optimization(payload, cache_dir)
        elif job_type == "alpha_factory":
            result = _run_alpha_factory(payload, cache_dir)
        elif job_type == "ohlcv_refresh":
            result = _run_ohlcv_refresh(payload)
        else:
            result = {
                "status": "failed",
                "error": f"Unknown job type: {job_type}",
            }

        result["job_id"] = job_id
        result["execution_time"] = round(time.time() - t0, 2)
        return result

    except Exception as e:
        return {
            "job_id": job_id,
            "status": "failed",
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
            "execution_time": round(time.time() - t0, 2),
        }


# ══════════════════════════════════════════════════════════════════════
# EXECUTORS
# ══════════════════════════════════════════════════════════════════════

def _run_research_backtest(job_dict: Dict, cache_dir: Path) -> Dict:
    """Run a research backtest using the standalone backtest engine."""
    from .backtest_engine import run_single_research_job
    return run_single_research_job(job_dict, cache_dir)


def _run_signal_gen(payload: Dict, cache_dir: Path) -> Dict:
    """Generate signals for a chunk of symbols.

    Payload: {symbols: [{symbol, region}], strategies: "all"|[list], mode: str}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    sys.path.insert(0, str(BASE))

    symbols = payload.get("symbols", [])
    mode = payload.get("mode", "scan")
    strategies = payload.get("strategies", "all")

    try:
        from data.athena_backtest_v3 import load_strategy_catalogue
        from scripts.generate_athena_signals import scan_symbol

        catalogue = load_strategy_catalogue()
        if strategies != "all":
            catalogue = {k: v for k, v in catalogue.items() if k in strategies}

        total_signals = 0
        symbols_scanned = 0

        for item in symbols:
            sym = item.get("symbol", item) if isinstance(item, dict) else item
            region = item.get("region", "us") if isinstance(item, dict) else "us"

            try:
                signals = scan_symbol(sym, catalogue, mode=mode)
                total_signals += len(signals) if signals else 0
                symbols_scanned += 1
            except Exception as e:
                log.debug("Signal gen failed for %s: %s", sym, e)

        return {
            "status": "completed",
            "metrics": {
                "symbols_scanned": symbols_scanned,
                "total_signals": total_signals,
                "mode": mode,
            },
        }
    except ImportError as e:
        # Run as subprocess fallback
        return _run_subprocess(
            "signal_gen",
            [sys.executable, str(BASE / "scripts" / "generate_athena_signals.py"),
             "--mode", mode, "--max-symbols", str(len(symbols))],
        )


def _run_ml_train(payload: Dict) -> Dict:
    """Train ML model for one strategy.

    Payload: {strategy: str, trials: int}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    strategy = payload.get("strategy", "")
    trials = payload.get("trials", 30)

    return _run_subprocess(
        "ml_train",
        [sys.executable, str(BASE / "scripts" / "train_loop.py"),
         "--strategy", strategy, "--trials", str(trials)],
        timeout=600,
    )


def _run_walk_forward(payload: Dict, cache_dir: Path) -> Dict:
    """Run walk-forward validation for a candidate.

    Payload: {candidate_id: str}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    candidate_id = payload.get("candidate_id", "")
    sys.path.insert(0, str(BASE))

    try:
        from research_engine.walk_forward_engine import run_walk_forward
        from research_engine import research_database as db

        db.init_db()

        # Look up candidate spec from DB
        import sqlite3
        conn = sqlite3.connect(str(BASE / "research_db" / "research.db"))
        conn.row_factory = sqlite3.Row
        row = conn.execute(
            "SELECT traits, family, direction FROM candidates WHERE candidate_id = ?",
            (candidate_id,),
        ).fetchone()
        conn.close()

        if not row:
            return {"status": "failed", "error": f"Candidate {candidate_id} not found in DB"}

        spec = json.loads(row["traits"]) if row["traits"] else {}
        spec["family"] = row["family"]
        spec["direction"] = row["direction"]

        wf_result = run_walk_forward(spec)

        return {
            "status": "completed",
            "metrics": {
                "candidate_id": candidate_id,
                "passed": wf_result.get("passed", False),
                "walk_forward_score": wf_result.get("walk_forward_score", 0),
                "out_of_sample_score": wf_result.get("out_of_sample_score", 0),
                "degradation_ratio": wf_result.get("degradation_ratio", 1.0),
            },
        }
    except Exception as e:
        return {"status": "failed", "error": f"Walk-forward failed: {e}"}


def _run_optimization(payload: Dict, cache_dir: Path) -> Dict:
    """Run parameter optimization for a strategy.

    Payload: {strategy: str, region: str}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    strategy = payload.get("strategy", "")
    region = payload.get("region", "us")

    return _run_subprocess(
        "optimization",
        [sys.executable, str(BASE / "data" / "athena_optimizer.py"),
         "--strategy", strategy, "--region", region],
        timeout=900,
    )


def _run_alpha_factory(payload: Dict, cache_dir: Path) -> Dict:
    """Run alpha factory for one genome family.

    Payload: {family: str, count: int}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    family = payload.get("family", "")
    count = payload.get("count", 50)

    return _run_subprocess(
        "alpha_factory",
        [sys.executable, str(BASE / "scripts" / "alpha_factory_nightly.py"),
         "--families", family, "--count-per-family", str(count), "--workers", "2"],
        timeout=1200,
    )


def _run_ohlcv_refresh(payload: Dict) -> Dict:
    """Refresh OHLCV cache for a chunk of symbols.

    Only works on machines with IBKR Gateway access.
    Payload: {region: str, symbols: [str]}
    """
    if not BASE:
        return {"status": "failed", "error": "Project base not found"}

    region = payload.get("region", "us")
    symbols = payload.get("symbols", [])

    # Check if IBKR is available
    import socket
    ibkr_host = "127.0.0.1"
    ibkr_port = 4001
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(2)
        s.connect((ibkr_host, ibkr_port))
        s.close()
    except (socket.error, OSError):
        return {
            "status": "completed",
            "metrics": {
                "skipped": True,
                "reason": "IBKR Gateway not available on this machine",
                "region": region,
            },
        }

    return _run_subprocess(
        "ohlcv_refresh",
        [sys.executable, str(BASE / "cache_ibkr_ohlcv_v3.py"),
         "--mode", "cron", "--region", region],
        timeout=600,
    )


# ══════════════════════════════════════════════════════════════════════
# SUBPROCESS HELPER
# ══════════════════════════════════════════════════════════════════════

def _run_subprocess(
    job_type: str,
    cmd: list,
    timeout: int = 300,
) -> Dict:
    """Run a command as a subprocess and capture result."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=str(BASE) if BASE else None,
        )

        if result.returncode == 0:
            # Try to parse structured output
            metrics = {"exit_code": 0}
            # Check for JSON output in last lines
            for line in reversed(result.stdout.strip().split("\n")[-5:]):
                try:
                    parsed = json.loads(line)
                    if isinstance(parsed, dict):
                        metrics.update(parsed)
                        break
                except (json.JSONDecodeError, TypeError):
                    continue

            return {"status": "completed", "metrics": metrics}
        else:
            return {
                "status": "failed",
                "error": f"{job_type} exited with code {result.returncode}: {result.stderr[-500:]}",
            }

    except subprocess.TimeoutExpired:
        return {"status": "failed", "error": f"{job_type} timed out after {timeout}s"}
    except FileNotFoundError:
        return {"status": "failed", "error": f"Script not found for {job_type}"}
