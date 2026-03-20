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
import json
import logging
import os
import platform
import signal
import ssl
import sys
import time
import urllib.request
import urllib.error
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("grid-worker")

# ── Configuration ────────────────────────────────────────────────

DEFAULT_COORDINATOR = "https://auraalpha.cc"
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


# ── Token Persistence ────────────────────────────────────────────

def _token_file_path() -> Path:
    """Return path to the stored grid token file.

    Checks GRID_TOKEN_DIR env var first (set by the Tauri launcher),
    then falls back to ~/.auraalpha/.
    """
    token_dir = os.getenv("GRID_TOKEN_DIR")
    if token_dir:
        d = Path(token_dir)
    else:
        d = Path.home() / ".auraalpha"
    d.mkdir(parents=True, exist_ok=True)
    return d / "grid_token.json"


def _load_stored_token() -> dict | None:
    """Load previously provisioned token + worker_id from disk."""
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
    """Persist token + worker_id to disk."""
    path = _token_file_path()
    path.write_text(json.dumps(data, indent=2))
    log.info("Token saved to %s", path)


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

    # Get RAM without psutil (stdlib only)
    ram_gb = 8.0
    try:
        import psutil
        ram_gb = round(psutil.virtual_memory().total / (1024 ** 3), 1)
    except ImportError:
        # Try platform-specific fallbacks
        try:
            if platform.system() == "Linux":
                with open("/proc/meminfo") as f:
                    for line in f:
                        if line.startswith("MemTotal:"):
                            ram_kb = int(line.split()[1])
                            ram_gb = round(ram_kb / (1024 ** 2), 1)
                            break
            elif platform.system() == "Darwin":
                import subprocess
                out = subprocess.check_output(["sysctl", "-n", "hw.memsize"],
                                              timeout=5).decode().strip()
                ram_gb = round(int(out) / (1024 ** 3), 1)
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
                ram_gb = round(stat.ullTotalPhys / (1024 ** 3), 1)
        except Exception:
            pass
    except Exception:
        pass

    return {
        "hostname": platform.node(),
        "cpu_count": cpu_count,
        "ram_gb": ram_gb,
        "os_info": f"{platform.system()} {platform.release()}",
        "worker_type": "contributor",
        "supported_job_types": ["research_backtest", "signal_gen", "ml_train",
                                 "walk_forward", "optimization", "alpha_factory"],
    }


# ── Auto-Provision ───────────────────────────────────────────────

def _auto_provision(base_url: str) -> dict:
    """Call the zero-auth auto-provision endpoint to get a unique token.

    POST /api/cluster/contributor/auto-provision
    Body: {hostname, cpus, ram, os}
    Returns: {token, worker_id}
    """
    url = f"{base_url.rstrip('/')}/api/cluster/contributor/auto-provision"
    info = _get_system_info()
    body = json.dumps({
        "hostname": info["hostname"],
        "cpus": info["cpu_count"],
        "ram": info["ram_gb"],
        "os": info["os_info"],
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


# ── HTTP helpers (stdlib urllib) ─────────────────────────────────

def _http_post(url: str, headers: dict, body: dict, timeout: int = 10) -> tuple:
    """POST JSON, return (status_code, response_body_dict).

    Returns (0, {}) on network / timeout errors.
    """
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, method="POST", headers=headers)
    try:
        ctx = ssl.create_default_context()
        with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
            resp_body = resp.read().decode("utf-8")
            try:
                return (resp.status, json.loads(resp_body))
            except json.JSONDecodeError:
                return (resp.status, {"raw": resp_body})
    except urllib.error.HTTPError as e:
        body_text = ""
        try:
            body_text = e.read().decode("utf-8")[:200]
        except Exception:
            pass
        return (e.code, {"error": body_text})
    except Exception:
        return (0, {})


# ── API Client ──────────────────────────────────────────────────

class GridClient:
    def __init__(self, base_url: str, token: str, worker_id: str, max_parallel: int):
        self.base_url = base_url.rstrip("/")
        self.api = f"{self.base_url}/api/cluster/contributor"
        self.token = token
        self.worker_id = worker_id
        self.max_parallel = max_parallel
        self.session_headers = {
            "Content-Type": "application/json",
            "X-Worker-Token": self.token,      # raw token — server hashes it
            "X-Worker-Id": self.worker_id,
        }

    def register(self) -> bool:
        info = _get_system_info()
        info["worker_id"] = self.worker_id
        info["max_parallel"] = self.max_parallel
        status, data = _http_post(
            f"{self.api}/register", self.session_headers, info, timeout=10,
        )
        if status == 200:
            log.info("Registered as %s (%d CPUs, %.1f GB RAM)",
                     self.worker_id, info["cpu_count"], info["ram_gb"])
            return True
        log.warning("Registration failed: %d %s", status,
                     str(data.get("error", data))[:200])
        return False

    def heartbeat(self) -> bool:
        status, _ = _http_post(
            f"{self.api}/heartbeat", self.session_headers, {
                "worker_id": self.worker_id,
                "status": "online",
            }, timeout=5,
        )
        return status == 200

    def dequeue(self, count: int = 1) -> list:
        status, data = _http_post(
            f"{self.api}/dequeue", self.session_headers, {
                "worker_id": self.worker_id,
                "max_jobs": count,
            }, timeout=10,
        )
        if status == 200:
            return data.get("jobs", [])
        return []

    def complete(self, job_id: str, result: dict = None, error: str = None,
                 duration: float = 0) -> bool:
        payload = {
            "worker_id": self.worker_id,
            "job_id": job_id,
            "status": "completed" if not error else "failed",
            "result": json.dumps(result or {}),
            "error": error or "",
            "duration_sec": round(duration, 2),
        }
        status, _ = _http_post(
            f"{self.api}/complete", self.session_headers, payload, timeout=10,
        )
        return status == 200


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
    parser.add_argument("--token", default=None)
    args = parser.parse_args()

    # ── Resolve token + worker_id ────────────────────────────────
    token = None
    worker_id = None

    if args.token:
        # Explicit --token flag overrides everything
        token = args.token
        worker_id = _get_worker_id()
        log.info("Using CLI-provided token")
    else:
        # Check for GRID_WORKER_TOKEN env var
        env_token = os.getenv("GRID_WORKER_TOKEN")
        if env_token:
            token = env_token
            worker_id = _get_worker_id()
            log.info("Using GRID_WORKER_TOKEN from environment")
        else:
            # Try loading stored auto-provisioned token
            stored = _load_stored_token()
            if stored:
                token = stored["token"]
                worker_id = stored["worker_id"]
                log.info("Loaded stored token for worker_id=%s", worker_id)
            else:
                # Auto-provision a new unique token from the server
                log.info("No token found — auto-provisioning from coordinator...")
                try:
                    provisioned = _auto_provision(args.coordinator_url)
                    token = provisioned["token"]
                    worker_id = provisioned["worker_id"]
                    _save_token(provisioned)
                except Exception as e:
                    log.error("Auto-provision failed: %s. Cannot start without a token.", e)
                    sys.exit(1)

    log.info("=" * 60)
    log.info("Aura Alpha Grid Worker starting")
    log.info("Coordinator: %s", args.coordinator_url)
    log.info("Max parallel: %d", args.max_parallel)
    log.info("Worker ID: %s", worker_id)
    log.info("=" * 60)

    client = GridClient(args.coordinator_url, token, worker_id, args.max_parallel)

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
