# DEPRECATED

This project (AuraAlphaDesktop / Tauri v2 desktop app) has been deprecated
and consolidated into **AuraCommandV2**.

## What moved where

| Feature | New Location |
|---------|-------------|
| Grid Worker (Python) | `AuraCommandV2/grid_worker/` |
| Standalone Worker Module | `AuraCommandV2/grid_worker/standalone/` |
| All UI panels (17 panels) | Already covered by AuraCommandV2's 200+ pages |
| Desktop download page | `AuraCommandV2/frontend/src/pages/DesktopPage.jsx` |
| Compute dashboard | `AuraCommandV2/frontend/src/pages/ComputeDashboardPage.jsx` |

## What was NOT migrated (desktop-only, not needed in web)

- **Tauri Rust backend** (tray icon, credential store, crash reporter, updater,
  telemetry consent) -- these are native OS integrations that only made sense
  in a native desktop shell. The web app uses browser-native equivalents.
- **FlexLayout docking system** (`flexlayout-react`) -- the Bloomberg-style
  panel docking layout. AuraCommandV2 uses page-based routing instead, which
  works across web, iOS, and Android.
- **GPU compute shaders** (WGSL via wgpu) -- Rust-native SMA/EMA compute
  shaders. Server-side compute handles this in production.
- **PyInstaller build** (`aura-grid-worker.spec`) -- the grid worker is now
  distributed as a plain Python script, not a frozen executable.

## Why

AuraCommandV2 is the ONE frontend: React + Vite, 200+ pages, Capacitor for
iOS/Android. Maintaining a separate Tauri codebase for desktop added complexity
without enough unique value. The grid worker (the most useful piece) has been
extracted as a standalone script.

## Do NOT delete this repo

Keep it archived for reference. The Rust compute engine and GPU shaders may
be useful as reference material for future server-side optimizations.
