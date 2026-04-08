# Aura Alpha Desktop — Production Coordination

Cross-agent coordination document for the Aura Alpha Desktop application.
Last updated: 2026-04-08.

---

## Current State

- **Repo**: github.com/sgallup23/AuraAlphaDesktop (master branch)
- **Tauri source version**: v7.4.0 (Cargo.toml + tauri.conf.json + package.json all aligned)
- **Installed Electron app**: v8.2.0 (separate codebase, installed on Shane's desktop)
- **Important**: The INSTALLED desktop app is Electron. This repo contains the Tauri rewrite.
- **Electron app.asar was hot-patched** on Shane's desktop (API_BASE fix, loadURL fix, API proxy simplification)
- **Identifier**: `cc.auraalpha.desktop`
- **Bundle targets**: Windows (NSIS + WiX), macOS (universal binary, min 10.15), Linux (deb + AppImage)
- **Auto-updater**: configured via Tauri plugin-updater, endpoint at `auraalpha.cc/api/desktop/update/`

---

## Compute Engine (Rust — src-tauri/src/compute/)

Pure Rust computation engine. Zero Python dependencies. ~7,600 lines across 15 modules.

| Module          | Lines | Purpose |
|-----------------|------:|---------|
| `gpu.rs`        | 1,725 | GPU-accelerated compute via wgpu (Vulkan/Metal/DX12). 7 WGSL shaders. |
| `ml_train.rs`   | 1,206 | Local ML training via smartcore Random Forest. Walk-forward CV. Zero Python. |
| `backtest.rs`   |   819 | Full backtest engine with 25+ entry conditions, ATR-based exits. |
| `cache.rs`      |   587 | Offline data cache manager (seed, stats, download, eviction). |
| `data.rs`       |   556 | OHLCV data loading from Parquet/CSV via Polars. |
| `indicators.rs` |   490 | RSI, EMA, ATR, SMA, Bollinger Bands, OBV. Auto-routes to GPU when available. |
| `types.rs`      |   357 | All shared data types (BacktestResult, ScanSignal, OhlcvBars, etc). |
| `scanner.rs`    |   316 | Signal scanner (momentum, reversal, breakout, volume). |
| `demo.rs`       |   312 | Demo/test data generation. |
| `hardware.rs`   |   290 | CPU/RAM/GPU detection (cross-platform). |
| `metrics.rs`    |   238 | Sharpe, Sortino, profit factor, max drawdown. |
| `ml.rs`         |   204 | Hand-coded ML ensemble inference. Batched DenseMatrix prediction. |
| `features.rs`   |   143 | Technical feature extraction for ML pipeline (10 features per symbol). |
| `gpu_stub.rs`   |   118 | No-op GPU stub when `gpu` feature is off. |
| `mod.rs`        |    55 | Module declarations and re-exports. |

---

## GPU Shaders (wgpu — WGSL)

All shaders are WGSL compute shaders using `@workgroup_size(256)`. They operate on `f32` arrays (WGSL does not support f64 without extensions; f32 precision is sufficient for price data with ~7 significant digits).

| Shader Constant      | Line | Purpose |
|-----------------------|-----:|---------|
| `SHADER_SMA`         |   25 | Simple Moving Average — sums window, divides by period. |
| `SHADER_EMA`         |   68 | Exponential Moving Average — recursive smoothing. |
| `SHADER_RSI`         |  123 | Relative Strength Index — gain/loss separation, ratio. |
| `SHADER_ATR`         |  190 | Average True Range — true range calculation + smoothing. |
| `SHADER_BATCH_RSI`   |  258 | Batch RSI — processes multiple symbols in one GPU dispatch. |
| `SHADER_BATCH_ATR`   |  333 | Batch ATR — processes multiple symbols in one GPU dispatch. |
| `SHADER_BATCH_SMA`   |  412 | Batch SMA — processes multiple symbols in one GPU dispatch. |

- **Threshold**: Auto-routes to GPU when data exceeds `GPU_THRESHOLD = 100` bars (defined in `indicators.rs`).
- **Fallback**: CPU path is always available. When GPU feature is disabled or no adapter found, all functions gracefully fall back.
- **GPU feature**: Opt-in via `--features gpu` in Cargo build. Default build is CPU-only.
- **Backends**: Vulkan (Linux/Windows), DX12 (Windows), Metal (macOS). No CUDA dependency.
- **GpuContext**: Initialized once (lazy via `OnceLock`/`OnceCell`), cached for process lifetime.
- **Indicators auto-routing**: `compute_sma_auto()`, `compute_ema_auto()`, `compute_rsi_auto()`, `compute_atr_auto()` all check `GPU_THRESHOLD` + `is_gpu_available()` before dispatching.

---

## Worker Architecture

Worker modules live in `src-tauri/src/worker/` (~2,063 lines across 4 files).

| Module             | Lines | Purpose |
|--------------------|------:|---------|
| `grid_worker.rs`   |   757 | Main worker loop: authenticate, dequeue, execute, heartbeat (30s). |
| `job_executor.rs`  |   617 | Job dispatch — routes job types to Rust compute engine functions. |
| `redis_worker.rs`  |   504 | Redis-backed dispatch queue (feeder/worker/reporter architecture). |
| `mod.rs`           |   185 | IPC commands: `start_grid_worker`, `stop_grid_worker`, `grid_worker_status`. Managed state via Tokio Mutex/RwLock/Notify. |

### Grid Worker (Rust — Tauri)
- Redis dispatch: dequeue jobs via ZPOPMIN from Redis sorted set, execute via Rust compute engine, report results via POST batch endpoint.
- Tokio-based: async runtime, graceful shutdown via `tokio::sync::Notify`.
- Heartbeat every 30 seconds — extends Redis lease TTL for in-flight jobs.
- Hardware-aware concurrency: reads CPU/RAM from `hardware` module.
- Fallback: if Redis is unavailable, falls back to HTTP API dequeue (`/api/grid/dequeue`).
- Result batching: reporter sends results in batches of 50 via `/api/grid/complete-batch`.

### Electron Worker (Python — Legacy)
- `worker.py` + `compute_worker.py` in resources directory.
- Coordinator API: polls for jobs, executes via Python, reports results.
- Dependencies: numpy, polars, psutil, requests, pyyaml.
- Used by the installed Electron v8.2.0 app on Shane's desktop.

### Job Types

| Job Type             | Executor | Timeout | Notes |
|----------------------|----------|--------:|-------|
| `backtest`           | Rust compute (rayon parallel over symbols) | 300s | 25+ entry conditions, ATR exits |
| `scan`               | Rust compute (rayon parallel over symbols) | 300s | Momentum, reversal, breakout, volume |
| `feature_extraction` | Rust compute (rayon parallel) | 300s | 10 technical features per symbol |
| `ml_train`           | Rust compute (smartcore RF) | 600s | Max 2 concurrent (semaphore), 32 MB stack |
| `ml_inference`       | Rust compute (cached model) | 300s | Batched DenseMatrix prediction |
| `health_check`       | Pure Rust (no subprocess) | N/A | Lightweight |
| `ping`               | Pure Rust (no subprocess) | N/A | Lightweight |

---

## Optimizations Applied (2026-04-08)

### Round 1 — Frontend

1. **Visibility-based polling pause** — `useVisibility()` hook. Applied to 4 polling hooks (`useAlerts`, `useLiveBots`, `usePositions`, `useLocalBots`). Zero network requests when tab is hidden.
2. **Console cleanup** — 33 console statements across 18 files gated behind `import.meta.env.DEV`. Terser `drop_console: true` strips any remaining in production.
3. **Lazy loading** — 4 pages (`LoginPage`, `StartupPage`, `ExplorerPage`, `WorkspaceShell`) + all 17 panels via `React.lazy()`. Shimmer `PanelLoader.jsx` as Suspense fallback.
4. **Cargo deps trimmed** — tokio reduced to 6 features, GPU default off, polars features minimized.
5. **React memoization** — 8 components: `ExplorerPage`, `BotActivityFeed`, `CommandPalette`, `DataTable`, `PortfolioBrainPanel`, `RegimePanel`, `GridComputePanel`, `PositionsPanel`. Uses `React.memo`, `useMemo`, `useCallback`.
6. **Vite terser + CSP tightened** — 2-pass terser compression, `drop_console`/`drop_debugger`, dead code elimination. CSP: removed `unsafe-eval` from `script-src`.

### Round 2 — Intelligence Engine

7. **GPU shaders expanded** — RSI, ATR, Batch SMA, Batch RSI, Batch ATR shaders added (7 total, up from 3).
8. **Rayon parallelism** — Applied across 6 compute modules: `backtest`, `features`, `indicators`, `ml`, `ml_train`, `scanner`. Work-stealing across all cores.
9. **Worker throughput** — Hardware-aware concurrency, batch job fetching from Redis, result batching (50 per POST).
10. **ML pipeline** — Flat `Vec<f64>` matrices for cache-friendly layout, parallel walk-forward folds, scaler parameter caching.
11. **Data pipeline** — Concurrent downloads via tokio, LRU cache (`lru` crate), incremental updates (skip unchanged symbols).

### Round 3 — Deep Optimization (in progress)

12. **Backtest engine** — Parameter sweep parallelism via rayon, indicator pre-computation to avoid redundant calculation.
13. **Scanner** — Batch GPU dispatch for multi-symbol scans, early termination on low confidence signals.
14. **Build profile** — `opt-level=3`, `target-cpu=native`, fat LTO, PGO instructions documented in Cargo.toml.
15. **Feature extraction** — SIMD-friendly contiguous memory layout, loop fusion across indicator computations.
16. **Electron main.js** — Simplified API proxy (removed dead routes), worker reliability improvements.

---

## Build Profile (Cargo)

```toml
[profile.release]
opt-level = 3        # maximum speed
lto = "fat"          # full cross-crate LTO: dead code elimination across all crates
codegen-units = 1    # single codegen unit: better inlining + optimization at cost of compile time
strip = true         # strip debug symbols from binary
panic = "abort"      # no unwind tables: smaller binary + faster panics

[profile.dev]
opt-level = 1        # slightly optimized dev builds so rayon/polars aren't painfully slow
```

PGO (Profile-Guided Optimization) steps documented in Cargo.toml:
1. Build instrumented: `RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release`
2. Run representative workload (backtest 200 symbols, full scan, ML inference).
3. Merge profiles: `llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data`
4. Build optimized: `RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release`

PGO typically yields 10-20% additional speedup on hot paths (indicator loops, backtest sim).

---

## Electron Hot-Patches Applied (Shane's Desktop)

These changes were applied directly to the installed Electron v8.2.0 `app.asar`:

- `API_BASE_DIRECT` changed from `http://54.172.235.137:8020` to `https://auraalpha.cc`
- `loadURL` changed from `aura://app/index.html` to `aura://app/`
- API proxy simplified in `main.js` (removed dead routes)
- `worker.py` + `compute_worker.py` copied to `resources/resources/grid_worker/`
- Python deps installed on desktop: numpy, polars, psutil, requests, pyyaml

---

## Frontend Stack

| Dependency             | Version  | Purpose |
|------------------------|----------|---------|
| React                  | 19.2.4   | UI framework |
| react-router-dom       | 7.13.1   | Client-side routing |
| flexlayout-react       | 0.8.19   | Dockable panel layout (replaced rc-dock at v7.0.0) |
| lightweight-charts     | 5.1.0    | TradingView charting |
| framer-motion          | 12.35.2  | Animations |
| tailwindcss            | 3.4.19   | Utility CSS |
| Vite                   | 7.3.1    | Build tool (dev server on port 1420) |
| vite-plugin-singlefile | 2.3.2    | Bundle everything into one HTML file for Tauri |
| terser                 | 5.46.1   | JS minification (2-pass, dead code elim) |

Vite config: `viteSingleFile` plugin inlines all assets. Build target: Chrome 105 (Windows) / Safari 13 (macOS). All assets inlined (100 MB inline limit). Single chunk output (no code splitting — Tauri loads one file).

---

## Tauri Plugins

| Plugin                       | Purpose |
|------------------------------|---------|
| tauri-plugin-shell           | Spawn child processes (sidecars) |
| tauri-plugin-notification    | Desktop notifications |
| tauri-plugin-updater         | Auto-update from auraalpha.cc endpoint |
| tauri-plugin-window-state    | Persist window size/position across sessions |
| tauri-plugin-process         | Process info (PID, exit) |
| tauri-plugin-opener          | Open URLs in default browser |
| tauri-plugin-store           | Persistent key-value storage |

---

## Key Rust Dependencies

| Crate     | Version | Purpose |
|-----------|---------|---------|
| tauri     | 2       | Desktop framework (tray-icon feature) |
| tokio     | 1       | Async runtime (rt-multi-thread, sync, time, macros, process, signal) |
| rayon     | 1.10    | Data-parallel CPU computation |
| polars    | 0.46    | DataFrame engine (parquet, csv, lazy) |
| smartcore | 0.4     | ML (Random Forest training/inference, serde) |
| wgpu      | 29      | GPU compute (optional: wgsl, vulkan, dx12, metal) |
| redis     | 0.27    | Job queue (tokio-comp) |
| reqwest   | 0.12    | HTTP client (json, native-tls) |
| aes-gcm   | 0.10    | AES-256-GCM credential encryption |
| keyring   | 3       | OS keychain access |
| sysinfo   | 0.32    | Hardware detection |
| lru       | 0.12    | LRU cache for data pipeline |
| bytemuck  | 1       | Safe transmute for GPU buffer uploads (derive) |
| uuid      | 1       | UUID v4 generation for worker/job IDs |
| chrono    | 0.4     | Date/time with serde |

---

## Production Recommendations

### Build Commands
```bash
# CPU-only (smaller binary, no GPU dependency)
cargo build --release

# GPU-enabled (Vulkan/DX12/Metal compute)
cargo build --release --features gpu

# Maximum SIMD on known hardware
RUSTFLAGS="-C target-cpu=native" cargo build --release --features gpu

# Full Tauri app bundle (platform-specific)
npm run tauri:build                # auto-detect
npm run tauri:build:windows        # x86_64-pc-windows-msvc
npm run tauri:build:macos          # universal-apple-darwin
npm run tauri:build:linux          # x86_64-unknown-linux-gnu
```

### Networking
- EC2 port 8020 is NOT publicly accessible -- always use `https://auraalpha.cc` domain.
- CSP restricts connections to `*.auraalpha.cc` and `wss://*.auraalpha.cc`.
- Cloudflare sits in front -- User-Agent must be wrapped in Mozilla/5.0 to avoid bot detection.

### Worker Deployment
- Electron needs `worker.py` bundled in `resources/resources/grid_worker/`.
- Tauri bundles sidecar and resources via `tauri.conf.json` bundle.resources (`sidecar/**/*`, `resources/**/*`).
- Redis worker requires Redis connection string in app config.

---

## Known Issues

1. **Electron v8.2.0 and Tauri v7.4.0 are separate codebases** -- the installed app (Electron) and this repo (Tauri) need reconciliation. The Electron app was hot-patched manually.
2. **GPU feature is opt-in** (`--features gpu`) -- default build is CPU-only. Must explicitly enable for GPU-accelerated indicators.
3. **Python 3.14 on Shane's desktop** may have compatibility issues with some packages (numpy, polars wheels for 3.14 are bleeding-edge).
4. **Auto-updater endpoint** (`/api/desktop/update/{target}/{arch}/{current_version}`) is configured but the server-side handler has not been built yet.
5. **Code signing**: SSL.com certificate integrated in CI (GitHub Actions CodeSignTool), validation still in progress.
6. **Singlefile build**: `vite-plugin-singlefile` inlines everything into one HTML -- very large apps may hit memory limits on low-end devices.
7. **GPU EMA performance**: The single-workgroup EMA shader is often slower than CPU for individual calls. Only beneficial in batch pipeline context.
8. **Telemetry consent**: Dialog exists (`telemetry_consent.rs`, 97 lines) but telemetry collection backend is not implemented.

---

## Architecture Relationships

- **This repo** (AuraAlphaDesktop): Tauri v2, Rust compute engine, React frontend. Canonical desktop app.
- **AuraCommandV2** (separate repo): Web frontend (Vite + React). Shares no code with desktop.
- **Electron wrapper** (not in this repo): Legacy quick-ship wrapper. Uses the web build, not this codebase.
- **prodesk** (separate repo): FastAPI backend, 3 trading bots (SHAWN/SHANE/Nova), backtests. Desktop connects to this API via `https://auraalpha.cc`.

---

## GitHub

- **Repo**: github.com/sgallup23/AuraAlphaDesktop
- **Branch**: master (single branch workflow)
- **Last push**: 2026-04-08 (intelligence engine optimization)
- **CI**: GitHub Actions -- builds Windows/macOS/Linux on `v*` tags
- **Code signing**: SSL.com CodeSignTool integrated in Windows CI workflow

### Recent Commit History (last 25)

```
f4d481d perf: intelligence engine optimization — GPU shaders, rayon parallelism, worker throughput, ML pipeline, data cache
c68268d perf: full desktop optimization — polling, memoization, lazy loading, Cargo, Vite, CSP
851ab02 fix: ML train semaphore (max 2 concurrent) + 32MB stack threads + feeder capacity fix
fa9f3d6 feat: wire ml_train + walk_forward + GPU threshold 100
7986438 release: v7.4.0 — Redis dispatch queue + headless grid worker
6cbcca2 feat: Redis-backed grid worker — feeder/worker/reporter architecture
77d7972 feat: headless Rust grid worker — pure compute, no GTK/Tauri UI
104a7cf fix: Cloudflare bot protection — wrap User-Agent in Mozilla/5.0
7a69c93 security: replace XOR fallback with AES-256-GCM for credential encryption
d7c3691 security: remove shell:allow-execute + add CSP to webview
f55d7c8 chore: mark as deprecated — consolidated into AuraCommandV2
9075d00 release: v7.3.0 — bump version for strategy routing release
231881a feat: v7.3.0 — strategy routing panel + execution wiring
449d71f fix: use official SSLcom CodeSignTool v1.3.2 + java -jar directly
daea707 fix: call CodeSignTool jar directly via Java, bypass bat file
d8142c4 fix: use env vars for CodeSignTool to avoid quoting issues
6a4e0d2 fix: cd into CodeSignTool dir before signing (path resolution)
b0047be fix: add Java setup for Windows code signing in CI
774e8dc fix: replace grep -P with sed for Windows CI compat
5ac50b5 fix: v7.2.1 — grid worker sidecar crash on --verbose flag
1e47880 fix: v7.2.0 — always send real hostname and hardware in grid worker
002070b release: v7.1.0 — WebView to auraalpha.cc with cloud-first connection
96659f8 style: proper Aura Alpha dark theme for flexlayout panels
4a508f8 release: v7.0.0 — replace rc-dock with flexlayout-react
ce6d4d2 release: v6.3.2 — fix workspace layout crash (remove group field, add groups prop)
```

---

## Coordination Rules

- This file (`cooperative.md`) is the coordination reference for desktop app state.
- Any agent modifying the desktop app should update this file with what changed.
- Check git log before starting work -- another agent may have pushed.
- Never run `git add -A` -- use specific file paths.
- Test builds locally before pushing to CI (20-minute build cycles are expensive).
- Do not deploy Aura to EC2 during trading hours (9:30 AM - 4:00 PM ET).
