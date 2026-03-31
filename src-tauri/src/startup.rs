/// Startup logic: process discovery, Python resolution, sidecar spawning,
/// crash detection, hardware checks, and the startup_check / clean_shutdown IPC commands.

use std::process::{Child, Command};

use serde::Serialize;
use tauri::Manager;

use crate::credential_store;

use crate::config;

// ── 0N: Minimum hardware checks (warnings only) ─────────────────────

/// Check system hardware against minimum requirements and log warnings.
/// This never blocks startup — it only emits log::warn messages.
pub fn check_minimum_hardware() {
    let sys = sysinfo::System::new_all();

    // Check RAM (minimum 4 GB)
    let total_ram_bytes = sys.total_memory();
    let total_ram_gb = total_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if total_ram_gb < 4.0 {
        log::warn!(
            "LOW MEMORY: System has {:.1} GB RAM (minimum recommended: 4 GB). \
             Performance may be degraded.",
            total_ram_gb
        );
    } else {
        log::info!("RAM check OK: {:.1} GB available", total_ram_gb);
    }

    // Check CPU cores (minimum 2)
    let cpu_count = sys.cpus().len();
    if cpu_count < 2 {
        log::warn!(
            "LOW CPU: System has {} CPU core(s) (minimum recommended: 2). \
             Grid worker and backtesting performance will be limited.",
            cpu_count
        );
    } else {
        log::info!("CPU check OK: {} cores available", cpu_count);
    }

    // Check disk free space (minimum 500 MB)
    // Use the disk where the app data directory lives
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let app_data_path = dirs::data_local_dir().unwrap_or_default();
    let mut best_match: Option<(usize, u64)> = None; // (mount_point_len, available_space)

    for disk in disks.list() {
        let mount = disk.mount_point();
        if app_data_path.starts_with(mount) {
            let mount_len = mount.to_string_lossy().len();
            match best_match {
                Some((prev_len, _)) if mount_len > prev_len => {
                    best_match = Some((mount_len, disk.available_space()));
                }
                None => {
                    best_match = Some((mount_len, disk.available_space()));
                }
                _ => {}
            }
        }
    }

    if let Some((_, available)) = best_match {
        let available_mb = available as f64 / (1024.0 * 1024.0);
        if available_mb < 500.0 {
            log::warn!(
                "LOW DISK SPACE: {:.0} MB free (minimum recommended: 500 MB). \
                 Logs, grid results, and updates may fail.",
                available_mb
            );
        } else {
            log::info!("Disk check OK: {:.0} MB free", available_mb);
        }
    } else {
        log::info!("Disk check: could not determine free space for app data path");
    }
}

// Local-first: try local sidecar, cloud is backup only
// 0W: Cloud-first — production API is on auraalpha.cc, not localhost.
pub(crate) fn health_url() -> String {
    format!("{}/api/system/health", config::DEFAULT_CLOUD_URL)
}
pub(crate) fn telemetry_url() -> String {
    format!("{}/api/telemetry/latest", config::DEFAULT_CLOUD_URL)
}
pub(crate) fn remote_api_url() -> String {
    format!("{}/api/remote", config::DEFAULT_CLOUD_URL)
}

// ── Shared types (also used by tray.rs) ──────────────────────────────

#[derive(Clone, Serialize)]
pub struct HealthSummary {
    pub api_up: bool,
    pub bots_active: u32,
    pub total_positions: u32,
    pub total_pnl_today: f64,
}

