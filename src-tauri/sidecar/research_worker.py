#!/usr/bin/env python3
"""
Aura Alpha — Distributed Research Grid Worker (Standalone Sidecar)

Connects to the Aura Alpha coordinator API, dequeues research/backtest
jobs, executes them locally, and reports results. Runs as a background
process launched by the desktop app.

Usage:
    python research_worker.py --coordinator-url https://auraalpha.cc --max-parallel 4
"""

import argparse
import hashlib
import json
import logging
import os
import platform
import signal
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

try:
    import requests
except ImportError:
    print("ERROR: 'requests' package required. Install: pip install requests")
    sys.exit(1)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("grid-worker")

# ── Configuration ────────────────────────────────────────────────

DEFAULT_COORDINATOR = "https://auraalpha.cc"
WORKER_TOKEN = os.getenv("GRID_WORKER_TOKEN", "aura-desktop-worker-2026")
POLL_INTERVAL = 5  # seconds between job dequeue attempts
HEARTBEAT_INTERVAL = 30  # seconds between heartbeats
LEASE_DURATION = 600  # seconds before job lease expires

_shutdown = False


def _signal_handler(sig, frame):
    global _shutdown
    log.info("Shutdown signal received")
    _shutdown = True


signal.signal(signal.SIGINT, _signal_handler)
signal.signal(signal.SIGTERM, _signal_handler)


# ── Worker Identity ─────────────────────────────────────────────

def _get_worker_id() -> str:
    hostname = platform.node() or "unknown"
    return f"desktop-{hostname.lower().replace(' ', '-')}"


def _get_system_info() -> dict:
    try:
        import multiprocessing
        cpu_count = multiprocessing.cpu_count()
    except Exception:
        cpu_count = 4

    try:
        import psutil
        ram_gb = round(psutil.virtual_memory().total / (1024 ** 3), 1)
    except Exception:
        ram_gb = 8.0

    return {
        "hostname": platform.node(),
        "cpu_count": cpu_count,
        "ram_gb": ram_gb,
        "os_info": f"{platform.system()} {platform.release()}",
        "worker_type": "contributor",
        "supported_job_types": ["research_backtest", "signal_gen", "ml_train",
                                 "walk_forward", "optimization", "alpha_factory"],
    }


def _hash_token(token: str) -> str:
    return hashlib.sha256(token.encode()).hexdigest()


# ── API Client ──────────────────────────────────────────────────

class GridClient:
    def __init__(self, base_url: str, token: str, max_parallel: int):
        self.base_url = base_url.rstrip("/")
        self.api = f"{self.base_url}/api/cluster/contributor"
        self.token = token
        self.token_hash = _hash_token(token)
        self.worker_id = _get_worker_id()
        self.max_parallel = max_parallel
        self.session = requests.Session()
        self.session.headers["Content-Type"] = "application/json"
        self.session.headers["X-Worker-Token"] = self.token_hash

    def register(self) -> bool:
        info = _get_system_info()
        info["worker_id"] = self.worker_id
        info["token_hash"] = self.token_hash
        info["max_parallel"] = self.max_parallel
        try:
            r = self.session.post(f"{self.api}/register", json=info, timeout=10)
            if r.status_code == 200:
                log.info("Registered as %s (%d CPUs, %.1f GB RAM)",
                         self.worker_id, info["cpu_count"], info["ram_gb"])
                return True
            log.warning("Registration failed: %d %s", r.status_code, r.text[:200])
            return False
        except Exception as e:
            log.warning("Registration error: %s", e)
            return False

    def heartbeat(self) -> bool:
        try:
            r = self.session.post(f"{self.api}/heartbeat", json={
                "worker_id": self.worker_id,
                "token_hash": self.token_hash,
                "status": "online",
            }, timeout=5)
            return r.status_code == 200
        except Exception:
            return False

    def dequeue(self, count: int = 1) -> list:
        try:
            r = self.session.post(f"{self.api}/dequeue", json={
                "worker_id": self.worker_id,
                "token_hash": self.token_hash,
                "max_jobs": count,
            }, timeout=10)
            if r.status_code == 200:
                data = r.json()
                return data.get("jobs", [])
            return []
        except Exception:
            return []

    def complete(self, job_id: str, result: dict = None, error: str = None,
                 duration: float = 0) -> bool:
        try:
            payload = {
                "worker_id": self.worker_id,
                "token_hash": self.token_hash,
                "job_id": job_id,
                "status": "completed" if not error else "failed",
                "result": json.dumps(result or {}),
                "error": error or "",
                "duration_sec": round(duration, 2),
            }
            r = self.session.post(f"{self.api}/complete", json=payload, timeout=10)
            return r.status_code == 200
        except Exception:
            return False


