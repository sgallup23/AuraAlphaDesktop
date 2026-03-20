#!/usr/bin/env bash
# ============================================================
#  Aura Alpha Grid Worker -- macOS/Linux Setup
# ============================================================
set -e

echo ""
echo "  ==================================="
echo "   Aura Alpha Grid Worker Setup"
echo "  ==================================="
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# -- Check Python is installed ------------------------------------
if ! command -v python3 &>/dev/null; then
    echo "[ERROR] python3 not found."
    echo ""
    echo "  Install Python 3.10+:"
    echo "    macOS:  brew install python@3.12"
    echo "    Ubuntu: sudo apt install python3 python3-venv python3-pip"
    echo ""
    exit 1
fi

# -- Check Python version -----------------------------------------
if ! python3 -c "import sys; exit(0 if sys.version_info >= (3, 10) else 1)" 2>/dev/null; then
    echo "[ERROR] Python 3.10 or newer is required."
    python3 --version
    exit 1
fi

echo "[OK] Python found: $(python3 --version)"
echo ""

# -- Create virtual environment ------------------------------------
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
    echo "[OK] Virtual environment created."
else
    echo "[OK] Virtual environment already exists."
fi
echo ""

# -- Activate and install dependencies ----------------------------
source venv/bin/activate

pip install --upgrade pip >/dev/null 2>&1
echo "Installing dependencies..."
pip install numpy polars
echo "[OK] Dependencies installed."

# -- Optional: psutil for adaptive throttle -----------------------
if pip install psutil >/dev/null 2>&1; then
    echo "[OK] psutil installed (adaptive throttle will use it)."
else
    echo "[INFO] psutil not installed -- adaptive throttle will use OS fallbacks."
fi

echo ""

# -- Copy .env template if needed ---------------------------------
if [ ! -f ".env" ] && [ -f ".env.template" ]; then
    cp .env.template .env
    echo "[OK] Created .env from template. Edit it to customize settings."
elif [ -f ".env" ]; then
    echo "[OK] .env already exists."
fi

echo ""
echo "  ==================================="
echo "   Setup complete!"
echo "  ==================================="
echo ""
echo "  To start the worker, run:"
echo "    ./run.sh"
echo ""
echo "  Or manually:"
echo "    source venv/bin/activate"
echo "    python worker.py"
echo ""
