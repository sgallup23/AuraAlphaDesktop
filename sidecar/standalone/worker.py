"""
StandaloneWorker — main worker loop.
Connects to coordinator via HTTPS, pulls jobs, executes backtests in parallel,
reports results. No Redis or shared filesystem needed.

Supported job types:
  - research_backtest  — Alpha Factory research backtests
  - strategy_backtest  — production strategy backtests
  - signal_gen         — signal generation (requires generate_signals executor)
"""
from __future__ import annotations

import logging
import os
import platform
import signal
import sys
import time
import traceback
from concurrent.futures import ProcessPoolExecutor, as_completed, TimeoutError as FuturesTimeout
from threading import Event, Thread
from typing import Any, Callable, Dict, List, Optional

from .api_client import CoordinatorClient
from .backtest_engine import run_single_research_job
from .config import WorkerConfig
from .data_fetcher import DataFetcher

log = logging.getLogger("standalone.worker")


# ── Job executors for each type ──────────────────────────────────────────────


def _run_strategy_backtest(job_dict: Dict[str, Any], cache_dir) -> Dict[str, Any]:
    """Execute a production strategy backtest job.

    Uses the same engine as research_backtest — the job payload format is
    identical.  Separated so future optimisations can diverge.
    """
    return run_single_research_job(job_dict, cache_dir)


def _run_signal_gen(job_dict: Dict[str, Any], cache_dir) -> Dict[str, Any]:
    """Execute a signal generation job.

    Expected job_dict fields (beyond the standard ones):
        strategy_id   str
        symbols       list[str]
        region        str
        parameters    dict
    """
    try:
        job_id = job_dict.get("job_id", "unknown")
        strategy_id = job_dict.get("strategy_id", "unknown")
        symbols = job_dict.get("symbols", job_dict.get("symbol_universe", []))
        region = job_dict.get("region", job_dict.get("backtest_config", {}).get("region", "us"))
        parameters = job_dict.get("parameters", job_dict.get("parameter_set", {}))

        # Import the lightweight signal generator bundled with the sidecar.
        # Falls back gracefully if the module isn't present (older installs).
        try:
            from .signal_engine import generate_signals  # type: ignore[import-not-found]
        except ImportError:
            return {
                "job_id": job_id,
                "status": "failed",
                "error": "signal_engine module not available on this worker",
            }

        signals = generate_signals(
            strategy_id=strategy_id,
            symbols=symbols,
            region=region,
            parameters=parameters,
            cache_dir=cache_dir,
        )

        return {
            "job_id": job_id,
            "status": "completed",
            "metrics": {
                "strategy_id": strategy_id,
                "symbols_processed": len(symbols),
                "signals_generated": len(signals),
            },
            "signals": signals,
        }
    except Exception as e:
        return {
            "job_id": job_dict.get("job_id", "unknown"),
            "status": "failed",
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
        }


# Registry mapping job_type -> executor function
_JOB_EXECUTORS: Dict[str, Callable] = {
    "research_backtest": run_single_research_job,
    "strategy_backtest": _run_strategy_backtest,
    "signal_gen": _run_signal_gen,
}


