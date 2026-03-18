"""
StandaloneWorker — main worker loop.
Connects to coordinator via HTTPS, pulls jobs, executes backtests in parallel,
reports results. No Redis or shared filesystem needed.
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
from typing import Any, Dict, List, Optional

from .api_client import CoordinatorClient
from .backtest_engine import run_single_research_job
from .config import WorkerConfig
from .data_fetcher import DataFetcher
from .job_router import route_job

log = logging.getLogger("standalone.worker")


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
            "cpu_count": self.config.cpu_count,
            "ram_gb": self.config.ram_gb,
            "max_parallel": self.config.max_parallel,
            "os": f"{platform.system()} {platform.release()}",
            "python_version": platform.python_version(),
            "worker_version": "1.0.0",
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

    def _execute_batch(self, jobs: List[Dict]) -> List[Dict[str, Any]]:
        """Prefetch data, then run backtests in parallel via ProcessPoolExecutor."""
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
                result = route_job(job, cache_dir)
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
                    fut = executor.submit(route_job, job, cache_dir)
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
        """Report completed/failed jobs back to the coordinator."""
        for result in results:
            job_id = result.get("job_id", "unknown")
            try:
                if result.get("status") == "completed":
                    self.client.complete(job_id, result.get("metrics", {}))
                    self.stats["completed"] += 1
                    self.stats["total_job_seconds"] += result.get("execution_time", 0)
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
                # Dequeue a batch
                jobs = self.client.dequeue(count=self.config.batch_size)

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
