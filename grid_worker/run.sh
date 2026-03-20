#!/usr/bin/env bash
# ============================================================
#  Aura Alpha Grid Worker -- macOS/Linux Runner
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# -- Check venv exists --------------------------------------------
if [ ! -f "venv/bin/activate" ]; then
    echo "[ERROR] Virtual environment not found. Run ./setup.sh first."
    exit 1
fi

# -- Activate venv -------------------------------------------------
source venv/bin/activate

# -- Load .env into environment ------------------------------------
if [ -f ".env" ]; then
    set -a
    source <(grep -v '^\s*#' .env | grep -v '^\s*$')
    set +a
fi

# -- Build args from .env values -----------------------------------
ARGS=""
if [ -n "$COORDINATOR_URL" ]; then
    ARGS="$ARGS --coordinator-url $COORDINATOR_URL"
fi
if [ -n "$MAX_PARALLEL" ]; then
    ARGS="$ARGS --max-parallel $MAX_PARALLEL"
fi
if [ -n "$GRID_WORKER_TOKEN" ]; then
    ARGS="$ARGS --token $GRID_WORKER_TOKEN"
fi

echo ""
echo "  Starting Aura Alpha Grid Worker..."
echo "  Press Ctrl+C to stop."
echo ""

python worker.py $ARGS "$@"
