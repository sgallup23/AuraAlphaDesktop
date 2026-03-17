# ═══════════════════════════════════════════════════════════════════════
# Aura Alpha Research Worker — Windows Installer
# ═══════════════════════════════════════════════════════════════════════
# Run as: powershell -ExecutionPolicy Bypass -File install_worker.ps1
#
# What this does:
#   1. Checks/installs Python 3.11+
#   2. Installs redis dependency
#   3. Creates startup shortcut (runs on login)
#   4. Starts the worker immediately
# ═══════════════════════════════════════════════════════════════════════

$ErrorActionPreference = "Continue"
$WORKER_DIR = "$env:LOCALAPPDATA\AuraAlpha\research-worker"
$REDIS_URL = "redis://:eBKI21LTAh8zRAe0AHMbLmfYO00n4vn_6W-iNcPHRvk@54.172.235.137:6379/0"

Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  Aura Alpha Research Worker Installer" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# ── Step 1: Check Python ──
Write-Host "[1/5] Checking Python..." -ForegroundColor Yellow
$python = $null
foreach ($cmd in @("python", "python3", "py")) {
    try {
        $ver = & $cmd --version 2>&1
        if ($ver -match "Python 3\.(\d+)") {
            $minor = [int]$Matches[1]
            if ($minor -ge 9) {
                $python = $cmd
                Write-Host "  Found: $ver" -ForegroundColor Green
                break
            }
        }
    } catch {}
}

if (-not $python) {
    Write-Host "  Python 3.9+ not found. Installing via winget..." -ForegroundColor Yellow
    winget install Python.Python.3.12 --accept-source-agreements --accept-package-agreements 2>$null
    $python = "python"
    # Refresh PATH
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
}

# ── Step 2: Create worker directory ──
Write-Host "[2/5] Setting up worker directory..." -ForegroundColor Yellow
New-Item -ItemType Directory -Force -Path $WORKER_DIR | Out-Null
New-Item -ItemType Directory -Force -Path "$WORKER_DIR\logs" | Out-Null

# Copy worker script
$scriptSrc = Join-Path $PSScriptRoot "research_worker.py"
if (Test-Path $scriptSrc) {
    Copy-Item $scriptSrc "$WORKER_DIR\research_worker.py" -Force
    Write-Host "  Copied research_worker.py" -ForegroundColor Green
} else {
    Write-Host "  ERROR: research_worker.py not found next to installer" -ForegroundColor Red
    exit 1
}

# ── Step 3: Install Python dependencies ──
Write-Host "[3/5] Installing dependencies..." -ForegroundColor Yellow
& $python -m pip install --quiet --upgrade pip 2>$null
& $python -m pip install --quiet redis psutil 2>$null

# Optional GPU deps (don't fail if unavailable)
Write-Host "  Checking for CUDA GPU..." -ForegroundColor Gray
& $python -c "import torch; print(f'  PyTorch + CUDA: {torch.cuda.is_available()}')" 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Host "  No PyTorch/CUDA (optional — CPU mode is fine)" -ForegroundColor Gray
}
Write-Host "  Dependencies installed" -ForegroundColor Green

# ── Step 4: Create startup shortcut ──
Write-Host "[4/5] Creating startup shortcut..." -ForegroundColor Yellow
$startupDir = [System.Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startupDir "AuraAlphaResearch.lnk"

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = "pythonw"
$shortcut.Arguments = "`"$WORKER_DIR\research_worker.py`" --redis-url `"$REDIS_URL`""
$shortcut.WorkingDirectory = $WORKER_DIR
$shortcut.Description = "Aura Alpha Research Worker"
$shortcut.WindowStyle = 7  # Minimized
$shortcut.Save()
Write-Host "  Auto-start shortcut created" -ForegroundColor Green

# Also create a start/stop batch file on desktop
$desktopDir = [System.Environment]::GetFolderPath("Desktop")

@"
@echo off
echo Starting Aura Alpha Research Worker...
start /min pythonw "$WORKER_DIR\research_worker.py" --redis-url "$REDIS_URL"
echo Worker started in background. Close this window.
timeout /t 3
"@ | Out-File -FilePath "$desktopDir\Start Research Worker.bat" -Encoding ascii

@"
@echo off
echo Stopping Aura Alpha Research Worker...
taskkill /f /im pythonw.exe /fi "WINDOWTITLE eq *research_worker*" 2>nul
taskkill /f /im python.exe /fi "COMMANDLINE eq *research_worker*" 2>nul
wmic process where "commandline like '%%research_worker%%'" call terminate 2>nul
echo Worker stopped.
timeout /t 3
"@ | Out-File -FilePath "$desktopDir\Stop Research Worker.bat" -Encoding ascii

Write-Host "  Desktop shortcuts created" -ForegroundColor Green

# ── Step 5: Start worker now ──
Write-Host "[5/5] Starting worker..." -ForegroundColor Yellow
Write-Host ""

# Test connection first
$testResult = & $python -c "
import redis, json
try:
    r = redis.Redis.from_url('$REDIS_URL', decode_responses=True, socket_timeout=5)
    r.ping()
    print('CONNECTED')
except Exception as e:
    print(f'FAILED: {e}')
" 2>&1

if ($testResult -match "CONNECTED") {
    Write-Host "  Redis connection: OK" -ForegroundColor Green

    # Run hardware detection
    & $python "$WORKER_DIR\research_worker.py" --detect 2>&1 | Write-Host
    Write-Host ""

    # Start in background
    Start-Process -FilePath $python -ArgumentList "`"$WORKER_DIR\research_worker.py`" --redis-url `"$REDIS_URL`"" -WorkingDirectory $WORKER_DIR -WindowStyle Minimized -RedirectStandardOutput "$WORKER_DIR\logs\worker.log" -RedirectStandardError "$WORKER_DIR\logs\worker_err.log"

    Write-Host "═══════════════════════════════════════════════" -ForegroundColor Green
    Write-Host "  Worker RUNNING!" -ForegroundColor Green
    Write-Host "  Logs: $WORKER_DIR\logs\worker.log" -ForegroundColor Gray
    Write-Host "  Auto-starts on login" -ForegroundColor Gray
    Write-Host "  Desktop shortcuts: Start/Stop Research Worker" -ForegroundColor Gray
    Write-Host "═══════════════════════════════════════════════" -ForegroundColor Green
} else {
    Write-Host "  Redis connection FAILED: $testResult" -ForegroundColor Red
    Write-Host "  The worker will retry when Redis is reachable." -ForegroundColor Yellow
    Write-Host "  Check: Is port 6379 open in AWS security group?" -ForegroundColor Yellow

    # Start anyway — it will retry
    Start-Process -FilePath $python -ArgumentList "`"$WORKER_DIR\research_worker.py`" --redis-url `"$REDIS_URL`"" -WorkingDirectory $WORKER_DIR -WindowStyle Minimized
}

Write-Host ""
Write-Host "Installation complete." -ForegroundColor Cyan