/// Non-IPC helper: fetch health summary from EC2.
/// Used by tray menu and the `check_health` IPC command.
pub async fn fetch_health_summary() -> Result<HealthSummary, String> {
    let client = reqwest::Client::new();
    match client
        .get(&health_url())
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let data: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Parse error: {}", e))?;

            let bots = data.get("bots").and_then(|b| b.as_object());
            let mut bots_active: u32 = 0;
            let mut total_positions: u32 = 0;
            let mut total_pnl: f64 = 0.0;

            if let Some(bots_obj) = bots {
                for (_key, info) in bots_obj {
                    if !info.is_object() || info.get("bot").is_none() {
                        continue;
                    }
                    if info.get("status").and_then(|s| s.as_str()) == Some("OK") {
                        bots_active += 1;
                    }
                    if let Some(payload) = info.get("payload") {
                        if let Some(positions) =
                            payload.get("positions").and_then(|p| p.as_array())
                        {
                            total_positions += positions.len() as u32;
                        }
                        if let Some(equity) = payload.get("equity") {
                            total_pnl += equity
                                .get("day_pnl")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                        }
                    }
                }
            }

            Ok(HealthSummary {
                api_up: true,
                bots_active,
                total_positions,
                total_pnl_today: total_pnl,
            })
        }
        Ok(resp) => Err(format!("API returned status {}", resp.status())),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

/// Non-IPC helper: send a native notification.
/// Used by tray menu and the `send_notification` IPC command.
pub async fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("Failed to show notification '{}': {}", title, e);
    }
}

// ── Project / worker helpers ──────────────────────────────────────────

/// Find the prodesk project directory (checks common paths).
pub fn find_project_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("TRADING_DESK").join("prodesk"),
        home.join("prodesk"),
        home.join("AuraAlpha").join("prodesk"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("ops").join("remote_worker.py").exists())
}

/// Read the REMOTE_WORKER_TOKEN from the project's .env file.
fn read_worker_token(project_dir: &std::path::Path) -> Option<String> {
    let env_file = project_dir.join(".env");
    if !env_file.exists() {
        return None;
    }
    let content = std::fs::read_to_string(env_file).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("REMOTE_WORKER_TOKEN=") {
            let token = val.trim().trim_matches('"').trim_matches('\'');
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Find a Python interpreter (prefers project venv, falls back to system python3).
fn find_python(project_dir: &std::path::Path) -> String {
    let venv_python = project_dir.join(".venv").join("bin").join("python");
    if venv_python.exists() {
        return venv_python.to_string_lossy().to_string();
    }
    let venv_python3 = project_dir.join(".venv").join("bin").join("python3");
    if venv_python3.exists() {
        return venv_python3.to_string_lossy().to_string();
    }
    "python3".to_string()
}

/// Spawn the remote worker process.
pub fn spawn_worker(project_dir: &std::path::Path) -> Result<Child, String> {
    let worker_script = project_dir.join("ops").join("remote_worker.py");
    if !worker_script.exists() {
        return Err("remote_worker.py not found".to_string());
    }

    let token = read_worker_token(project_dir)
        .ok_or_else(|| "REMOTE_WORKER_TOKEN not found in .env".to_string())?;

    let python = find_python(project_dir);
    let log_dir = project_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::warn!("Failed to create log dir {}: {}", log_dir.display(), e);
    }
    let log_file = std::fs::File::create(log_dir.join("remote_worker.log"))
        .map_err(|e| format!("Cannot create log file: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Cannot clone log file: {}", e))?;

    let mut cmd = Command::new(&python);
    cmd.arg(worker_script.to_string_lossy().as_ref())
        .current_dir(project_dir)
        .env("REMOTE_API_URL", &remote_api_url())
        .env("REMOTE_WORKER_TOKEN", &token)
        .env("WORKER_ID", "desktop")
        .env("POLL_INTERVAL", "30")
        .stdout(log_file)
        .stderr(log_err);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to spawn worker: {}", e))
}

// ── Research-worker / sidecar helpers ────────────────────────────────

/// Find the grid worker script path (checks bundled resource dirs per-platform, then fallbacks).
pub fn find_research_worker_script() -> Option<std::path::PathBuf> {
    let target = std::path::Path::new("grid_worker").join("worker.py");
    let legacy = std::path::Path::new("sidecar").join("research_worker.py");

    for search_target in [&target, &legacy] {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let candidate = exe_dir.join(search_target);
                if candidate.exists() {
                    return Some(candidate);
                }
                // macOS .app bundle
                let macos_resources = exe_dir.join("../Resources").join(search_target);
                if macos_resources.exists() {
                    return Some(macos_resources);
                }
                // Linux .deb
                let deb_resources =
                    std::path::Path::new("/usr/share/cc.auraalpha.desktop").join(search_target);
                if deb_resources.exists() {
                    return Some(deb_resources);
                }
                // Dev: walk up to project root
                let dev_candidate = exe_dir
                    .ancestors()
                    .find(|p| p.join(search_target).exists())
                    .map(|p| p.join(search_target));
                if let Some(path) = dev_candidate {
                    return Some(path);
                }
            }
        }
        // Fallback: home dir dev clone
        if let Some(home) = dirs::home_dir() {
            let candidate = home.join("AuraAlphaDesktop").join(search_target);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Find the compiled sidecar binary by name.
pub fn find_sidecar_binary(name: &str) -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
            // macOS
            let macos = exe_dir.join("../Resources").join(name);
            if macos.exists() {
                return Some(macos);
            }
            // Linux .deb
            let deb = std::path::Path::new("/usr/share/cc.auraalpha.desktop").join(name);
            if deb.exists() {
                return Some(deb);
            }
            // Dev: walk up to project root
            for ancestor in exe_dir.ancestors() {
                let dev = ancestor.join("src-tauri").join("binaries").join(name);
                if dev.exists() {
                    return Some(dev);
                }
                let release = ancestor
                    .join("src-tauri")
                    .join("target")
                    .join("release")
                    .join(name);
                if release.exists() {
                    return Some(release);
                }
                let debug = ancestor
                    .join("src-tauri")
                    .join("target")
                    .join("debug")
                    .join(name);
                if debug.exists() {
                    return Some(debug);
                }
            }
            // Triple-suffixed binary (Tauri convention, Linux)
            if cfg!(target_os = "linux") {
                let triple_name = format!("{}-x86_64-unknown-linux-gnu", name);
                for ancestor in exe_dir.ancestors() {
                    let dev = ancestor
                        .join("src-tauri")
                        .join("binaries")
                        .join(&triple_name);
                    if dev.exists() {
                        return Some(dev);
                    }
                }
            }
        }
    }
    // Home dir fallback
    if let Some(home) = dirs::home_dir() {
        let candidate = home
            .join("AuraAlphaDesktop")
            .join("src-tauri")
            .join("binaries")
            .join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        let dist = home
            .join("AuraAlphaDesktop")
            .join("grid_worker")
            .join("dist")
            .join(name);
        if dist.exists() {
            return Some(dist);
        }
    }
    None
}

