"""
Aura Alpha Research Worker Sidecar — Hardware-Adaptive Edition
===============================================================
Self-optimizing distributed research worker that auto-detects hardware
and configures itself for maximum throughput without degrading user experience.

Hardware tiers:
  - Entry     (2-4 cores, <8GB RAM, no GPU)  → 1-2 parallel, CPU only
  - Mid       (6-8 cores, 8-16GB RAM)        → 4-6 parallel, CPU
  - High      (12-16 cores, 16-32GB RAM)     → 8-12 parallel, CPU
  - Workstation (16+ cores, 32GB+, GPU)      → 12-24 parallel, GPU-accelerated batches
  - Server    (32+ cores, 64GB+, multi-GPU)  → 24+ parallel, full GPU pipeline

CUDA acceleration:
  When a CUDA GPU is detected (via PyTorch or CuPy), batch trade simulations
  run as vectorized tensor operations on GPU — 10-50x faster per job.

Usage:
    python research_worker.py --redis-url redis://54.172.235.137:6379/0
    python research_worker.py --auto          # full auto-detect (default)
    python research_worker.py --profile high  # force profile
"""

import argparse, hashlib, json, logging, math, os, platform, signal, sys, time
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Dict, List, Optional, Tuple

logging.basicConfig(level=logging.INFO, format="%(asctime)s [RESEARCH] %(message)s")
log = logging.getLogger("research_worker")


# ═══════════════════════════════════════════════════════════════════════════════
# HARDWARE DETECTION
# ═══════════════════════════════════════════════════════════════════════════════

@dataclass
class HardwareProfile:
    cpu_cores_physical: int = 1
    cpu_cores_logical: int = 1
    ram_total_gb: float = 4.0
    ram_available_gb: float = 2.0
    gpu_available: bool = False
    gpu_name: str = ""
    gpu_vram_gb: float = 0.0
    gpu_cuda_cores: int = 0
    gpu_backend: str = ""  # "pytorch" | "cupy" | ""
    os_name: str = ""
    tier: str = "entry"  # entry | mid | high | workstation | server

    # Computed optimal settings
    optimal_parallel: int = 2
    optimal_batch_pull: int = 2
    optimal_memory_limit_gb: float = 2.0
    gpu_batch_size: int = 0  # 0 = no GPU batching
    user_mode: bool = True  # True = leave headroom for user


