# Aura Alpha Desktop -- Coordination Report

Last updated: 2026-04-08
Repo: github.com/sgallup23/AuraAlphaDesktop (branch: master)
Identifier: cc.auraalpha.desktop

---

## Desktop App Status

| Field | Value |
|---|---|
| Version | 7.4.0 |
| Framework | Tauri v2 (Rust + Webview) |
| Bundle targets | Windows (NSIS/WiX), macOS (DMG/app), Linux (deb/AppImage) |
| Frontend | React 19.2 + Vite 7.3 + TailwindCSS 3.4 |
| Bundle format | Single-file HTML (vite-plugin-singlefile) |
| Built bundle size | 783 KB (dist/index.html), ~224 KB gzip |
| Module count | 489 (Vite build) |
| Rust edition | 2021 |
| Minimum macOS | 10.15 |

### Codebase Size

| Layer | Files | Lines |
|---|---|---|
| Rust backend (compute + worker + app) | 31 .rs files | ~11,988 |
| Frontend (JSX + JS + CSS) | 57 files | ~7,626 |
| **Total** | 88 source files | **~19,614** |

### Rust Module Inventory (src-tauri/src/)

**Compute engine** (src-tauri/src/compute/):

| Module | Lines | Purpose |
|---|---|---|
| gpu.rs | 1,138 | wgpu GPU compute (SMA, EMA, Batch SMA shaders) |
| backtest.rs | 819 | Full backtest engine, 25+ entry conditions, ATR exits |
| ml_train.rs | 794 | smartcore RandomForest training, walk-forward CV |
| cache.rs | 386 | Offline data cache, seeding, eviction |
| types.rs | 357 | All shared data types and IPC structs |
| indicators.rs | 332 | RSI, EMA, ATR, SMA, Bollinger, OBV |
| demo.rs | 312 | Demo/test data generation |
| hardware.rs | 290 | CPU/RAM/GPU detection (cross-platform) |
| scanner.rs | 289 | Signal scanner (momentum, reversal, breakout, volume) |
| ml.rs | 204 | ML ensemble inference |
| metrics.rs | 198 | Sharpe, Sortino, profit factor, drawdown |
| features.rs | 188 | Technical feature extraction for ML pipeline |
| data.rs | 185 | OHLCV data loading from Parquet/CSV |
| gpu_stub.rs | 90 | No-op GPU stub when `gpu` feature is off |
| mod.rs | 55 | Module declarations and re-exports |

**Grid worker** (src-tauri/src/worker/):

| Module | Lines | Purpose |
|---|---|---|
| grid_worker.rs | 708 | HTTP-based grid worker loop |
| redis_worker.rs | 489 | Redis ZPOPMIN feeder/worker/reporter architecture |
| job_executor.rs | 410 | Job dispatch to Rust compute, per-job timeouts |
| mod.rs | 185 | IPC commands: start/stop/status |

**Application layer** (src-tauri/src/):

| Module | Lines | Purpose |
|---|---|---|
| startup.rs | 1,035 | App initialization, data seeding, migration |
| credential_store.rs | 619 | AES-256-GCM credential encryption |
| lib.rs | 401 | Tauri command registration, plugin setup |
| config.rs | 279 | AppConfig, managed state |
| tray.rs | 278 | System tray icon and menu |
| auth.rs | 245 | JWT authentication, token refresh |
| bot_manager.rs | 217 | Bot lifecycle management |
| local_bots.rs | 210 | Local bot execution |
| preferences.rs | 159 | User preferences persistence |
| background.rs | 146 | Background polling tasks |
| safe_io.rs | 146 | Atomic file writes |
| api_proxy.rs | 105 | API proxy for Cloudflare bypass |
| telemetry_consent.rs | 97 | GDPR-compliant telemetry opt-in |
| updater.rs | 64 | Auto-update check logic |
| crash_reporter.rs | 50 | Crash dump collection |
| main.rs | 6 | Entry point |

### Frontend Structure (src/)

| Area | Key Files |
|---|---|
| Routing | App.jsx (258 lines) -- phase-based: startup -> login/explorer -> workspace |
| Docking | flexlayout-react, WorkspaceShell.jsx, panelRegistry.js, defaultLayout.js |
| Pages | LoginPage, StartupPage, ExplorerPage (536 lines, largest component) |
| Panels (17) | BotManager, PortfolioBrain, GridCompute, Regime, BotCommand, Positions, Watchlist, Chart, Scanner, Alerts, StrategyManager, BrokerSetup, Backtest, Settings, IntelligenceDashboard, MetaAllocator, StrategyRouting |
| Hooks (12) | useAlerts, useBacktests, useBrokerSetup, useChartData, useLiveBots, useLocalBots, usePositions, useScanners, useStrategies, useStrategyRouting, useVisibility, useWatchlists |
| Contexts | AuthContext, ConfigContext, PreferencesContext |
| Charting | lightweight-charts v5.1 |
| Animation | framer-motion v12.35 |

