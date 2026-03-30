//! Crash reporter — installs a panic hook that logs to a crash log file.
//!
//! Call `install_panic_hook()` early in app setup to capture any panics
//! that occur during the application lifecycle.

use std::io::Write;

/// Install a panic hook that writes crash details to a log file
/// before calling the default panic handler.
///
/// Log location: `{data_dir}/AuraAlpha/crash.log`
///   - Windows: `%APPDATA%/AuraAlpha/crash.log`
///   - macOS:   `~/Library/Application Support/AuraAlpha/crash.log`
///   - Linux:   `~/.local/share/AuraAlpha/crash.log`
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Log to file
        let crash_log = dirs::data_dir()
            .unwrap_or_default()
            .join("AuraAlpha")
            .join("crash.log");

        // Ensure parent directory exists (best-effort)
        if let Some(parent) = crash_log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
        {
            let timestamp = chrono::Utc::now();
            let _ = writeln!(f, "[{}] PANIC: {}", timestamp, info);

            // Also log backtrace if available
            let bt = std::backtrace::Backtrace::capture();
            let bt_str = format!("{}", bt);
            if !bt_str.is_empty() && !bt_str.contains("disabled") {
                let _ = writeln!(f, "  Backtrace:\n{}", bt_str);
            }

            let _ = writeln!(f, "---");
        }

        // Call the default hook (prints to stderr)
        default_hook(info);
    }));
}
