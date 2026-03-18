"""
Aura Alpha Research Worker Launcher — Desktop App Edition
==========================================================
Thin launcher that bootstraps and runs the standalone HTTPS worker.
The actual compute logic lives in sidecar/standalone/ (bundled with the app).

On first run: creates venv, installs deps, copies standalone code → ~/.aura-worker/
Subsequent runs: <3s startup (venv exists, standalone copied).

Usage (called by lib.rs auto-start):
    python research_worker.py --coordinator-url https://auraalpha.cc
    python research_worker.py --coordinator-url https://auraalpha.cc --token TOKEN
    python research_worker.py --coordinator-url https://auraalpha.cc --max-parallel 4
"""

import argparse
import hashlib
import json
import logging
import os
import platform
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

logging.basicConfig(level=logging.INFO, format="%(asctime)s [LAUNCHER] %(message)s")
log = logging.getLogger("research_launcher")

WORKER_HOME = Path.home() / ".aura-worker"
VENV_DIR = WORKER_HOME / "venv"
STANDALONE_DST = WORKER_HOME / "standalone"
LOG_DIR = WORKER_HOME / "logs"
CONFIG_PATH = WORKER_HOME / "config.yaml"
TOKEN_FILE = WORKER_HOME / "token"

# Bundled standalone source (relative to this script)
BUNDLED_STANDALONE = Path(__file__).resolve().parent / "standalone"


def detect_hardware() -> dict:
    """Auto-detect CPU/RAM for max_parallel cap."""
    cpus = os.cpu_count() or 1
    ram_gb = 4.0
    try:
        import psutil
        ram_gb = round(psutil.virtual_memory().total / (1024 ** 3), 1)
    except ImportError:
        try:
            with open("/proc/meminfo") as f:
                for line in f:
                    if line.startswith("MemTotal:"):
                        ram_gb = round(int(line.split()[1]) / (1024 ** 2), 1)
        except Exception:
            pass

    # Desktop cap: max 8 parallel to leave headroom for user
    max_parallel = min(8, max(1, cpus - 1))
    # Further cap by RAM: ~300MB per backtest worker
    max_by_ram = max(1, int(ram_gb * 0.5 / 0.3))
    max_parallel = min(max_parallel, max_by_ram)

    return {"cpus": cpus, "ram_gb": ram_gb, "max_parallel": max_parallel}


def resolve_token(coordinator_url: str, auth_token: str = "") -> str:
    """Resolve contributor token: config.yaml → token file → auto-provision."""

    # 1. Check config.yaml
    if CONFIG_PATH.exists():
        try:
            import yaml
            with open(CONFIG_PATH) as f:
                cfg = yaml.safe_load(f) or {}
            if cfg.get("token"):
                log.info("Token loaded from config.yaml")
                return cfg["token"]
        except Exception:
            pass

    # 2. Check token file — validate it still works before trusting it
    if TOKEN_FILE.exists():
        token = TOKEN_FILE.read_text().strip()
        if token:
            # Quick validation: try a heartbeat to see if the server knows this token
            try:
                import urllib.request
                import urllib.error
                vurl = f"{coordinator_url.rstrip('/')}/api/cluster/contributor/heartbeat"
                vreq = urllib.request.Request(vurl, method="POST",
                    data=json.dumps({"job_ids": []}).encode(),
                    headers={"Content-Type": "application/json",
                             "X-Contributor-Token": token})
                with urllib.request.urlopen(vreq, timeout=10):
                    pass
                log.info("Token loaded from token file (validated)")
                return token
            except urllib.error.HTTPError as e:
                if e.code in (401, 403, 404):
                    log.warning("Saved token is invalid (HTTP %d) — will re-provision", e.code)
                    TOKEN_FILE.unlink(missing_ok=True)
                else:
                    # Server error / other — trust the token for now
                    log.info("Token loaded from token file (server returned %d, trusting)", e.code)
                    return token
            except Exception:
                # Network error — trust the saved token
                log.info("Token loaded from token file (offline, trusting)")
                return token

    # 3. Auto-provision via API
    log.info("No existing token found — auto-provisioning from coordinator...")
    try:
        import urllib.request
        import urllib.error

        # Get friendly device name (macOS ComputerName, Windows COMPUTERNAME, Linux hostname)
        hostname = platform.node()
        if platform.system() == "Darwin":
            try:
                import subprocess as _sp
                _r = _sp.run(["scutil", "--get", "ComputerName"], capture_output=True, text=True, timeout=5)
                if _r.returncode == 0 and _r.stdout.strip():
                    hostname = _r.stdout.strip()
            except Exception:
                pass
        elif platform.system() == "Windows":
            hostname = os.environ.get("COMPUTERNAME", hostname)
        payload = json.dumps({
            "auth_token": auth_token or "desktop-auto",
            "hostname": hostname,
        }).encode()
        url = f"{coordinator_url.rstrip('/')}/api/cluster/contributor/auto-provision"
        req = urllib.request.Request(url, data=payload, method="POST",
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())
            token = data.get("token", "")
            if token:
                # Save for next time
                WORKER_HOME.mkdir(parents=True, exist_ok=True)
                TOKEN_FILE.write_text(token)
                log.info("Auto-provisioned and saved token")
                return token
    except Exception as e:
        log.warning("Auto-provision failed: %s", e)

    return ""