---

## Compute Engine Architecture

### CPU: rayon Parallelism

rayon is used in 6 compute modules for multi-symbol parallel processing:

| Module | Usage |
|---|---|
| backtest.rs | `par_iter()` over symbols for parallel backtest execution |
| features.rs | `par_iter()` over symbols for parallel feature extraction |
| indicators.rs | `into_par_iter()` for parallel ATR True Range computation |
| ml.rs | `par_iter()` over symbols for parallel ML inference |
| ml_train.rs | `par_iter()` for training sample extraction + walk-forward folds |
| scanner.rs | `par_iter()` over symbols for parallel signal scanning |

rayon uses all available CPU cores by default (work-stealing thread pool). The `PAR_WINDOW_THRESHOLD` constant (500) prevents rayon overhead for small datasets where sequential execution is faster.

### GPU: wgpu Compute Shaders

Built behind the `gpu` Cargo feature flag (off by default). When enabled, compiles against wgpu v29 with Vulkan, DX12, and Metal backends.

**Shaders (WGSL):**

| Shader | Workgroup Size | Design |
|---|---|---|
| SMA | 256 | Each thread computes one output element. Parallelism = ceil(N/256) workgroups. |
| EMA | 1 | Inherently sequential (each value depends on previous). Single workgroup, serial on GPU. Useful when part of a larger batch pipeline to avoid CPU-GPU context switches. |
| Batch SMA | 256 | Multi-symbol: each workgroup Y-axis = symbol, X-axis = data point. One GPU dispatch for all symbols. |

**GPU/CPU selection:**

- `GPU_THRESHOLD`: 100 bars (lowered from 1000 to catch grid jobs with ~250 bars)
- `compute_sma_auto()` / `compute_ema_auto()` check threshold + adapter availability
- Falls back to CPU transparently if no GPU or below threshold
- f32 on GPU (WGSL limitation), f64 on CPU; precision loss ~1e-7, acceptable for financial indicators
- GpuContext is initialized once (lazy `OnceCell`) and cached for the process lifetime
- `is_gpu_available()` uses a synchronous `OnceLock` for fast repeated checks after first probe
- Benchmark IPC command available: `gpu_benchmark` compares GPU vs CPU timing

### ML: smartcore RandomForest

- Zero Python dependencies -- pure Rust ML
- `RandomForestClassifier` from smartcore v0.4
- 10 features: RSI14, EMA9, EMA21, ATR14, BB width, returns (1d/5d/20d), volume ratio, close
- Labels: binary (profitable = return > 0 over 5-day forward window)
- Model serialized to `~/.aura-worker/models/rf_model.json`
- In-memory model cache (OnceLock) avoids repeated disk IO
- Batched prediction via single DenseMatrix call
- Walk-forward cross-validation with parallel folds (rayon)
- Feature normalization with cached scaler parameters
- ML train semaphore: max 2 concurrent training jobs (prevent OOM)
- 32 MB stack threads for training (large matrices)

### Data: Polars + Parquet/CSV

- Polars v0.46 with `parquet`, `csv`, `lazy` features
- OHLCV data loaded from `~/.aura-worker/data/{region}/` directory
- Parquet preferred, CSV fallback
- Cache manager handles: seeding from bundled sample data, downloading from API, stale eviction
- Bundled sample data: `resources/sample_data/*.csv` copied on first launch

---

## Optimizations Applied (2026-04-08)

Commit `c68268d` -- 6 parallel optimization agents, 35 files changed, +532/-261 lines.

### 1. Visibility-Based Polling Pause

**Hook**: `useVisibility()` (11 lines) -- listens to `document.visibilitychange`

**Applied to 4 polling hooks:**
- `useAlerts.js` -- alert polling pauses when tab hidden
- `useLiveBots.js` -- bot status polling pauses when tab hidden
- `usePositions.js` -- position polling pauses when tab hidden
- `useLocalBots.js` -- local bot polling pauses when tab hidden

**Impact**: Zero network requests and zero React re-renders when the app window is minimized or in background. Resumes instantly on focus.

### 2. Console Cleanup

33 `console.log`/`console.warn`/`console.error` statements across 18 files gated behind `import.meta.env.DEV`. In production builds, these are dead code eliminated by Vite. Additionally, Terser's `drop_console: true` strips any that slip through.

