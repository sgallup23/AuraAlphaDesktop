"""
CLI entry point: python -m distributed_research.standalone --token X
"""
from __future__ import annotations

import argparse
import logging
import sys

from .config import WorkerConfig
from .worker import StandaloneWorker


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Aura Alpha Standalone Research Worker",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  python -m distributed_research.standalone --token MY_TOKEN
  python -m distributed_research.standalone --token MY_TOKEN --max-parallel 4
  python -m distributed_research.standalone --token MY_TOKEN --coordinator-url https://auraalpha.cc
""",
    )
    parser.add_argument("--token", type=str, default="",
                        help="Contributor authentication token")
    parser.add_argument("--coordinator-url", type=str, default="",
                        help="Coordinator API URL (default: https://auraalpha.cc)")
    parser.add_argument("--worker-id", type=str, default="",
                        help="Custom worker ID (default: auto-generated)")
    parser.add_argument("--max-parallel", type=int, default=0,
                        help="Max parallel backtests (0 = auto-detect from CPU count)")
    parser.add_argument("--batch-size", type=int, default=0,
                        help="Jobs to pull per batch (default: 5)")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Enable debug logging")

    args = parser.parse_args()

    # Logging
    level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(name)s] %(levelname)s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    # Load config (defaults → YAML → env vars → CLI args)
    config = WorkerConfig.load()

    # CLI overrides take highest precedence
    if args.token:
        config.token = args.token
    if args.coordinator_url:
        config.coordinator_url = args.coordinator_url
    if args.worker_id:
        config.worker_id = args.worker_id
    if args.max_parallel > 0:
        config.max_parallel = args.max_parallel
    if args.batch_size > 0:
        config.batch_size = args.batch_size

    # Validate token
    if not config.token:
        print("ERROR: --token is required (or set AURA_TOKEN env var)", file=sys.stderr)
        sys.exit(1)

    worker = StandaloneWorker(config)
    worker.run()


if __name__ == "__main__":
    main()