/// On macOS, remove Gatekeeper quarantine and fix execute bits on the sidecar dir.
#[cfg(target_os = "macos")]
pub fn fix_gatekeeper_permissions(sidecar_dir: &std::path::Path) {
    log::info!(
        "Removing quarantine xattrs from sidecar dir: {}",
        sidecar_dir.display()
    );
    let xattr_result = Command::new("xattr").arg("-cr").arg(sidecar_dir).output();
    match xattr_result {
        Ok(output) => {
            if !output.status.success() {
                log::warn!(
                    "xattr -cr returned non-zero ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Err(e) => log::warn!("Failed to run xattr -cr: {}", e),
    }

    let python_bin_dir = sidecar_dir.join("python").join("bin");
    if python_bin_dir.exists() {
        log::info!(
            "Fixing execute permissions on: {}",
            python_bin_dir.display()
        );
        if let Err(e) = Command::new("chmod")
            .arg("-R")
            .arg("+x")
            .arg(&python_bin_dir)
            .status()
        {
            log::warn!("chmod +x on python/bin failed: {}", e);
        }
    }
}

/// Ensure the bundled Python binary has execute permissions on non-macOS Unix.
#[cfg(not(target_os = "macos"))]
pub fn ensure_python_executable(python_path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(python_path) {
            let mut perms = meta.permissions();
            let mode = perms.mode();
            if mode & 0o111 == 0 {
                log::info!(
                    "Adding execute permission to: {}",
                    python_path.display()
                );
                perms.set_mode(mode | 0o755);
                if let Err(e) = std::fs::set_permissions(python_path, perms) {
                    log::warn!("Failed to chmod bundled Python: {}", e);
                }
            }
        }
    }
}

/// Locate the bundled Python binary path from the sidecar script directory.
/// Returns `(python_path_string, is_bundled)`.
fn find_python_for_worker(script: &std::path::Path) -> (String, bool) {
    let mut found = String::from("python3");
    let mut is_bundled = false;

    if let Some(script_dir) = script.parent() {
        let bundled_unix = script_dir.join("python").join("bin").join("python3");
        let bundled_win = script_dir.join("python").join("python.exe");
        let bundled_unix_res = script_dir
            .parent()
            .map(|p| {
                let grid = p
                    .join("grid_worker")
                    .join("python")
                    .join("bin")
                    .join("python3");
                if grid.exists() {
                    return grid;
                }
                p.join("sidecar").join("python").join("bin").join("python3")
            })
            .unwrap_or_default();

        if bundled_unix.exists() {
            found = bundled_unix.to_string_lossy().to_string();
            is_bundled = true;
            log::info!("Using bundled Python: {}", found);
        } else if bundled_win.exists() {
            found = bundled_win.to_string_lossy().to_string();
            is_bundled = true;
            log::info!("Using bundled Python (Windows): {}", found);
        } else if bundled_unix_res.exists() {
            found = bundled_unix_res.to_string_lossy().to_string();
            is_bundled = true;
            log::info!("Using bundled Python (Resources): {}", found);
        }
    }

    if found == "python3" {
        if let Some(home) = dirs::home_dir() {
            let prodesk_venv = home
                .join("TRADING_DESK")
                .join("prodesk")
                .join(".venv")
                .join("bin")
                .join("python");
            if prodesk_venv.exists() {
                found = prodesk_venv.to_string_lossy().to_string();
            }
        }
    }

    (found, is_bundled)
}

/// Spawn the research worker sidecar (compiled binary or Python fallback).
pub fn spawn_research_worker(coordinator_url: &str, max_parallel: u32) -> Result<Child, String> {
    let binary_name = if cfg!(target_os = "windows") {
        "aura-grid-worker.exe"
    } else {
        "aura-grid-worker"
    };

    let binary = find_sidecar_binary(binary_name)
        .or_else(find_research_worker_script)
        .ok_or_else(|| "Grid worker binary not found".to_string())?;

    let use_python = binary.extension().map_or(false, |ext| ext == "py");

    #[cfg(target_os = "macos")]
    if let Some(dir) = binary.parent() {
        fix_gatekeeper_permissions(dir);
    }

    #[cfg(unix)]
    if !use_python {
        if let Err(e) = std::fs::set_permissions(
            &binary,
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
        ) {
            log::warn!("Failed to set permissions on {}: {}", binary.display(), e);
        }
    }

    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
        })
        .join("cc.auraalpha.desktop");
    let log_dir = data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::warn!("Failed to create log dir {}: {}", log_dir.display(), e);
    }
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        log::warn!("Failed to create data dir {}: {}", data_dir.display(), e);
    }

    let log_file = std::fs::File::create(log_dir.join("research_worker.log"))
        .map_err(|e| format!("Cannot create log file: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Cannot clone log file: {}", e))?;

    let mut cmd = if use_python {
        let (python, _) = find_python_for_worker(&binary);
        let mut c = Command::new(&python);
        c.arg(binary.to_string_lossy().as_ref());
        c
    } else {
        Command::new(binary.to_string_lossy().as_ref())
    };

    cmd.arg("--coordinator-url")
        .arg(coordinator_url)
        .arg("--max-parallel")
        .arg(max_parallel.to_string())
        .arg("--verbose")
        .env("GRID_TOKEN_DIR", data_dir.to_string_lossy().as_ref())
        .stdout(log_file)
        .stderr(log_err);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let label = if use_python { "Python" } else { "binary" };
    cmd.spawn()
        .map_err(|e| format!("Failed to start grid worker ({}): {}", label, e))
}