### 3. Lazy Loading

**Page-level** (App.jsx):
- `LoginPage` -- lazy loaded
- `StartupPage` -- lazy loaded
- `ExplorerPage` -- lazy loaded
- `WorkspaceShell` -- lazy loaded

**Panel-level** (panelRegistry.js) -- all 17 panels are `React.lazy()`:
BotCommandPanel, PositionsPanel, WatchlistPanel, ChartPanel, ScannerPanel, AlertsPanel, StrategyManagerPanel, BrokerSetupPanel, BotManagerPanel, BacktestPanel, SettingsPanel, IntelligenceDashboardPanel, RegimePanel, PortfolioBrainPanel, MetaAllocatorPanel, GridComputePanel, StrategyRoutingPanel

**Skeleton**: `PanelLoader.jsx` (44 lines) provides a shimmer placeholder during chunk load, used as `Suspense` fallback for every panel.

### 4. Cargo Optimization

**tokio**: Reduced from full feature set to 6 specific features:
```
["rt-multi-thread", "sync", "time", "macros", "process", "signal"]
```
Removed: `io-util`, `io-std`, `net`, `fs` (not used in this codebase).

**GPU**: Changed from `default = ["gpu"]` to `default = []`. GPU is now opt-in via `--features gpu`. Eliminates wgpu compile time and binary size for standard builds.

**Release profile**:
```toml
[profile.release]
opt-level = "z"      # optimize for size
lto = true           # link-time optimization
codegen-units = 1    # single codegen unit for better optimization
strip = true         # strip debug symbols
panic = "abort"      # smaller binary, no unwinding
```

### 5. React Memoization

8 components received `React.memo`, `useMemo`, and/or `useCallback`:

| Component | Optimizations |
|---|---|
| ExplorerPage | `memo()` wrapper, `MetricCard` memo, `SymbolRow` memo, `SymbolRowWrapper` memo, `useCallback` on toggle/demo |
| BotActivityFeed | `memo()` wrapper, `TradeRow` memo, `useMemo` for filters + filtered data, `useCallback` on fetchTrades |
| CommandPalette | `useMemo` for filtered, grouped, flatItems; `useCallback` on executeCommand, handleKeyDown |
| DataTable | Component-level optimization |
| PortfolioBrainPanel | Memoized render sections |
| RegimePanel | Memoized computation |
| GridComputePanel | Memoized state derivations |
| PositionsPanel | Memoized data handling |

### 6. Vite + CSP

**Vite build** (vite.config.js):
- Minifier: terser (2-pass compression, dead code elimination)
- `drop_console: true` + `drop_debugger: true`
- `dead_code: true` + `unused: true`
- Comments stripped
- Single-file output via vite-plugin-singlefile (all assets inlined)
- Target: chrome105 (Windows), safari13 (macOS)

**CSP** (tauri.conf.json):
- Removed `unsafe-eval` from script-src (confirmed zero eval usage in codebase)
- Final CSP: `default-src 'self' https://auraalpha.cc https://*.auraalpha.cc; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' https://*.auraalpha.cc wss://*.auraalpha.cc; img-src 'self' https: data:; font-src 'self' data:`

---

## Electron v8.2.0 Hotfixes

The Electron wrapper is a separate installation (NOT in this repo -- no main.js, preload.js, or Electron deps in package.json). The Tauri source in this repo is the primary build target. Electron was used as a quick-ship wrapper around the web frontend.

Known fixes applied to the Electron build (external):
- `API_BASE_DIRECT` pointed to a dead EC2 IP address; fixed to `https://auraalpha.cc`
- `loadURL` was loading `/index.html` directly; fixed to `/` so React Router handles routing (prevented 404 on refresh)

**Current state**: Tauri v7.4.0 is the canonical desktop app. Electron wrapper exists for legacy compatibility but is not built from this repo.

---

## Intelligence Processing Pipeline

### Grid Worker Architecture

```
EC2 API (coordinator)
    |
    v
Redis sorted set (priority queue)
    |
    v  ZPOPMIN
Feeder (async Tokio task)
    |
    v  mpsc channel
Worker Pool (spawn_blocking, rayon + wgpu)
    |
    v  results mpsc
Reporter (async Tokio task)
    |
    v  POST /api/grid/complete-batch (batches of 50)
EC2 API (result storage)
```

**Heartbeat**: Extends Redis lease TTL every 30 seconds for in-flight jobs.
**Fallback**: If Redis is unavailable, falls back to HTTP API dequeue (`/api/grid/dequeue`).

