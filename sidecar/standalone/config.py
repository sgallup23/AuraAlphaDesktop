"""
Worker configuration with auto-detection, YAML file, and env var overrides.
"""
from __future__ import annotations

import os
import platform
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional

import psutil
import yaml


def _default_worker_id() -> str:
    """Generate a deterministic-ish worker ID from hostname + short uuid."""
    hostname = platform.node().split(".")[0].lower()
    short = uuid.uuid4().hex[:8]
    return f"{hostname}-{short}"


def _auto_max_parallel() -> int:
    """Auto-detect reasonable parallelism from CPU count."""
    cpus = os.cpu_count() or 1
    # Leave 1 core free for the OS / main thread
    return max(1, cpus - 1)


def _auto_ram_gb() -> float:
    """Total system RAM in GB."""
    return round(psutil.virtual_memory().total / (1024 ** 3), 1)


@dataclass
class WorkerConfig:
    """Configuration for a standalone research worker."""

    # ── Coordinator connection ─────────────────────────────────────────
    coordinator_url: str = "https://auraalpha.cc"
    token: str = ""
    worker_id: str = ""

    # ── Compute limits ─────────────────────────────────────────────────
    max_parallel: int = 0  # 0 = auto-detect
    batch_size: int = 5

    # ── Paths ──────────────────────────────────────────────────────────
    cache_dir: Path = field(default_factory=lambda: Path.home() / ".aura-worker" / "data")
    log_dir: Path = field(default_factory=lambda: Path.home() / ".aura-worker" / "logs")

    # ── Timing ─────────────────────────────────────────────────────────
    heartbeat_interval: int = 30  # seconds
    job_timeout: int = 600  # seconds per job

    # ── Job types ──────────────────────────────────────────────────────
    supported_job_types: List[str] = field(
        default_factory=lambda: ["research_backtest", "strategy_backtest"],
    )

    # ── Detected hardware ──────────────────────────────────────────────
    cpu_count: int = field(default_factory=lambda: os.cpu_count() or 1)
    ram_gb: float = field(default_factory=_auto_ram_gb)

    def __post_init__(self) -> None:
        # Auto-generate worker_id if not set
        if not self.worker_id:
            self.worker_id = _default_worker_id()
        # Auto-detect parallelism if not set
        if self.max_parallel <= 0:
            self.max_parallel = _auto_max_parallel()
        # Ensure Path types
        if isinstance(self.cache_dir, str):
            self.cache_dir = Path(self.cache_dir)
        if isinstance(self.log_dir, str):
            self.log_dir = Path(self.log_dir)

    def ensure_dirs(self) -> None:
        """Create cache and log directories if they don't exist."""
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.log_dir.mkdir(parents=True, exist_ok=True)

    @classmethod
    def load(cls) -> "WorkerConfig":
        """Load config with layered precedence: defaults → YAML → env vars."""
        config_path = Path.home() / ".aura-worker" / "config.yaml"

        # Start with defaults
        kwargs: dict = {}

        # Layer 1: YAML file
        if config_path.exists():
            try:
                with open(config_path) as f:
                    data = yaml.safe_load(f) or {}
                for key in (
                    "coordinator_url", "token", "worker_id", "max_parallel",
                    "batch_size", "cache_dir", "log_dir", "heartbeat_interval",
                    "job_timeout", "supported_job_types",
                ):
                    if key in data:
                        kwargs[key] = data[key]
            except Exception:
                pass  # Fall through to defaults

        # Layer 2: Environment variable overrides
        env_map = {
            "AURA_COORDINATOR_URL": "coordinator_url",
            "AURA_TOKEN": "token",
            "AURA_WORKER_ID": "worker_id",
            "AURA_MAX_PARALLEL": "max_parallel",
            "AURA_JOB_TYPES": "supported_job_types",
        }
        for env_key, config_key in env_map.items():
            val = os.environ.get(env_key)
            if val is not None:
                if config_key == "max_parallel":
                    kwargs[config_key] = int(val)
                elif config_key == "supported_job_types":
                    # Comma-separated: "research_backtest,strategy_backtest,signal_gen"
                    kwargs[config_key] = [t.strip() for t in val.split(",") if t.strip()]
                else:
                    kwargs[config_key] = val

        return cls(**kwargs)
