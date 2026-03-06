# Aura Alpha Desktop — Setup Guide

Cross-platform desktop wrapper for Aura Alpha Trading Desk.
Built with **Tauri v2** (Rust + native webview). ~10MB installer.

## Supported Platforms

| Platform | Format | Webview |
|----------|--------|---------|
| Windows 10/11 | `.msi`, `.exe` (NSIS) | WebView2 (Edge) |
| macOS 12+ | `.dmg` | WebKit (Safari) |
| Linux (Ubuntu/Debian) | `.deb`, `.AppImage` | WebKitGTK |

## Features

- Loads auraalpha.cc in a native window (instant updates, no reinstall needed)
- System tray with bot status and health checks
- Native desktop notifications for trade alerts
- Auto-update via GitHub Releases
- Minimize to tray (keeps running in background)
- Window state persistence (remembers size/position)
- Offline detection with retry

## Prerequisites

### All Platforms
- Node.js 18+
- Rust 1.77+ (`rustup update stable`)

### Linux (Ubuntu/Debian)
```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev
```

### Windows
- WebView2 (pre-installed on Windows 10/11)
- Visual Studio Build Tools with C++ workload

### macOS
- Xcode Command Line Tools (`xcode-select --install`)

## Development

```bash
cd ~/AuraAlphaDesktop
npm install
npm run tauri:dev
```

## Building

```bash
# Current platform
npm run tauri:build

# Platform-specific
npm run tauri:build:windows
npm run tauri:build:macos
npm run tauri:build:linux
```

Output goes to `src-tauri/target/release/bundle/`.

## CI/CD (GitHub Actions)

Push a version tag to trigger cross-platform builds:

```bash
git tag v1.0.0
git push origin v1.0.0
```

This creates a draft GitHub Release with installers for all 3 platforms.

### Required Secrets

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Signs update bundles |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Key password |

Generate signing keys:
```bash
cargo tauri signer generate -w ~/.tauri/aura-alpha.key
```

## Architecture

```
AuraAlphaDesktop/
├── package.json              # npm scripts
├── src/
│   └── index.html            # Loading screen (shown before remote loads)
├── src-tauri/
│   ├── Cargo.toml            # Rust dependencies
│   ├── tauri.conf.json       # App config (window, tray, bundle, updater)
│   ├── capabilities/
│   │   └── default.json      # Security permissions
│   ├── icons/                # App icons (all sizes)
│   └── src/
│       ├── main.rs           # Entry point
│       └── lib.rs            # App logic (tray, IPC commands, health checks)
└── .github/workflows/
    └── release.yml           # Cross-platform CI/CD
```

### How It Works

1. App launches → shows local `index.html` (loading screen)
2. Checks `auraalpha.cc/api/system/health`
3. If reachable → navigates webview to `https://auraalpha.cc`
4. If not → shows connection error with retry button
5. System tray stays active when window is closed
6. Auto-update checks on startup via configured endpoint