### Job Types

| Job Type | Executor | Timeout | Notes |
|---|---|---|---|
| backtest | Rust compute (rayon parallel over symbols) | 300s | 25+ entry conditions, ATR exits |
| scan | Rust compute (rayon parallel over symbols) | 300s | Momentum, reversal, breakout, volume |
| feature_extraction | Rust compute (rayon parallel) | 300s | 10 technical features per symbol |
| ml_train | Rust compute (smartcore RF) | 600s | Max 2 concurrent (semaphore), 32 MB stack |
| ml_inference | Rust compute (cached model) | 300s | Batched DenseMatrix prediction |
| health_check | Pure Rust (no subprocess) | N/A | Lightweight |
| ping | Pure Rust (no subprocess) | N/A | Lightweight |

### Current Bottlenecks

1. **EMA on GPU**: Inherently sequential -- single workgroup. Only beneficial in batch pipeline context.
2. **ML training memory**: 32 MB stack threads + semaphore (max 2) to prevent OOM on large datasets.
3. **Data download**: Cache seeding from API is network-bound; Parquet preferred over CSV for read speed.
4. **Single-threaded JSON serialization**: smartcore model serialize/deserialize is single-threaded.

---

## Hardware Utilization

### CPU

- **rayon**: Work-stealing thread pool, defaults to all available cores
- **tokio**: Multi-threaded runtime (`rt-multi-thread`), handles async IO, grid worker feeder/reporter, heartbeats
- **spawn_blocking**: Compute-heavy jobs run via `tokio::task::spawn_blocking` to avoid blocking the async runtime
- **PAR_WINDOW_THRESHOLD**: 500 -- prevents rayon overhead for small datasets

### GPU

- **Detection**: Cross-platform via `sysinfo` + platform-specific probes
  - Windows: `nvidia-smi` first, `wmic` fallback
  - Linux/WSL: `nvidia-smi` first, `/proc/driver/nvidia/gpus` fallback
  - macOS: `system_profiler SPDisplaysDataType`
- **wgpu adapter**: Requests `HighPerformance` preference, supports Vulkan + DX12 + Metal
- **Context**: `OnceLock<Option<GpuContext>>` -- initialized once, cached forever
- **Pipelines**: 3 pre-compiled compute pipelines (SMA, EMA, Batch SMA)
- **Buffer strategy**: input -> storage, output -> storage + copy_src, staging -> copy_dst + map_read
- **Precision**: f32 on GPU (WGSL limitation), f64 on CPU; max error ~0.01 for values around 100-200

### Memory

- **Polars DataFrames**: Lazy evaluation where possible, eager only for final materialization
- **ML model cache**: `OnceLock` -- model loaded once from disk, kept in memory
- **GPU context cache**: `OnceCell` -- device + queue + pipelines kept for process lifetime
- **Hardware info cache**: `OnceLock<GpuInfo>` -- detected once per process
- **Data cache**: `~/.aura-worker/data/{region}/` on disk, loaded into memory per-job
- **Feature matrix**: Flat `Vec<f64>` for cache-friendly contiguous layout

---

## Production Recommendations

### Build Commands

**Standard build (no GPU)**:
```bash
npm run tauri:build
```

**GPU-enabled build**:
```bash
cd src-tauri && cargo build --release --features gpu
# Or via tauri CLI:
TAURI_ARGS="--features gpu" npm run tauri:build
```

**Platform-specific**:
```bash
npm run tauri:build:windows   # x86_64-pc-windows-msvc
npm run tauri:build:macos     # universal-apple-darwin
npm run tauri:build:linux     # x86_64-unknown-linux-gnu
```

### GPU Feature Flag

- `default = []` -- GPU is OFF by default to reduce compile time and binary size
- Enable with `--features gpu` when building for machines with dedicated GPUs
- The `gpu_stub.rs` module provides no-op implementations when the feature is off, so all code paths compile cleanly either way
- At runtime, `is_gpu_available()` returns false when compiled without the feature
- The frontend `gpu_compute_status` IPC command works in both modes

### Recommended Hardware

| Workload | CPU | RAM | GPU | Notes |
|---|---|---|---|---|
| Light (5-10 symbols, paper trading) | 4 cores | 8 GB | Not needed | Runs fine without GPU feature |
| Standard (50-100 symbols, live trading) | 8 cores | 16 GB | Optional | rayon saturates 8 cores well |
| Heavy (600 symbols, grid compute) | 16+ cores | 32 GB | Recommended | GPU for batch SMA across all symbols |
| ML training (walk-forward CV) | 16+ cores | 32 GB | Optional | CPU-bound (smartcore), 32 MB stack threads |

