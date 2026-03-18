"""
Adaptive Throttle — Monitors system load and yields to other processes.
========================================================================
Checks CPU and RAM pressure from non-worker processes (games, apps, etc.)
and dynamically reduces/increases the worker pool size.

Rules:
  - If other processes use >60% CPU → drop to 25% of max workers
  - If other processes use >40% CPU → drop to 50% of max workers
  - If other processes use >20% CPU → drop to 75% of max workers
  - If RAM available <25% → drop to minimum (2 workers)
  - Otherwise → full max_parallel
  - Never drops below 1 worker
  - Changes are gradual (ramp up slowly, drop fast)

Cross-platform: works on Windows, Linux, macOS.
"""
from __future__ import annotations

import logging
import os
import time
from typing import Optional

log = logging.getLogger("standalone.throttle")

# How often to re-check (seconds)
CHECK_INTERVAL = 10

# Thresholds for "other process" CPU usage (percentage of total CPU)
HEAVY_LOAD = 60    # gaming, video editing — drop to 25%
MEDIUM_LOAD = 40   # moderate apps — drop to 50%
LIGHT_LOAD = 20    # normal desktop use — drop to 75%

# RAM threshold — if available RAM drops below this %, go minimal
RAM_CRITICAL_PCT = 25

# How fast to ramp back up (prevents yo-yoing)
RAMP_UP_STEP = 2   # add 2 workers per check when ramping up
RAMP_DOWN_INSTANT = True  # drop immediately when load detected


def _get_system_metrics() -> dict:
    """Get CPU and RAM metrics. Returns {cpu_pct, ram_available_pct, cpu_count}."""
    try:
        import psutil
        # CPU: average over 1 second (non-blocking if called periodically)
        cpu_pct = psutil.cpu_percent(interval=0.5)
        mem = psutil.virtual_memory()
        return {
            "cpu_pct": cpu_pct,
            "ram_available_pct": mem.available / mem.total * 100,
            "ram_used_gb": (mem.total - mem.available) / (1024 ** 3),
            "ram_total_gb": mem.total / (1024 ** 3),
            "cpu_count": psutil.cpu_count(),
        }
    except ImportError:
        pass

    # Fallback: /proc on Linux
    try:
        # CPU from /proc/stat
        with open("/proc/stat") as f:
            parts = f.readline().split()
            idle = int(parts[4])
            total = sum(int(p) for p in parts[1:])

        time.sleep(0.3)

        with open("/proc/stat") as f:
            parts2 = f.readline().split()
            idle2 = int(parts2[4])
            total2 = sum(int(p) for p in parts2[1:])

        d_total = total2 - total
        d_idle = idle2 - idle
        cpu_pct = ((d_total - d_idle) / max(d_total, 1)) * 100

        # RAM from /proc/meminfo
        meminfo = {}
        with open("/proc/meminfo") as f:
            for line in f:
                parts = line.split()
                meminfo[parts[0].rstrip(":")] = int(parts[1])

        total_kb = meminfo.get("MemTotal", 1)
        avail_kb = meminfo.get("MemAvailable", meminfo.get("MemFree", 0))

        return {
            "cpu_pct": round(cpu_pct, 1),
            "ram_available_pct": round(avail_kb / total_kb * 100, 1),
            "ram_used_gb": round((total_kb - avail_kb) / (1024 ** 2), 1),
            "ram_total_gb": round(total_kb / (1024 ** 2), 1),
            "cpu_count": os.cpu_count() or 1,
        }
    except Exception:
        return {
            "cpu_pct": 0,
            "ram_available_pct": 100,
            "ram_used_gb": 0,
            "ram_total_gb": 0,
            "cpu_count": os.cpu_count() or 1,
        }


def _estimate_worker_cpu(max_parallel: int, cpu_count: int) -> float:
    """Estimate how much CPU our workers use (rough: each worker ≈ 1 core)."""
    return (max_parallel / max(cpu_count, 1)) * 100


class AdaptiveThrottle:
    """Monitors system load and recommends worker count."""

    def __init__(self, max_parallel: int):
        self.max_parallel = max_parallel
        self.current_parallel = max_parallel
        self._last_check = 0.0
        self._last_metrics: Optional[dict] = None

    def recommended_workers(self) -> int:
        """Returns the recommended number of parallel workers right now.

        Call this before each batch. It checks system metrics and adjusts.
        Caches the result for CHECK_INTERVAL seconds to avoid hammering /proc.
        """
        now = time.time()
        if now - self._last_check < CHECK_INTERVAL:
            return self.current_parallel

        self._last_check = now
        metrics = _get_system_metrics()
        self._last_metrics = metrics

        total_cpu = metrics["cpu_pct"]
        ram_avail = metrics["ram_available_pct"]
        cpu_count = metrics["cpu_count"]

        # Estimate how much CPU is US vs OTHER processes
        our_estimated_cpu = _estimate_worker_cpu(self.current_parallel, cpu_count)
        other_cpu = max(0, total_cpu - our_estimated_cpu)

        # Determine target based on OTHER process load
        if ram_avail < RAM_CRITICAL_PCT:
            target = max(1, 2)
            reason = f"RAM critical ({ram_avail:.0f}% available)"
        elif other_cpu >= HEAVY_LOAD:
            target = max(1, self.max_parallel // 4)
            reason = f"heavy load ({other_cpu:.0f}% other CPU)"
        elif other_cpu >= MEDIUM_LOAD:
            target = max(1, self.max_parallel // 2)
            reason = f"medium load ({other_cpu:.0f}% other CPU)"
        elif other_cpu >= LIGHT_LOAD:
            target = max(2, int(self.max_parallel * 0.75))
            reason = f"light load ({other_cpu:.0f}% other CPU)"
        else:
            target = self.max_parallel
            reason = "idle"

        prev = self.current_parallel

        # Drop fast, ramp up slowly
        if target < self.current_parallel:
            self.current_parallel = target
        elif target > self.current_parallel:
            self.current_parallel = min(target, self.current_parallel + RAMP_UP_STEP)

        if self.current_parallel != prev:
            log.info(
                "Throttle: %d → %d workers (%s) | CPU: %.0f%% (other: %.0f%%) | RAM: %.0f%% free",
                prev, self.current_parallel, reason,
                total_cpu, other_cpu, ram_avail,
            )

        return self.current_parallel

    @property
    def metrics(self) -> Optional[dict]:
        """Last captured system metrics."""
        return self._last_metrics

    @property
    def is_throttled(self) -> bool:
        """True if currently running below max capacity."""
        return self.current_parallel < self.max_parallel