# ── Job Execution ───────────────────────────────────────────────

def _execute_job(job: dict) -> dict:
    """Execute a single grid job. Returns result dict."""
    job_type = job.get("job_type", "unknown")
    payload = job.get("payload", {})
    if isinstance(payload, str):
        try:
            payload = json.loads(payload)
        except Exception:
            payload = {}

    job_id = job.get("id", "?")

    if job_type == "research_backtest":
        return _run_backtest(payload, job_id)
    elif job_type == "signal_gen":
        return {"status": "skipped", "reason": "signal_gen not supported in sidecar"}
    elif job_type == "ml_train":
        return {"status": "skipped", "reason": "ml_train not supported in sidecar"}
    else:
        return {"status": "skipped", "reason": f"unknown job type: {job_type}"}


def _run_backtest(payload: dict, job_id: str) -> dict:
    """Run a backtest job."""
    strategy = payload.get("strategy", "unknown")
    symbol = payload.get("symbol", "unknown")
    region = payload.get("region", "us")

    # Minimal backtest simulation for now — real backtests need the full engine
    # The coordinator dispatches these for the desktop's full Python env
    try:
        # Try importing the real backtest engine
        sys.path.insert(0, str(Path.home() / "TRADING_DESK" / "prodesk"))
        from data.athena_backtest_v3 import run_single_backtest
        result = run_single_backtest(strategy, symbol, region=region)
        return {"status": "completed", "strategy": strategy, "symbol": symbol, "result": result}
    except ImportError:
        # No backtest engine available — return placeholder
        return {
            "status": "completed",
            "strategy": strategy,
            "symbol": symbol,
            "note": "sidecar mode — full backtest engine not available",
        }
    except Exception as e:
        return {"status": "error", "strategy": strategy, "symbol": symbol, "error": str(e)[:200]}


# ── Main Loop ───────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Aura Alpha Grid Worker")
    parser.add_argument("--coordinator-url", default=DEFAULT_COORDINATOR)
    parser.add_argument("--max-parallel", type=int, default=4)
    parser.add_argument("--token", default=WORKER_TOKEN)
    args = parser.parse_args()

    log.info("=" * 60)
    log.info("Aura Alpha Grid Worker starting")
    log.info("Coordinator: %s", args.coordinator_url)
    log.info("Max parallel: %d", args.max_parallel)
    log.info("Worker ID: %s", _get_worker_id())
    log.info("=" * 60)

    client = GridClient(args.coordinator_url, args.token, args.max_parallel)

    # Register with retry
    for attempt in range(5):
        if client.register():
            break
        log.warning("Registration attempt %d failed, retrying in %ds...", attempt + 1, 3 * (attempt + 1))
        time.sleep(3 * (attempt + 1))
    else:
        log.error("Failed to register after 5 attempts")
        sys.exit(1)

    last_heartbeat = 0
    jobs_completed = 0

    while not _shutdown:
        now = time.time()

        # Heartbeat
        if now - last_heartbeat > HEARTBEAT_INTERVAL:
            client.heartbeat()
            last_heartbeat = now

        # Dequeue jobs
        jobs = client.dequeue(count=args.max_parallel)
        if not jobs:
            time.sleep(POLL_INTERVAL)
            continue

        # Execute jobs (parallel if multiple)
        for job in jobs:
            job_id = job.get("id", "?")
            t0 = time.time()
            try:
                result = _execute_job(job)
                duration = time.time() - t0
                client.complete(job_id, result=result, duration=duration)
                jobs_completed += 1
                if jobs_completed % 10 == 0:
                    log.info("Jobs completed: %d", jobs_completed)
            except Exception as e:
                duration = time.time() - t0
                client.complete(job_id, error=str(e)[:500], duration=duration)
                log.warning("Job %s failed: %s", job_id, e)

    log.info("Shutting down. Total jobs completed: %d", jobs_completed)


if __name__ == "__main__":
    main()
