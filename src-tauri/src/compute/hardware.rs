//! Hardware detection — CPU, RAM, GPU, OS info for registration
//! and the frontend "System Info" display.

use serde::Serialize;
use sysinfo::System;

/// Hardware summary returned to the frontend via Tauri IPC.
#[derive(Debug, Clone, Serialize)]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub ram_gb: f64,
    pub gpu_name: String,
    pub gpu_vram_mb: u64,
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
}

/// Detect hardware capabilities.
///
/// Uses the `sysinfo` crate for CPU/RAM. GPU detection is best-effort
/// (reads sysinfo; actual VRAM detection requires platform-specific
/// APIs that we can add later).
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

    // GPU detection — sysinfo doesn't expose GPU details directly.
    // We attempt to read from common system paths on Linux; on Windows
    // we'd use DXGI. For now, report as "Unknown" if not detectable.
    let (gpu_name, gpu_vram_mb) = detect_gpu();

    // OS info.
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = std::env::consts::ARCH.to_string();

    HardwareInfo {
        cpu_name,
        cpu_cores,
        ram_gb: (ram_gb * 10.0).round() / 10.0, // 1 decimal place
        gpu_name,
        gpu_vram_mb,
        os_name,
        os_version,
        arch,
    }
}

/// Best-effort GPU detection.
///
/// On Linux: tries to read `/proc/driver/nvidia/gpus/*/information`.
/// On Windows: placeholder for future DXGI integration.
/// Falls back to "No GPU detected" / 0.
fn detect_gpu() -> (String, u64) {
    // Try NVIDIA on Linux.
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") {
            for entry in entries.flatten() {
                let info_path = entry.path().join("information");
                if let Ok(content) = std::fs::read_to_string(&info_path) {
                    for line in content.lines() {
                        if line.starts_with("Model:") {
                            let name = line.trim_start_matches("Model:").trim().to_string();
                            return (name, 0); // VRAM requires nvidia-smi parsing.
                        }
                    }
                }
            }
        }
    }

    ("No GPU detected".to_string(), 0)
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
    }
}