class StandaloneWorker:
    """SETI@home-style research worker that pulls jobs over HTTPS."""

    def __init__(self, config: WorkerConfig):
        self.config = config
        config.ensure_dirs()

        self.client = CoordinatorClient(
            coordinator_url=config.coordinator_url,
            token=config.token,
            worker_id=config.worker_id,
        )
        self.fetcher = DataFetcher(client=self.client, cache_dir=config.cache_dir)

        self._shutdown = Event()
        self._active_job_ids: List[str] = []
        self._heartbeat_thread: Optional[Thread] = None

        # Stats
        self.stats = {
            "completed": 0,
            "failed": 0,
            "started_at": 0.0,
            "total_job_seconds": 0.0,
        }

    # ── Capabilities ──────────────────────────────────────────────────

    def _capabilities(self) -> Dict[str, Any]:
        """Build capabilities dict for registration."""
        return {
            "hostname": platform.node(),
            "cpu_count": self.config.cpu_count,
            "ram_gb": self.config.ram_gb,
            "max_parallel": self.config.max_parallel,
            "os_info": f"{platform.system()} {platform.release()}",
            "supported_job_types": self.config.supported_job_types,
            "python_version": platform.python_version(),
            "worker_version": "2.0.0",
        }

    def _throughput(self) -> float:
        """Jobs per minute since start."""
        elapsed = time.time() - self.stats["started_at"]
        if elapsed < 10:
            return 0.0
        return self.stats["completed"] / (elapsed / 60.0)

    # ── Heartbeat ─────────────────────────────────────────────────────

    def _heartbeat_loop(self) -> None:
        """Background daemon thread: sends heartbeats for active jobs."""
        while not self._shutdown.is_set():
            try:
                if self._active_job_ids:
                    self.client.heartbeat(list(self._active_job_ids))
            except Exception as e:
                log.debug("Heartbeat error: %s", e)
            self._shutdown.wait(timeout=self.config.heartbeat_interval)

    def _start_heartbeat(self) -> None:
        """Start the heartbeat daemon thread."""
        self._heartbeat_thread = Thread(
            target=self._heartbeat_loop, daemon=True, name="heartbeat"
        )
        self._heartbeat_thread.start()

    # ── Batch Execution ───────────────────────────────────────────────

    def _prefetch_data(self, jobs: List[Dict]) -> None:
        """Download any missing OHLCV data before running backtests."""
        # Collect all (region, symbols) pairs
        region_symbols: Dict[str, List[str]] = {}
        for job in jobs:
            config = job.get("backtest_config", {})
            region = config.get("region", "us")
            symbols = job.get("symbol_universe", [])
            if region not in region_symbols:
                region_symbols[region] = []
            region_symbols[region].extend(symbols)

        # Deduplicate and fetch
        for region, syms in region_symbols.items():
            unique_syms = list(dict.fromkeys(syms))  # preserve order, dedupe
            self.fetcher.ensure_data(unique_syms, region)

    def _resolve_executor(self, job: Dict) -> Callable:
        """Resolve the executor function for a job based on its type."""
        job_type = job.get("job_type", "research_backtest")
        executor = _JOB_EXECUTORS.get(job_type)
        if executor is None:
            log.warning("Unknown job_type %r, falling back to research_backtest", job_type)
            executor = run_single_research_job
        return executor

    def _execute_batch(self, jobs: List[Dict]) -> List[Dict[str, Any]]:
        """Prefetch data, then run jobs in parallel via ProcessPoolExecutor.

        Dispatches each job to the appropriate executor based on its job_type.
        """
        # Prefetch all needed data
        self._prefetch_data(jobs)

        # Track active jobs for heartbeats
        self._active_job_ids = [j["job_id"] for j in jobs]

        results: List[Dict[str, Any]] = []
        max_workers = min(self.config.max_parallel, len(jobs))
        cache_dir = self.config.cache_dir

        if max_workers <= 1 or len(jobs) == 1:
            # Sequential execution
            for job in jobs:
                start = time.time()
                executor_fn = self._resolve_executor(job)
                result = executor_fn(job, cache_dir)
                result["execution_time"] = round(time.time() - start, 2)
                result["worker_id"] = self.config.worker_id
                results.append(result)
            self._active_job_ids = []
            return results

        # Parallel execution
        try:
            with ProcessPoolExecutor(max_workers=max_workers) as executor:
                future_to_job = {}
                for job in jobs:
                    executor_fn = self._resolve_executor(job)
                    fut = executor.submit(executor_fn, job, cache_dir)
                    future_to_job[fut] = job["job_id"]

                timeout = self.config.job_timeout * len(jobs)
                for future in as_completed(future_to_job, timeout=timeout):
                    job_id = future_to_job[future]
                    try:
                        result = future.result(timeout=self.config.job_timeout)
                        result["worker_id"] = self.config.worker_id
                        results.append(result)
                    except FuturesTimeout:
                        results.append({
                            "job_id": job_id,
                            "status": "failed",
                            "error": f"Job timed out after {self.config.job_timeout}s",
                            "worker_id": self.config.worker_id,
                        })
                    except Exception as e:
                        results.append({
                            "job_id": job_id,
                            "status": "failed",
                            "error": str(e),
                            "worker_id": self.config.worker_id,
                        })
        except Exception as e:
            log.error("Batch execution error: %s", e)
            completed_ids = {r["job_id"] for r in results}
            for job in jobs:
                if job["job_id"] not in completed_ids:
                    results.append({
                        "job_id": job["job_id"],
                        "status": "failed",
                        "error": f"Batch error: {e}",
                        "worker_id": self.config.worker_id,
                    })
        finally:
            self._active_job_ids = []

        return results

    # ── Result Reporting ──────────────────────────────────────────────

    def _report_results(self, results: List[Dict]) -> None:
        """Report completed/failed jobs back to the compute grid."""
        for result in results:
            job_id = result.get("job_id", "unknown")
            try:
                if result.get("status") == "completed":
                    # Build the result payload from metrics + any extra data
                    result_payload: Dict[str, Any] = {}
                    if result.get("metrics"):
                        result_payload["metrics"] = result["metrics"]
                    if result.get("signals"):
                        result_payload["signals"] = result["signals"]
                    compute_seconds = result.get("execution_time", 0.0)
                    self.client.complete(job_id, result_payload, compute_seconds)
                    self.stats["completed"] += 1
                    self.stats["total_job_seconds"] += compute_seconds
                else:
                    self.client.fail(job_id, result.get("error", "unknown error"))
                    self.stats["failed"] += 1
            except Exception as e:
                log.error("Failed to report result for job %s: %s", job_id, e)
                self.stats["failed"] += 1

    # ── Main Loop ─────────────────────────────────────────────────────

    def run(self) -> None:
        """Main worker loop: register -> dequeue -> execute -> report -> repeat."""
        self.stats["started_at"] = time.time()

        log.info("=" * 70)
        log.info("Aura Alpha Standalone Worker starting: %s", self.config.worker_id)
        log.info(
            "Coordinator: %s | CPUs: %d | RAM: %.1fGB | Parallel: %d | Batch: %d",
            self.config.coordinator_url,
            self.config.cpu_count,
            self.config.ram_gb,
            self.config.max_parallel,
            self.config.batch_size,
        )
        log.info("Job types: %s", ", ".join(self.config.supported_job_types))
        log.info("Cache: %s", self.config.cache_dir)
        log.info("=" * 70)

        # Graceful shutdown handlers
        def _signal_handler(signum, frame):
            log.info("Received signal %d, shutting down gracefully...", signum)
            self._shutdown.set()

        signal.signal(signal.SIGINT, _signal_handler)
        signal.signal(signal.SIGTERM, _signal_handler)

        # Register with coordinator
        try:
            resp = self.client.register(self._capabilities())
            log.info("Registered with coordinator: %s", resp.get("message", "ok"))
        except Exception as e:
            log.error("Failed to register with coordinator: %s", e)
            log.error("Check your --token and --coordinator-url settings.")
            return

        # Start heartbeat thread
        self._start_heartbeat()

        # Main dequeue-execute loop with exponential backoff on idle
        idle_backoff = 1  # seconds
        max_backoff = 30

        while not self._shutdown.is_set():
            try:
                # Dequeue a batch, filtered to this worker's supported types
                jobs = self.client.dequeue(
                    count=self.config.batch_size,
                    job_types=self.config.supported_job_types,
                )

                if not jobs:
                    # No work available — back off
                    log.debug("No jobs available, sleeping %ds", idle_backoff)
                    self._shutdown.wait(timeout=idle_backoff)
                    idle_backoff = min(idle_backoff * 2, max_backoff)
                    continue

                # Reset backoff on successful dequeue
                idle_backoff = 1
                log.info("Dequeued %d jobs", len(jobs))

                # Execute batch
                batch_start = time.time()
                results = self._execute_batch(jobs)
                batch_elapsed = time.time() - batch_start

                # Report results
                self._report_results(results)

                completed = sum(1 for r in results if r.get("status") == "completed")
                failed = len(results) - completed
                log.info(
                    "Batch done: %d completed, %d failed in %.1fs (%.1f jobs/min total)",
                    completed, failed, batch_elapsed, self._throughput(),
                )

            except KeyboardInterrupt:
                log.info("Worker interrupted.")
                break
            except Exception as e:
                log.error("Worker loop error: %s", e)
                log.debug(traceback.format_exc())
                self._shutdown.wait(timeout=5)

        # Shutdown
        self._shutdown.set()
        log.info("=" * 70)
        log.info(
            "Worker %s stopped. Completed: %d | Failed: %d | Throughput: %.1f/min",
            self.config.worker_id,
            self.stats["completed"],
            self.stats["failed"],
            self._throughput(),
        )
        log.info("=" * 70)