// ── IPC commands ──────────────────────────────────────────────────────

/// IPC command: run startup checks (API reachability, crash detection, auth/broker status).
#[tauri::command]
pub async fn startup_check(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let mut status = serde_json::json!({
        "api_reachable": false,
        "has_auth": false,
        "has_broker": false,
        "previous_crash": false,
        "needs_onboarding": true,
    });

    // 1. Check API health (cloud-first — auraalpha.cc is production)
    let client = reqwest::Client::new();
    if let Ok(resp) = client
        .get(&health_url())
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        status["api_reachable"] = serde_json::Value::Bool(resp.status().is_success());
    }

    // 2. Check lock file for crash detection
    let lock_path = dirs::data_dir()
        .unwrap_or_default()
        .join("AuraAlpha")
        .join(".running.lock");
    if lock_path.exists() {
        status["previous_crash"] = serde_json::Value::Bool(true);
        if let Err(e) = std::fs::remove_file(&lock_path) {
            log::warn!("Failed to remove crash lock file {}: {}", lock_path.display(), e);
        }
    }
    if let Some(lock_dir) = lock_path.parent() {
        if let Err(e) = std::fs::create_dir_all(lock_dir) {
            log::warn!("Failed to create lock dir {}: {}", lock_dir.display(), e);
        }
    }
    if let Err(e) = std::fs::write(&lock_path, format!("{}", std::process::id())) {
        log::warn!("Failed to write lock file {}: {}", lock_path.display(), e);
    }

    // 3. Check for saved auth tokens
    {
        use tauri_plugin_store::StoreExt;
        if let Ok(store) = app.store("auth.json") {
            let access = store
                .get("access_token")
                .unwrap_or(serde_json::Value::Null);
            if access.is_string()
                && access.as_str().unwrap_or("") != ""
                && access.as_str() != Some("null")
            {
                status["has_auth"] = serde_json::Value::Bool(true);
                status["needs_onboarding"] = serde_json::Value::Bool(false);
            }
        }
    }

    // 4. Check for configured broker
    let configured = credential_store::list_configured_brokers();
    if !configured.is_empty() {
        status["has_broker"] = serde_json::Value::Bool(true);
    }

    Ok(status)
}