### Known Issues and TODOs

1. **Auto-update server endpoint not built**: Tauri updater is configured with pubkey and endpoint URL (`/api/desktop/update/{target}/{arch}/{version}`) but the server-side endpoint does not exist yet.
2. **Code signing**: SSL.com certificate is in validation. CI workflow has CodeSignTool wired but signing is not yet active for release builds.
3. **GPU EMA performance**: The single-workgroup EMA shader is often slower than CPU for individual calls. Only beneficial in batch pipeline context. Consider removing standalone GPU EMA path.
4. **Model serialization**: smartcore model JSON can be large. Consider bincode or MessagePack for faster serialize/deserialize.
5. **Polars lazy not fully utilized**: Some compute paths use eager mode. Audit for lazy evaluation opportunities.
6. **Telemetry consent**: Dialog exists but telemetry collection backend is not implemented.
7. **Offline-first**: Cache seeding works for sample data but full offline mode (no API dependency) is not complete.

---

## Coordination Notes

### Cross-Session / Cross-Agent Rules

- This file (`cooperative.md`) is the single source of truth for desktop app state.
- Any agent modifying the desktop app MUST update this file with what changed.
- Check git log before starting work -- another agent may have pushed.
- Never run `git add -A` -- use specific file paths.
- Test builds locally before pushing to CI (20-minute build cycles are expensive).

### Repository

- **GitHub**: github.com/sgallup23/AuraAlphaDesktop
- **Branch**: master (single branch workflow)
- **Remote**: origin (fetch + push)
- **CI**: GitHub Actions -- builds Windows/macOS/Linux on v* tags

### Architecture Relationship

- **This repo** (AuraAlphaDesktop): Tauri v2, Rust compute engine, React frontend. Canonical desktop app.
- **AuraCommandV2** (separate repo): Web frontend (Vite + React). Shares no code with desktop.
- **Electron wrapper** (not in this repo): Legacy quick-ship wrapper. Uses the web build, not this codebase.
- **prodesk** (separate repo): FastAPI backend, 3 trading bots, backtests. Desktop connects to this API.

### Key Dependencies (Cargo)

| Crate | Version | Purpose |
|---|---|---|
| tauri | 2 | Desktop framework |
| tokio | 1 | Async runtime (6 features) |
| rayon | 1.10 | CPU parallelism |
| polars | 0.46 | DataFrames (parquet, csv, lazy) |
| smartcore | 0.4 | ML (RandomForest) |
| wgpu | 29 | GPU compute (optional) |
| reqwest | 0.12 | HTTP client (json, native-tls) |
| redis | 0.27 | Grid worker dispatch |
| sysinfo | 0.32 | Hardware detection |
| aes-gcm | 0.10 | Credential encryption |
| keyring | 3 | OS keychain integration |
| flexlayout-react | 0.8 | Docking layout (frontend) |

### Recent Commit History (last 20)

```
c68268d perf: full desktop optimization (polling, memo, lazy, Cargo, Vite, CSP)
851ab02 fix: ML train semaphore (max 2 concurrent) + 32MB stack + feeder capacity
fa9f3d6 feat: wire ml_train + walk_forward + GPU threshold 100
7986438 release: v7.4.0 -- Redis dispatch queue + headless grid worker
6cbcca2 feat: Redis-backed grid worker -- feeder/worker/reporter architecture
77d7972 feat: headless Rust grid worker -- pure compute, no GTK/Tauri UI
104a7cf fix: Cloudflare bot protection -- wrap User-Agent in Mozilla/5.0
7a69c93 security: replace XOR fallback with AES-256-GCM for credential encryption
d7c3691 security: remove shell:allow-execute + add CSP to webview
f55d7c8 chore: mark as deprecated -- consolidated into AuraCommandV2
9075d00 release: v7.3.0 -- bump version for strategy routing release
231881a feat: v7.3.0 -- strategy routing panel + execution wiring
449d71f fix: use official SSLcom CodeSignTool v1.3.2 + java -jar directly
daea707 fix: call CodeSignTool jar directly via Java, bypass bat file
d8142c4 fix: use env vars for CodeSignTool to avoid quoting issues
6a4e0d2 fix: cd into CodeSignTool dir before signing (path resolution)
b0047be fix: add Java setup for Windows code signing in CI
774e8dc fix: replace grep -P with sed for Windows CI compat
5ac50b5 fix: v7.2.1 -- grid worker sidecar crash on --verbose flag
1e47880 fix: v7.2.0 -- always send real hostname and hardware in grid worker
```
