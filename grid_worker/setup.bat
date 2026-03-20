@echo off
REM ============================================================
REM  Aura Alpha Grid Worker -- Windows Setup
REM ============================================================
setlocal

echo.
echo  ===================================
echo   Aura Alpha Grid Worker Setup
echo  ===================================
echo.

REM -- Check Python is installed -----------------------------------
where python >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Python not found in PATH.
    echo.
    echo   Install Python 3.10+ from https://www.python.org/downloads/
    echo   IMPORTANT: Check "Add Python to PATH" during installation.
    echo.
    pause
    exit /b 1
)

REM -- Check Python version ----------------------------------------
python -c "import sys; exit(0 if sys.version_info >= (3, 10) else 1)" 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] Python 3.10 or newer is required.
    python --version
    echo.
    echo   Download the latest from https://www.python.org/downloads/
    echo.
    pause
    exit /b 1
)

echo [OK] Python found:
python --version
echo.

REM -- Create virtual environment ----------------------------------
if not exist "venv" (
    echo Creating virtual environment...
    python -m venv venv
    if %ERRORLEVEL% NEQ 0 (
        echo [ERROR] Failed to create virtual environment.
        pause
        exit /b 1
    )
    echo [OK] Virtual environment created.
) else (
    echo [OK] Virtual environment already exists.
)
echo.

REM -- Activate and install dependencies ---------------------------
echo Installing dependencies...
call venv\Scripts\activate.bat

pip install --upgrade pip >nul 2>nul
pip install numpy polars
if %ERRORLEVEL% NEQ 0 (
    echo [WARNING] Some dependencies failed to install. Worker may have limited functionality.
) else (
    echo [OK] Dependencies installed.
)

REM -- Optional: psutil for adaptive throttle (not required) -------
pip install psutil >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    echo [OK] psutil installed (adaptive throttle will use it).
) else (
    echo [INFO] psutil not installed -- adaptive throttle will use OS fallbacks.
)

echo.

REM -- Copy .env template if needed --------------------------------
if not exist ".env" (
    if exist ".env.template" (
        copy .env.template .env >nul
        echo [OK] Created .env from template. Edit it to customize settings.
    )
) else (
    echo [OK] .env already exists.
)

echo.
echo  ===================================
echo   Setup complete!
echo  ===================================
echo.
echo   To start the worker, run:
echo     run.bat
echo.
echo   Or manually:
echo     venv\Scripts\activate
echo     python worker.py
echo.
pause