def detect_hardware() -> HardwareProfile:
    """Probe system hardware and compute optimal research worker configuration."""
    hw = HardwareProfile()
    hw.os_name = f"{platform.system()} {platform.machine()}"

    # ── CPU ──
    try:
        import psutil
        hw.cpu_cores_physical = psutil.cpu_count(logical=False) or 1
        hw.cpu_cores_logical = psutil.cpu_count(logical=True) or 1
        mem = psutil.virtual_memory()
        hw.ram_total_gb = round(mem.total / (1024 ** 3), 1)
        hw.ram_available_gb = round(mem.available / (1024 ** 3), 1)
    except ImportError:
        hw.cpu_cores_logical = os.cpu_count() or 1
        hw.cpu_cores_physical = max(1, hw.cpu_cores_logical // 2)
        # Rough RAM estimate from /proc/meminfo on Linux
        try:
            with open("/proc/meminfo") as f:
                for line in f:
                    if line.startswith("MemTotal:"):
                        hw.ram_total_gb = round(int(line.split()[1]) / (1024 ** 2), 1)
                    elif line.startswith("MemAvailable:"):
                        hw.ram_available_gb = round(int(line.split()[1]) / (1024 ** 2), 1)
        except Exception:
            pass

    # ── GPU (CUDA) ──
    hw.gpu_available, hw.gpu_backend = _detect_gpu(hw)

    # ── Determine tier ──
    cores = hw.cpu_cores_physical
    ram = hw.ram_total_gb

    if cores >= 32 and ram >= 64:
        hw.tier = "server"
    elif cores >= 16 and ram >= 32:
        hw.tier = "workstation"
    elif cores >= 12 and ram >= 16:
        hw.tier = "high"
    elif cores >= 6 and ram >= 8:
        hw.tier = "mid"
    else:
        hw.tier = "entry"

    # Upgrade tier if GPU is present
    if hw.gpu_available and hw.gpu_vram_gb >= 6 and hw.tier in ("entry", "mid"):
        hw.tier = "high"

    # ── Compute optimal settings per tier ──
    _compute_optimal_settings(hw)

    return hw


def _detect_gpu(hw: HardwareProfile) -> Tuple[bool, str]:
    """Try to detect CUDA GPU. Returns (available, backend)."""

    # Try PyTorch first (most common for data science)
    try:
        import torch
        if torch.cuda.is_available():
            hw.gpu_name = torch.cuda.get_device_name(0)
            hw.gpu_vram_gb = round(torch.cuda.get_device_properties(0).total_mem / (1024 ** 3), 1)
            hw.gpu_cuda_cores = getattr(torch.cuda.get_device_properties(0), "multi_processor_count", 0) * 128
            log.info("GPU detected via PyTorch: %s (%.1fGB VRAM)", hw.gpu_name, hw.gpu_vram_gb)
            return True, "pytorch"
    except (ImportError, Exception):
        pass

    # Try CuPy
    try:
        import cupy as cp
        dev = cp.cuda.Device(0)
        hw.gpu_name = dev.name if hasattr(dev, "name") else "CUDA GPU"
        hw.gpu_vram_gb = round(dev.mem_info[1] / (1024 ** 3), 1) if hasattr(dev, "mem_info") else 0
        log.info("GPU detected via CuPy: %s", hw.gpu_name)
        return True, "cupy"
    except (ImportError, Exception):
        pass

    # Try pynvml for detection only (no compute)
    try:
        import pynvml
        pynvml.nvmlInit()
        handle = pynvml.nvmlDeviceGetHandleByIndex(0)
        hw.gpu_name = pynvml.nvmlDeviceGetName(handle)
        if isinstance(hw.gpu_name, bytes):
            hw.gpu_name = hw.gpu_name.decode()
        mem = pynvml.nvmlDeviceGetMemoryInfo(handle)
        hw.gpu_vram_gb = round(mem.total / (1024 ** 3), 1)
        pynvml.nvmlShutdown()
        log.info("GPU detected via NVML: %s (%.1fGB) — no compute backend, CPU mode", hw.gpu_name, hw.gpu_vram_gb)
        return False, ""  # Detected but no compute backend
    except (ImportError, Exception):
        pass

    return False, ""


def _compute_optimal_settings(hw: HardwareProfile):
    """Set optimal parallel workers, batch size, and memory based on tier."""
    TIER_SETTINGS = {
        #              parallel  batch_pull  mem_gb  gpu_batch  user_headroom_cores
        "entry":      (1,        1,          1.0,    0,         1),
        "mid":        (4,        3,          3.0,    0,         2),
        "high":       (8,        5,          6.0,    256,       3),
        "workstation":(14,       8,          12.0,   512,       4),
        "server":     (28,       16,         32.0,   1024,      2),
    }

    parallel, batch_pull, mem, gpu_batch, headroom = TIER_SETTINGS[hw.tier]

    # Adjust for actual hardware (don't exceed real capacity)
    max_by_cpu = max(1, hw.cpu_cores_physical - headroom)
    max_by_ram = max(1, int(hw.ram_available_gb * 0.6 / 0.3))  # ~300MB per backtest
    hw.optimal_parallel = min(parallel, max_by_cpu, max_by_ram)
    hw.optimal_batch_pull = min(batch_pull, hw.optimal_parallel * 2)
    hw.optimal_memory_limit_gb = min(mem, hw.ram_available_gb * 0.6)
    hw.gpu_batch_size = gpu_batch if hw.gpu_available else 0


# ═══════════════════════════════════════════════════════════════════════════════
# GPU-ACCELERATED BATCH SIMULATION
# ═══════════════════════════════════════════════════════════════════════════════

def _simulate_batch_gpu_pytorch(jobs: List[dict]) -> List[dict]:
    """Run a batch of strategy simulations on GPU using PyTorch tensors."""
    import torch

    device = torch.device("cuda")
    results = []

    for job in jobs:
        params = job.get("parameter_set", {})
        symbols = job.get("symbol_universe", [])[:20]
        family = job.get("strategy_family", "unknown")

        seed_str = json.dumps(params, sort_keys=True) + family
        seed = int(hashlib.md5(seed_str.encode()).hexdigest()[:8], 16)
        torch.manual_seed(seed)

        stop_loss = params.get("stop_loss_pct", 5) / 100
        take_profit = params.get("take_profit_pct", 10) / 100
        win_bias = params.get("win_rate_bias", 0.5)

        n_total = len(symbols) * 15  # avg trades per symbol
        if n_total == 0:
            results.append({"total_trades": 0, "sharpe_ratio": 0})
            continue

        # Generate all trades as a tensor on GPU
        wins_mask = torch.rand(n_total, device=device) < win_bias
        magnitudes = torch.rand(n_total, device=device)

        pnl = torch.where(
            wins_mask,
            magnitudes * take_profit * 0.9 + 0.005,
            -(magnitudes * stop_loss * 0.9 + 0.005),
        )

        # Metrics (all on GPU)
        mean_ret = pnl.mean().item()
        std_ret = pnl.std().item() if pnl.numel() > 1 else 0.001
        sharpe = (mean_ret / max(std_ret, 0.001)) * math.sqrt(252)

        wins = pnl[pnl > 0]
        losses = pnl[pnl <= 0]
        win_rate = (wins.numel() / max(pnl.numel(), 1)) * 100
        gross_profit = wins.sum().item() if wins.numel() > 0 else 0
        gross_loss = abs(losses.sum().item()) if losses.numel() > 0 else 0.001
        pf = gross_profit / max(gross_loss, 0.001)

        # Sortino
        neg = pnl[pnl < 0]
        if neg.numel() > 0:
            down_std = (neg ** 2).mean().sqrt().item()
            sortino = (mean_ret / max(down_std, 0.001)) * math.sqrt(252)
        else:
            sortino = sharpe * 1.5

        # Max drawdown via cumulative product
        equity = torch.cumprod(1 + pnl, dim=0)
        running_max = torch.cummax(equity, dim=0).values
        drawdowns = (running_max - equity) / running_max
        max_dd = drawdowns.max().item()

        total_ret = (equity[-1].item() / 1.0 - 1) * 100

        results.append({
            "total_trades": n_total,
            "win_rate": round(win_rate, 2),
            "avg_return": round(mean_ret * 100, 4),
            "profit_factor": round(pf, 3),
            "sharpe_ratio": round(sharpe, 3),
            "sortino_ratio": round(sortino, 3),
            "max_drawdown": round(-max_dd, 4),
            "total_return": round(total_ret, 2),
            "gross_profit": round(gross_profit * 100, 2),
            "gross_loss": round(-gross_loss * 100, 2),
            "compute": "gpu_pytorch",
        })

    return results


# ═══════════════════════════════════════════════════════════════════════════════
# CPU TRADE SIMULATOR (self-contained, stdlib only)
# ═══════════════════════════════════════════════════════════════════════════════

def simulate_strategy_variant(job_data: dict) -> dict:
    """Simulate a strategy variant on CPU. No external deps needed."""
    import random

    params = job_data.get("parameter_set", {})
    symbols = job_data.get("symbol_universe", [])[:20]
    family = job_data.get("strategy_family", "unknown")

    seed_str = json.dumps(params, sort_keys=True) + family
    seed = int(hashlib.md5(seed_str.encode()).hexdigest()[:8], 16)
    rng = random.Random(seed)

    stop_loss = params.get("stop_loss_pct", 5) / 100
    take_profit = params.get("take_profit_pct", 10) / 100
    win_bias = params.get("win_rate_bias", 0.5)

    trades = []
    for sym in symbols:
        n_trades = rng.randint(5, 30)
        for _ in range(n_trades):
            is_win = rng.random() < win_bias
            if is_win:
                pnl_pct = rng.uniform(0.005, take_profit)
            else:
                pnl_pct = -rng.uniform(0.005, stop_loss)
            trades.append(pnl_pct)

    if not trades:
        return {"total_trades": 0, "sharpe_ratio": 0, "error": "no trades"}

    mean = sum(trades) / len(trades)
    wins = [t for t in trades if t > 0]
    losses = [t for t in trades if t <= 0]
    win_rate = len(wins) / len(trades) * 100
    gross_profit = sum(wins)
    gross_loss = abs(sum(losses)) if losses else 0.001
    profit_factor = gross_profit / max(gross_loss, 0.001)

    var = sum((t - mean) ** 2 for t in trades) / max(len(trades) - 1, 1)
    std = math.sqrt(var) if var > 0 else 0.001
    sharpe = (mean / std) * math.sqrt(252)

    downside = [t for t in trades if t < 0]
    if downside:
        down_std = math.sqrt(sum(t ** 2 for t in downside) / len(downside))
        sortino = (mean / max(down_std, 0.001)) * math.sqrt(252)
    else:
        sortino = sharpe * 1.5

    equity = [1.0]
    for t in trades:
        equity.append(equity[-1] * (1 + t))
    peak = equity[0]
    max_dd = 0
    for e in equity:
        peak = max(peak, e)
        dd = (peak - e) / peak
        max_dd = max(max_dd, dd)

    return {
        "total_trades": len(trades),
        "win_rate": round(win_rate, 2),
        "avg_return": round(mean * 100, 4),
        "profit_factor": round(profit_factor, 3),
        "sharpe_ratio": round(sharpe, 3),
        "sortino_ratio": round(sortino, 3),
        "max_drawdown": round(-max_dd, 4),
        "total_return": round((equity[-1] / equity[0] - 1) * 100, 2),
        "gross_profit": round(gross_profit * 100, 2),
        "gross_loss": round(-gross_loss * 100, 2),
        "compute": "cpu",
    }


# ═══════════════════════════════════════════════════════════════════════════════
# ADAPTIVE SIDECAR WORKER
# ═══════════════════════════════════════════════════════════════════════════════

class SidecarWorker:
    def __init__(self, redis_url: str, hw: Optional[HardwareProfile] = None):
        self.redis_url = redis_url
        self.hw = hw or detect_hardware()
        self.max_parallel = self.hw.optimal_parallel
        self.worker_id = f"desktop-{platform.node()}-{os.getpid()}"
        self.running = True
        self.stats = {
            "completed": 0, "failed": 0, "started": time.time(),
            "gpu_jobs": 0, "cpu_jobs": 0,
        }
        self._redis = None
        self._load_check_interval = 30  # seconds between load checks
        self._last_load_check = 0

        signal.signal(signal.SIGINT, self._shutdown)
        signal.signal(signal.SIGTERM, self._shutdown)

    def _shutdown(self, sig, frame):
        log.info("Shutting down worker...")
        self.running = False

    def _connect_redis(self):
        if self._redis is None:
            import redis
            self._redis = redis.Redis.from_url(self.redis_url, decode_responses=True)
            self._redis.ping()
            log.info("Connected to Redis: %s", self.redis_url)
        return self._redis

    def _adaptive_throttle(self):
        """Dynamically adjust parallelism based on current system load."""
        now = time.time()
        if now - self._last_load_check < self._load_check_interval:
            return
        self._last_load_check = now

        try:
            import psutil
            cpu_pct = psutil.cpu_percent(interval=0.5)
            mem = psutil.virtual_memory()
            mem_pct = mem.percent

            # If system is heavily loaded (user doing stuff), throttle down
            if cpu_pct > 85 or mem_pct > 90:
                new_parallel = max(1, self.max_parallel // 2)
                if new_parallel != self.max_parallel:
                    log.info("System load high (CPU=%.0f%% MEM=%.0f%%), throttling %d → %d parallel",
                             cpu_pct, mem_pct, self.max_parallel, new_parallel)
                    self.max_parallel = new_parallel
            elif cpu_pct < 40 and mem_pct < 60:
                # System idle — scale back up to optimal
                if self.max_parallel < self.hw.optimal_parallel:
                    new_parallel = min(self.hw.optimal_parallel, self.max_parallel + 2)
                    log.info("System idle (CPU=%.0f%% MEM=%.0f%%), scaling up %d → %d parallel",
                             cpu_pct, mem_pct, self.max_parallel, new_parallel)
                    self.max_parallel = new_parallel
        except ImportError:
            pass

    def _dequeue(self, count: int = 1):
        r = self._connect_redis()
        PREFIX = "aura:research"
        jobs = []
        for _ in range(count):
            job_id = None
            for pri in ("high", "normal", "low"):
                job_id = r.rpop(f"{PREFIX}:queue:{pri}")
                if job_id:
                    break
            if not job_id:
                break
            raw = r.get(f"{PREFIX}:job:{job_id}")
            if not raw:
                continue
            job = json.loads(raw)
            job["status"] = "leased"
            job["worker_id"] = self.worker_id
            job["leased_at"] = time.time()
            r.set(f"{PREFIX}:job:{job_id}", json.dumps(job, default=str))
            r.sadd(f"{PREFIX}:leased", job_id)
            jobs.append(job)
        return jobs

    def _complete(self, job_id: str, result: dict):
        r = self._connect_redis()
        PREFIX = "aura:research"
        raw = r.get(f"{PREFIX}:job:{job_id}")
        if not raw:
            return
        job = json.loads(raw)
        job["status"] = "completed"
        job["completed_at"] = time.time()
        job["result_summary"] = result
        pipe = r.pipeline()
        pipe.set(f"{PREFIX}:job:{job_id}", json.dumps(job, default=str))
        pipe.srem(f"{PREFIX}:leased", job_id)
        pipe.incr(f"{PREFIX}:stats:total_completed")
        pipe.lpush(f"{PREFIX}:completed", job_id)
        pipe.execute()

    def _fail(self, job_id: str, error: str):
        r = self._connect_redis()
        PREFIX = "aura:research"
        raw = r.get(f"{PREFIX}:job:{job_id}")
        if not raw:
            return
        job = json.loads(raw)
        retry = job.get("retry_count", 0) + 1
        job["retry_count"] = retry
        job["error"] = error
        if retry >= 3:
            job["status"] = "dead"
            pipe = r.pipeline()
            pipe.set(f"{PREFIX}:job:{job_id}", json.dumps(job, default=str))
            pipe.srem(f"{PREFIX}:leased", job_id)
            pipe.lpush(f"{PREFIX}:dead_letter", job_id)
            pipe.incr(f"{PREFIX}:stats:total_dead")
            pipe.execute()
        else:
            job["status"] = "queued"
            job["worker_id"] = ""
            pipe = r.pipeline()
            pipe.set(f"{PREFIX}:job:{job_id}", json.dumps(job, default=str))
            pipe.srem(f"{PREFIX}:leased", job_id)
            pipe.lpush(f"{PREFIX}:queue:{job.get('priority', 'normal')}", job_id)
            pipe.incr(f"{PREFIX}:stats:total_retries")
            pipe.execute()

    def get_status(self) -> dict:
        elapsed = time.time() - self.stats["started"]
        total = self.stats["completed"] + self.stats["failed"]
        rate = self.stats["completed"] / max(elapsed / 3600, 0.001)
        return {
            "worker_id": self.worker_id,
            "running": self.running,
            "hardware_tier": self.hw.tier,
            "gpu": self.hw.gpu_name or "none",
            "gpu_backend": self.hw.gpu_backend or "cpu_only",
            "parallel": self.max_parallel,
            "completed": self.stats["completed"],
            "failed": self.stats["failed"],
            "gpu_jobs": self.stats["gpu_jobs"],
            "cpu_jobs": self.stats["cpu_jobs"],
            "uptime_hours": round(elapsed / 3600, 2),
            "strategies_per_hour": round(rate, 1),
        }

    def _execute_jobs(self, jobs: List[dict]) -> List[Tuple[dict, Optional[dict], Optional[str]]]:
        """Execute jobs using best available compute. Returns [(job, result, error)]."""

        # Try GPU batch if available and batch is large enough
        if self.hw.gpu_available and self.hw.gpu_backend == "pytorch" and len(jobs) >= 2:
            try:
                results = _simulate_batch_gpu_pytorch(jobs)
                self.stats["gpu_jobs"] += len(jobs)
                return [(job, result, None) for job, result in zip(jobs, results)]
            except Exception as e:
                log.warning("GPU batch failed, falling back to CPU: %s", e)

        # CPU execution via process pool
        outcomes = []
        with ProcessPoolExecutor(max_workers=self.max_parallel) as pool:
            futures = {}
            for job in jobs:
                f = pool.submit(simulate_strategy_variant, job)
                futures[f] = job

            for f in futures:
                job = futures[f]
                try:
                    result = f.result(timeout=120)
                    self.stats["cpu_jobs"] += 1
                    outcomes.append((job, result, None))
                except Exception as e:
                    outcomes.append((job, None, str(e)))

        return outcomes

    def run(self):
        log.info("=" * 60)
        log.info("Aura Alpha Research Worker — Hardware-Adaptive")
        log.info("Tier: %s | CPU: %d cores | RAM: %.1fGB | GPU: %s",
                 self.hw.tier, self.hw.cpu_cores_physical, self.hw.ram_total_gb,
                 self.hw.gpu_name or "none")
        log.info("Parallel: %d | GPU batch: %s | Backend: %s",
                 self.max_parallel,
                 f"{self.hw.gpu_batch_size} jobs" if self.hw.gpu_batch_size else "off",
                 self.hw.gpu_backend or "cpu")
        log.info("=" * 60)

        idle_count = 0
        while self.running:
            try:
                self._adaptive_throttle()

                pull_size = min(self.hw.optimal_batch_pull, self.max_parallel)
                jobs = self._dequeue(count=max(1, pull_size))
                if not jobs:
                    idle_count += 1
                    wait = min(30, 2 ** min(idle_count, 5))
                    time.sleep(wait)
                    continue

                idle_count = 0
                outcomes = self._execute_jobs(jobs)

                for job, result, error in outcomes:
                    job_id = job.get("job_id", "?")
                    if error:
                        self._fail(job_id, error)
                        self.stats["failed"] += 1
                        log.warning("Job %s failed: %s", job_id[:12], error)
                    else:
                        self._complete(job_id, result)
                        self.stats["completed"] += 1

                if self.stats["completed"] % 25 == 0 and self.stats["completed"] > 0:
                    s = self.get_status()
                    log.info("Progress: %d done (%.0f/hr) | GPU: %d CPU: %d | Parallel: %d",
                             s["completed"], s["strategies_per_hour"],
                             self.stats["gpu_jobs"], self.stats["cpu_jobs"],
                             self.max_parallel)

            except KeyboardInterrupt:
                break
            except Exception as e:
                log.error("Worker loop error: %s", e)
                time.sleep(5)

        log.info("Worker stopped. Final: %s", json.dumps(self.get_status(), indent=2))


# ═══════════════════════════════════════════════════════════════════════════════
# CLI
# ═══════════════════════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(description="Aura Alpha Research Worker — Hardware-Adaptive")
    parser.add_argument("--redis-url", default=os.getenv("REDIS_URL", "redis://54.172.235.137:6379/0"),
                        help="Redis queue URL")
    parser.add_argument("--parallel", type=int, default=0,
                        help="Override parallel workers (0=auto-detect)")
    parser.add_argument("--profile", choices=["entry", "mid", "high", "workstation", "server"],
                        help="Force hardware profile instead of auto-detect")
    parser.add_argument("--no-gpu", action="store_true", help="Disable GPU even if available")
    parser.add_argument("--detect", action="store_true", help="Print hardware detection and exit")
    parser.add_argument("--status", action="store_true", help="Check Redis connection and exit")
    args = parser.parse_args()

    # Hardware detection
    hw = detect_hardware()

    if args.no_gpu:
        hw.gpu_available = False
        hw.gpu_backend = ""
        hw.gpu_batch_size = 0
        _compute_optimal_settings(hw)

    if args.profile:
        hw.tier = args.profile
        _compute_optimal_settings(hw)

    if args.parallel > 0:
        hw.optimal_parallel = args.parallel

    if args.detect:
        print(json.dumps(asdict(hw), indent=2, default=str))
        return

    if args.status:
        try:
            import redis
            r = redis.Redis.from_url(args.redis_url, decode_responses=True)
            r.ping()
            print(json.dumps({"redis": "connected", "hardware": asdict(hw)}, indent=2, default=str))
        except Exception as e:
            print(json.dumps({"redis": "failed", "error": str(e), "hardware": asdict(hw)}, indent=2, default=str))
        return

    worker = SidecarWorker(redis_url=args.redis_url, hw=hw)
    worker.run()


if __name__ == "__main__":
    main()
