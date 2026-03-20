# Aura Alpha Grid Worker

Distributed compute node for the Aura Alpha research grid. Connects to the
coordinator at `auraalpha.cc`, pulls research/backtest jobs, runs them locally,
and reports results back. Think SETI@home for trading research.

## Quick Start (Windows)

1. Install [Python 3.10+](https://www.python.org/downloads/) (check "Add to PATH")
2. Double-click `setup.bat`
3. Double-click `run.bat`

That's it. The worker auto-provisions a token on first run.

## Quick Start (macOS / Linux)

```bash
chmod +x setup.sh run.sh
./setup.sh
./run.sh
```

## Configuration

Edit `.env` to customize (created by setup from `.env.template`):

| Variable            | Default                  | Description                      |
|---------------------|--------------------------|----------------------------------|
| `COORDINATOR_URL`   | `https://auraalpha.cc`   | Coordinator hub URL              |
| `MAX_PARALLEL`      | `2`                      | Max concurrent backtest jobs     |
| `GRID_WORKER_TOKEN` | *(auto-provisioned)*     | Override token (optional)        |

Or pass as CLI flags:

```bash
python worker.py --max-parallel 4 --verbose
```

## How It Works

- Worker registers with the coordinator and receives a unique token (stored in `~/.aura-worker/`)
- Polls for `research_backtest` jobs (parameter-driven, no strategy code on the worker)
- Downloads OHLCV market data from the coordinator as needed (cached locally)
- Runs backtests using parameters provided by each job
- Reports metrics (Sharpe, win rate, drawdown, etc.) back to the coordinator
- Adaptive throttle automatically yields CPU to other apps (games, browsers, etc.)

## Requirements

- Python 3.10+
- `numpy` and `polars` (installed by setup script)
- Internet connection to reach `auraalpha.cc`
- Optional: `psutil` for better adaptive throttle (falls back to OS-level detection)

## What This Package Does NOT Include

- No strategy source code or signal generation logic
- No API keys or secrets
- No proprietary trading algorithms
- No database files

The worker is a generic compute engine. All job parameters come from the
coordinator; the worker returns computed metrics.