/// IPC command: clean shutdown — remove lock file used for crash detection.
#[tauri::command]
pub async fn clean_shutdown() -> Result<(), String> {
    let lock_path = dirs::data_dir()
        .unwrap_or_default()
        .join("AuraAlpha")
        .join(".running.lock");
    if let Err(e) = std::fs::remove_file(&lock_path) {
        log::warn!("Failed to remove shutdown lock file {}: {}", lock_path.display(), e);
    }
    Ok(())
}

// ── Non-IPC worker helpers (used by tray.rs) ─────────────────────────

/// Worker status — mirrors the IPC type but defined here to avoid macro conflicts.
#[derive(Clone, Serialize)]
pub struct WorkerStatusInfo {
    pub running: bool,
    pub pid: Option<u32>,
    pub project_path: Option<String>,
}

/// Research worker status — mirrors the IPC type.
#[derive(Clone, Serialize)]
pub struct ResearchWorkerStatusInfo {
    pub running: bool,
    pub pid: Option<u32>,
    pub coordinator_url: Option<String>,
}

/// Non-IPC: start the remote worker (called from tray menu handler).
pub async fn tray_start_worker(
    state: tauri::State<'_, crate::WorkerState>,
) -> Result<WorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(WorkerStatusInfo {
                    running: true,
                    pid: Some(child.id()),
                    project_path: find_project_dir().map(|p| p.to_string_lossy().to_string()),
                });
            }
            _ => {
                *guard = None;
            }
        }
    }

    let project_dir = find_project_dir().ok_or("Project directory not found")?;
    let child = spawn_worker(&project_dir)?;
    let pid = child.id();
    *guard = Some(child);

    Ok(WorkerStatusInfo {
        running: true,
        pid: Some(pid),
        project_path: Some(project_dir.to_string_lossy().to_string()),
    })
}

