"""
HTTP client for the coordinator API at /api/cluster/contributor/*.
All calls include contributor token and worker ID headers.
Retry logic with exponential backoff.
"""
from __future__ import annotations

import logging
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

import requests

log = logging.getLogger("standalone.api_client")

# Retry settings
MAX_RETRIES = 3
BACKOFF_BASE = 1  # seconds: 1, 2, 4


class CoordinatorClient:
    """Thin HTTP wrapper around the coordinator contributor API."""

    def __init__(self, coordinator_url: str, token: str, worker_id: str):
        self.base_url = coordinator_url.rstrip("/")
        self.token = token
        self.worker_id = worker_id
        self.session = requests.Session()
        self.session.headers.update({
            "X-Contributor-Token": self.token,
            "X-Worker-Id": self.worker_id,
            "Content-Type": "application/json",
        })
        # Reasonable timeouts: (connect, read)
        self.timeout = (10, 60)

    # ── Internal helpers ───────────────────────────────────────────────

    def _url(self, path: str) -> str:
        return f"{self.base_url}/api/cluster/contributor/{path.lstrip('/')}"

    def _request(
        self,
        method: str,
        path: str,
        json: Optional[Dict] = None,
        params: Optional[Dict] = None,
        stream: bool = False,
        timeout: Optional[tuple] = None,
    ) -> requests.Response:
        """Make an HTTP request with retry logic and exponential backoff."""
        url = self._url(path)
        last_exc: Optional[Exception] = None

        for attempt in range(MAX_RETRIES):
            try:
                resp = self.session.request(
                    method=method,
                    url=url,
                    json=json,
                    params=params,
                    stream=stream,
                    timeout=timeout or self.timeout,
                )
                resp.raise_for_status()
                return resp
            except requests.exceptions.HTTPError as e:
                # Don't retry on 4xx client errors (except 429)
                if e.response is not None and 400 <= e.response.status_code < 500:
                    if e.response.status_code != 429:
                        raise
                last_exc = e
            except (requests.exceptions.ConnectionError,
                    requests.exceptions.Timeout,
                    requests.exceptions.ChunkedEncodingError) as e:
                last_exc = e

            # Exponential backoff
            if attempt < MAX_RETRIES - 1:
                wait = BACKOFF_BASE * (2 ** attempt)
                log.warning(
                    "Request %s %s failed (attempt %d/%d), retrying in %ds: %s",
                    method, path, attempt + 1, MAX_RETRIES, wait, last_exc,
                )
                time.sleep(wait)

        raise ConnectionError(
            f"Failed after {MAX_RETRIES} retries: {method} {path} — {last_exc}"
        )

    # ── Public API methods ─────────────────────────────────────────────

    def ping(self) -> Dict[str, Any]:
        """Simple connectivity check."""
        resp = self._request("GET", "ping")
        return resp.json()

    def register(self, capabilities: Dict[str, Any]) -> Dict[str, Any]:
        """Register this worker with the coordinator.

        capabilities should include: hostname, cpus, ram_gb, os, max_parallel
        """
        resp = self._request("POST", "register", json={
            "hostname": capabilities.get("hostname", self.worker_id),
            "cpus": capabilities.get("cpus", 1),
            "ram_gb": capabilities.get("ram_gb", 0),
            "os": capabilities.get("os", "unknown"),
            "max_parallel": capabilities.get("max_parallel", 1),
        })
        return resp.json()

    def dequeue(self, count: int = 5) -> List[Dict[str, Any]]:
        """Request a batch of jobs from the coordinator.

        Returns list of job dicts, possibly empty if no work available.
        """
        resp = self._request("POST", "dequeue", json={
            "worker_id": self.worker_id,
            "count": count,
        })
        data = resp.json()
        return data.get("jobs", [])

    def complete(self, job_id: str, metrics: Dict[str, Any]) -> Dict[str, Any]:
        """Report a successfully completed job."""
        resp = self._request("POST", "complete", json={
            "job_id": job_id,
            "metrics": metrics,
        })
        return resp.json()

    def fail(self, job_id: str, error: str) -> Dict[str, Any]:
        """Report a failed job."""
        resp = self._request("POST", "fail", json={
            "job_id": job_id,
            "error": error,
        })
        return resp.json()

    def heartbeat(self, job_ids: List[str]) -> Dict[str, Any]:
        """Send heartbeat for active jobs to extend leases."""
        resp = self._request("POST", "heartbeat", json={
            "worker_id": self.worker_id,
            "job_ids": job_ids,
        })
        return resp.json()

    def download_data(self, region: str, symbol: str, dest_path: Path) -> bool:
        """Download a parquet file from the coordinator.

        Streams the response to dest_path. Returns True on success.
        """
        try:
            resp = self._request(
                "GET",
                f"data/{region}/{symbol}",
                stream=True,
                timeout=(10, 300),  # longer read timeout for data downloads
            )
            dest_path.parent.mkdir(parents=True, exist_ok=True)
            with open(dest_path, "wb") as f:
                for chunk in resp.iter_content(chunk_size=65536):
                    if chunk:
                        f.write(chunk)
            return True
        except Exception as e:
            log.warning("Failed to download %s/%s: %s", region, symbol, e)
            return False

    def get_stats(self) -> Dict[str, Any]:
        """Get cluster/worker stats from the coordinator."""
        resp = self._request("GET", "stats")
        return resp.json()
