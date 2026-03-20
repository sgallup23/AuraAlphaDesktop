@echo off
REM ============================================================
REM  Aura Alpha Grid Worker -- Windows Runner
REM ============================================================
setlocal enabledelayedexpansion

REM -- Check venv exists -------------------------------------------
if not exist "venv\Scripts\activate.bat" (
    echo [ERROR] Virtual environment not found. Run setup.bat first.
    pause
    exit /b 1
)

REM -- Activate venv -----------------------------------------------
call venv\Scripts\activate.bat

REM -- Load .env into environment ----------------------------------
if exist ".env" (
    for /f "usebackq tokens=1,* delims==" %%A in (".env") do (
        set "line=%%A"
        if not "!line:~0,1!"=="#" (
            set "%%A=%%B"
        )
    )
)

REM -- Build args from .env values ---------------------------------
set ARGS=
if defined COORDINATOR_URL set ARGS=!ARGS! --coordinator-url !COORDINATOR_URL!
if defined MAX_PARALLEL set ARGS=!ARGS! --max-parallel !MAX_PARALLEL!
if defined GRID_WORKER_TOKEN set ARGS=!ARGS! --token !GRID_WORKER_TOKEN!

REM -- Pass through any extra CLI args -----------------------------
echo.
echo  Starting Aura Alpha Grid Worker...
echo  Press Ctrl+C to stop.
echo.

python worker.py !ARGS! %*