/// Non-IPC: get research worker running status (called from tray menu handler).
pub async fn tray_research_worker_status(
    state: tauri::State<'_, crate::ResearchWorkerState>,
) -> bool {
    let mut guard = match state.child.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => return true,
            _ => {
                *guard = None;
            }
        }
    }
    false
}

/// Non-IPC: stop the research worker (called from tray menu handler).
pub async fn tray_stop_research_worker(
    state: tauri::State<'_, crate::ResearchWorkerState>,
) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut child) = *guard {
        if let Err(e) = child.kill() {
            log::warn!("Failed to kill research worker: {}", e);
        }
        if let Err(e) = child.wait() {
            log::warn!("Failed to wait on research worker: {}", e);
        }
    }
    *guard = None;
    log::info!("Research worker stopped (via tray)");
    Ok(())
}

/// Non-IPC: start the research worker (called from tray menu handler).
pub async fn tray_start_research_worker(
    state: tauri::State<'_, crate::ResearchWorkerState>,
) -> Result<ResearchWorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(ResearchWorkerStatusInfo {
                    running: true,
                    pid: Some(child.id()),
                    coordinator_url: Some(config::DEFAULT_CLOUD_URL.to_string()),
                });
            }
            _ => {
                *guard = None;
            }
        }
    }

    let url = config::DEFAULT_CLOUD_URL;
    let child = spawn_research_worker(url, 2)?;
    let pid = child.id();
    *guard = Some(child);

    log::info!("Research worker started (PID {}) (via tray)", pid);
    Ok(ResearchWorkerStatusInfo {
        running: true,
        pid: Some(pid),
        coordinator_url: Some(url.to_string()),
    })
}

// ── Worker IPC commands (registered in lib.rs generate_handler!) ──────

/// IPC command: start the remote worker.
#[tauri::command]
pub async fn start_worker(
    state: tauri::State<'_, crate::WorkerState>,
) -> Result<WorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(WorkerStatusInfo {
                    running: true,
                    pid: Some(child.id()),
                    project_path: find_project_dir().map(|p| p.to_string_lossy().to_string()),
                });
            }
            _ => {
                *guard = None;
            }
        }
    }

    let project_dir = find_project_dir().ok_or("Project directory not found")?;
    let child = spawn_worker(&project_dir)?;
    let pid = child.id();
    *guard = Some(child);

    Ok(WorkerStatusInfo {
        running: true,
        pid: Some(pid),
        project_path: Some(project_dir.to_string_lossy().to_string()),
    })
}

/// IPC command: stop the remote worker.
#[tauri::command]
pub async fn stop_worker(
    state: tauri::State<'_, crate::WorkerState>,
) -> Result<WorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut child) = *guard {
        if let Err(e) = child.kill() {
            log::warn!("Failed to kill remote worker: {}", e);
        }
        if let Err(e) = child.wait() {
            log::warn!("Failed to wait on remote worker: {}", e);
        }
    }
    *guard = None;

    Ok(WorkerStatusInfo {
        running: false,
        pid: None,
        project_path: find_project_dir().map(|p| p.to_string_lossy().to_string()),
    })
}

/// IPC command: check remote worker status.
#[tauri::command]
pub async fn get_worker_status(
    state: tauri::State<'_, crate::WorkerState>,
) -> Result<WorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    let project_path = find_project_dir().map(|p| p.to_string_lossy().to_string());

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => Ok(WorkerStatusInfo {
                running: true,
                pid: Some(child.id()),
                project_path,
            }),
            _ => {
                *guard = None;
                Ok(WorkerStatusInfo {
                    running: false,
                    pid: None,
                    project_path,
                })
            }
        }
    } else {
        Ok(WorkerStatusInfo {
            running: false,
            pid: None,
            project_path,
        })
    }
}

// ── Research worker IPC commands ──────────────────────────────────────

