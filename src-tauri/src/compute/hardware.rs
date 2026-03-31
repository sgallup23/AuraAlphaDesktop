//! Hardware detection — CPU, RAM, GPU, OS info for registration
//! and the frontend "System Info" display.
//!
//! GPU detection is cross-platform:
//! - Windows: `nvidia-smi` first, then `wmic` fallback
//! - Linux/WSL: `nvidia-smi` first, then `/proc/driver/nvidia/gpus`
//! - macOS: `system_profiler SPDisplaysDataType`
//!
//! Result is cached via `std::sync::OnceLock` so we only detect once.

use serde::Serialize;
use std::sync::OnceLock;
use sysinfo::System;

/// Hardware summary returned to the frontend via Tauri IPC.
#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub ram_gb: f64,
    pub gpu_name: String,
    pub gpu_vram_mb: u64,
    pub cuda_available: bool,
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
}

/// Cached GPU detection result.
#[derive(Debug, Clone)]
struct GpuInfo {
    name: String,
    vram_mb: u64,
    cuda: bool,
}

/// Global cached GPU info — detected once, reused forever.
static GPU_CACHE: OnceLock<GpuInfo> = OnceLock::new();

/// Detect hardware capabilities.
///
/// Uses the `sysinfo` crate for CPU/RAM. GPU detection is cross-platform
/// and cached so heartbeats don't re-detect.
pub fn detect_hardware() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU info.
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_cores = sys.cpus().len();

    // RAM in GB.
    let ram_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

    // GPU detection — cached.
    let gpu = GPU_CACHE.get_or_init(detect_gpu_cached);

    // OS info.
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = std::env::consts::ARCH.to_string();

    HardwareInfo {
        cpu_name,
        cpu_cores,
        ram_gb: (ram_gb * 10.0).round() / 10.0, // 1 decimal place
        gpu_name: gpu.name.clone(),
        gpu_vram_mb: gpu.vram_mb,
        cuda_available: gpu.cuda,
        os_name,
        os_version,
        arch,
    }
}

/// Cross-platform GPU detection (called once, result cached in OnceLock).
fn detect_gpu_cached() -> GpuInfo {
    // 1. Try nvidia-smi (works on Windows, Linux, WSL if NVIDIA driver installed)
    if let Some(info) = try_nvidia_smi() {
        return info;
    }

    // 2. Platform-specific fallbacks
    #[cfg(target_os = "windows")]
    {
        if let Some(info) = try_wmic() {
            return info;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(info) = try_proc_nvidia() {
            return info;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(info) = try_system_profiler() {
            return info;
        }
    }

    GpuInfo {
        name: "No GPU detected".to_string(),
        vram_mb: 0,
        cuda: false,
    }
}

/// Try `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader`
fn try_nvidia_smi() -> Option<GpuInfo> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?.trim().to_string();
    if line.is_empty() {
        return None;
    }

    // With --format=csv,noheader,nounits output is: "NVIDIA GeForce RTX 4090, 24564"
    // Without nounits it may be: "NVIDIA GeForce RTX 4090, 24564 MiB"
    let parts: Vec<&str> = line.splitn(2, ',').collect();
    let name = parts.first().map(|s| s.trim().to_string()).unwrap_or_default();
    let vram_mb = parts
        .get(1)
        .and_then(|s| {
            let cleaned = s.trim().replace("MiB", "").replace("MB", "").trim().to_string();
            cleaned.parse::<u64>().ok()
        })
        .unwrap_or(0);

    log::info!("GPU detected via nvidia-smi: {} ({} MB)", name, vram_mb);

    Some(GpuInfo {
        name,
        vram_mb,
        cuda: true,
    })
}

/// Windows fallback: `wmic path win32_VideoController get name,adapterram`
#[cfg(target_os = "windows")]
fn try_wmic() -> Option<GpuInfo> {
    let output = std::process::Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "name,adapterram"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip header line, find first data line
    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // wmic output is whitespace-delimited: "AdapterRAM  Name"
        // The AdapterRAM is in bytes (a large number), Name is the rest
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            continue;
        }
        let adapter_ram_bytes: u64 = parts[0].trim().parse().unwrap_or(0);
        let name = parts[1].trim().to_string();

        if name.is_empty() || name.to_lowercase().contains("basic") {
            continue; // Skip "Microsoft Basic Display Adapter"
        }

        let vram_mb = adapter_ram_bytes / (1024 * 1024);
        let cuda = name.to_lowercase().contains("nvidia");

        log::info!("GPU detected via wmic: {} ({} MB)", name, vram_mb);

        return Some(GpuInfo {
            name,
            vram_mb,
            cuda,
        });
    }
    None
}

/// Linux fallback: read `/proc/driver/nvidia/gpus/*/information`
#[cfg(target_os = "linux")]
fn try_proc_nvidia() -> Option<GpuInfo> {
    let entries = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
    for entry in entries.flatten() {
        let info_path = entry.path().join("information");
        if let Ok(content) = std::fs::read_to_string(&info_path) {
            for line in content.lines() {
                if line.starts_with("Model:") {
                    let name = line.trim_start_matches("Model:").trim().to_string();
                    log::info!("GPU detected via /proc: {}", name);
                    return Some(GpuInfo {
                        name,
                        vram_mb: 0, // /proc doesn't report VRAM
                        cuda: true,
                    });
                }
            }
        }
    }
    None
}

/// macOS fallback: `system_profiler SPDisplaysDataType`
#[cfg(target_os = "macos")]
fn try_system_profiler() -> Option<GpuInfo> {
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut name = String::new();
    let mut vram_mb: u64 = 0;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Chipset Model:") {
            name = trimmed
                .trim_start_matches("Chipset Model:")
                .trim()
                .to_string();
        }
        if trimmed.starts_with("VRAM") && trimmed.contains("MB") {
            // "VRAM (Total):  8192 MB" or similar
            if let Some(num_str) = trimmed.split_whitespace().rev().nth(1) {
                vram_mb = num_str.parse().unwrap_or(0);
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    log::info!("GPU detected via system_profiler: {} ({} MB)", name, vram_mb);

    Some(GpuInfo {
        name,
        vram_mb,
        cuda: false, // macOS doesn't have CUDA
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware_runs() {
        let info = detect_hardware();
        // Should always return something.
        assert!(info.cpu_cores > 0);
        assert!(info.ram_gb > 0.0);
        assert!(!info.arch.is_empty());
        // GPU name is never empty (at minimum "No GPU detected")
        assert!(!info.gpu_name.is_empty());
    }

    #[test]
    fn test_gpu_cache_returns_same_result() {
        let info1 = detect_hardware();
        let info2 = detect_hardware();
        assert_eq!(info1.gpu_name, info2.gpu_name);
        assert_eq!(info1.gpu_vram_mb, info2.gpu_vram_mb);
    }
}