def ensure_venv():
    """Create venv and install deps if not already present."""
    python_in_venv = VENV_DIR / "bin" / "python"
    if sys.platform == "win32":
        python_in_venv = VENV_DIR / "Scripts" / "python.exe"

    if python_in_venv.exists():
        return str(python_in_venv)

    log.info("Creating virtual environment at %s ...", VENV_DIR)
    VENV_DIR.parent.mkdir(parents=True, exist_ok=True)
    subprocess.check_call([sys.executable, "-m", "venv", str(VENV_DIR)], timeout=60)

    # Install deps from bundled requirements.txt
    req_file = BUNDLED_STANDALONE / "requirements.txt"
    if req_file.exists():
        log.info("Installing dependencies...")
        subprocess.check_call(
            [str(python_in_venv), "-m", "pip", "install", "-q", "-r", str(req_file)],
            timeout=300,
        )

    return str(python_in_venv)


def sync_standalone():
    """Copy bundled standalone/ into ~/.aura-worker/standalone/ if newer."""
    if not BUNDLED_STANDALONE.exists():
        log.error("Bundled standalone not found at %s", BUNDLED_STANDALONE)
        return False

    STANDALONE_DST.mkdir(parents=True, exist_ok=True)

    # Simple sync: copy all .py and .txt files
    for src_file in BUNDLED_STANDALONE.iterdir():
        if src_file.suffix in (".py", ".txt"):
            dst_file = STANDALONE_DST / src_file.name
            # Only copy if source is newer or dest doesn't exist
            if not dst_file.exists() or src_file.stat().st_mtime > dst_file.stat().st_mtime:
                shutil.copy2(src_file, dst_file)

    return True


def launch_worker(python: str, coordinator_url: str, token: str, max_parallel: int) -> subprocess.Popen:
    """Launch the standalone worker as a subprocess."""
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    log_file = open(LOG_DIR / "worker.log", "a")

    cmd = [
        python, "-m", "standalone",
        "--coordinator-url", coordinator_url,
        "--token", token,
        "--max-parallel", str(max_parallel),
    ]

    log.info("Launching: %s", " ".join(cmd))

    proc = subprocess.Popen(
        cmd,
        cwd=str(WORKER_HOME),
        stdout=log_file,
        stderr=subprocess.STDOUT,
        env={**os.environ, "PYTHONPATH": str(WORKER_HOME)},
    )

    log.info("Worker started (PID %d)", proc.pid)
    return proc


def main():
    parser = argparse.ArgumentParser(description="Aura Alpha Research Worker Launcher")

    # New HTTPS-based args
    parser.add_argument("--coordinator-url", default="https://auraalpha.cc",
                        help="Coordinator API URL")
    parser.add_argument("--token", default="", help="Contributor token")
    parser.add_argument("--max-parallel", type=int, default=0,
                        help="Max parallel backtests (0=auto)")
    parser.add_argument("--auth-token", default="",
                        help="Platform auth token for auto-provision")

    # Legacy args (backward compat with existing lib.rs during transition)
    parser.add_argument("--redis-url", default="", help="(Legacy, ignored)")
    parser.add_argument("--parallel", type=int, default=0, help="(Legacy alias for --max-parallel)")

    parser.add_argument("--detect", action="store_true", help="Print hardware info and exit")

    args = parser.parse_args()

    # Handle legacy --parallel → --max-parallel
    if args.parallel > 0 and args.max_parallel == 0:
        args.max_parallel = args.parallel

    # Hardware detection
    hw = detect_hardware()

    if args.detect:
        print(json.dumps(hw, indent=2))
        return

    max_parallel = args.max_parallel if args.max_parallel > 0 else hw["max_parallel"]

    # Resolve token
    token = args.token or resolve_token(args.coordinator_url, args.auth_token)
    if not token:
        log.error("No contributor token available. Worker cannot start.")
        log.error("Set token in ~/.aura-worker/config.yaml or provide --token")
        sys.exit(1)

    # Bootstrap
    log.info("Hardware: %d CPUs, %.1f GB RAM → max_parallel=%d", hw["cpus"], hw["ram_gb"], max_parallel)

    if not sync_standalone():
        log.error("Failed to sync standalone worker code")
        sys.exit(1)

    python = ensure_venv()

    # Launch worker subprocess
    proc = launch_worker(python, args.coordinator_url, token, max_parallel)

    # Forward signals to child
    def forward_signal(sig, frame):
        if proc.poll() is None:
            proc.send_signal(sig)

    signal.signal(signal.SIGINT, forward_signal)
    signal.signal(signal.SIGTERM, forward_signal)

    # Wait for child to exit
    try:
        proc.wait()
    except KeyboardInterrupt:
        proc.terminate()
        proc.wait(timeout=10)

    log.info("Worker exited with code %d", proc.returncode or 0)
    sys.exit(proc.returncode or 0)


if __name__ == "__main__":
    main()