/// IPC command: start the research/grid worker sidecar.
#[tauri::command]
pub async fn start_research_worker(
    state: tauri::State<'_, crate::ResearchWorkerState>,
    coordinator_url: Option<String>,
    max_parallel: Option<u32>,
) -> Result<ResearchWorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => {
                return Ok(ResearchWorkerStatusInfo {
                    running: true,
                    pid: Some(child.id()),
                    coordinator_url: Some(coordinator_url.unwrap_or_default()),
                });
            }
            _ => {
                *guard = None;
            }
        }
    }

    let url = coordinator_url.unwrap_or_else(|| config::DEFAULT_CLOUD_URL.to_string());
    let par = max_parallel.unwrap_or(2);
    let child = spawn_research_worker(&url, par)?;
    let pid = child.id();
    *guard = Some(child);

    log::info!("Research worker started (PID {})", pid);
    Ok(ResearchWorkerStatusInfo {
        running: true,
        pid: Some(pid),
        coordinator_url: Some(url),
    })
}

/// IPC command: stop the research/grid worker sidecar.
#[tauri::command]
pub async fn stop_research_worker(
    state: tauri::State<'_, crate::ResearchWorkerState>,
) -> Result<ResearchWorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut child) = *guard {
        if let Err(e) = child.kill() {
            log::warn!("Failed to kill research worker sidecar: {}", e);
        }
        if let Err(e) = child.wait() {
            log::warn!("Failed to wait on research worker sidecar: {}", e);
        }
    }
    *guard = None;

    log::info!("Research worker stopped");
    Ok(ResearchWorkerStatusInfo {
        running: false,
        pid: None,
        coordinator_url: None,
    })
}

/// IPC command: check research/grid worker sidecar status.
#[tauri::command]
pub async fn research_worker_status(
    state: tauri::State<'_, crate::ResearchWorkerState>,
) -> Result<ResearchWorkerStatusInfo, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(None) => Ok(ResearchWorkerStatusInfo {
                running: true,
                pid: Some(child.id()),
                coordinator_url: None,
            }),
            _ => {
                *guard = None;
                Ok(ResearchWorkerStatusInfo {
                    running: false,
                    pid: None,
                    coordinator_url: None,
                })
            }
        }
    } else {
        Ok(ResearchWorkerStatusInfo {
            running: false,
            pid: None,
            coordinator_url: None,
        })
    }
}

// ── Local API sidecar helper ──────────────────────────────────────────

/// Attempt to start the local API sidecar (standalone mode).
/// Tries python3 first, then python (Windows fallback).
pub async fn try_start_local_api(app_handle: tauri::AppHandle, local_api_state: tauri::State<'_, crate::LocalApiState>) {
    use std::process::Command;
    let sidecar_dir = app_handle
        .path()
        .resource_dir()
        .unwrap_or_default()
        .join("sidecar")
        .join("local_api");

    match Command::new("python3")
        .arg(sidecar_dir.join("main.py"))
        .current_dir(&sidecar_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            log::info!("Local API started (PID {})", child.id());
            // 0AB: Handle mutex poison — kill child if we can't store it
            match local_api_state.child.lock() {
                Ok(mut guard) => {
                    *guard = Some(child);
                }
                Err(e) => {
                    log::error!("Mutex poisoned storing local API child, killing process: {}", e);
                    if let Err(ke) = child.kill() {
                        log::warn!("Failed to kill orphaned local API process: {}", ke);
                    }
                }
            }
        }
        Err(e) => {
            match Command::new("python")
                .arg(sidecar_dir.join("main.py"))
                .current_dir(&sidecar_dir)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(mut child) => {
                    log::info!("Local API started via python (PID {})", child.id());
                    // 0AB: Handle mutex poison — kill child if we can't store it
                    match local_api_state.child.lock() {
                        Ok(mut guard) => {
                            *guard = Some(child);
                        }
                        Err(e) => {
                            log::error!("Mutex poisoned storing local API child, killing process: {}", e);
                            if let Err(ke) = child.kill() {
                                log::warn!("Failed to kill orphaned local API process: {}", ke);
                            }
                        }
                    }
                }
                Err(_) => {
                    log::warn!("Local API failed to start: {}. Cloud mode only.", e);
                }
            }
        }
    }
}
