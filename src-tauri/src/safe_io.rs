//! Safe I/O — write operations that never panic, with disk-full detection
//! and atomic file writes (write-to-temp then rename).

use std::path::Path;

/// Check available disk space on the volume containing `path`.
/// Returns available bytes, or None if the check fails.
#[cfg(unix)]
fn available_disk_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;

    // Find the mount point by using the path or its parent
    let check_path = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new("/").to_path_buf())
    };

    let c_path = CString::new(
        check_path.to_string_lossy().as_bytes()
    ).ok()?;

    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
            Some(stat.f_bavail as u64 * stat.f_bsize as u64)
        } else {
            None
        }
    }
}

#[cfg(not(unix))]
fn available_disk_space(_path: &Path) -> Option<u64> {
    // On Windows, we skip the pre-check and rely on write errors
    None
}

/// Minimum free space required before we allow a write (1 MB).
const MIN_FREE_BYTES: u64 = 1_048_576;

/// Write to file with error handling — never panics.
///
/// - Checks available disk space first (Unix only)
/// - Writes to a temporary file, then atomically renames
/// - Returns a user-friendly error string on failure
pub fn safe_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    // 1. Check disk space (best-effort on Unix)
    if let Some(avail) = available_disk_space(path) {
        let needed = contents.len() as u64 + MIN_FREE_BYTES;
        if avail < needed {
            return Err(format!(
                "Insufficient disk space: {} bytes available, need at least {} bytes. \
                 Free some disk space and try again.",
                avail, needed
            ));
        }
    }

    // 2. Ensure parent directory exists
    if let Some(parent) = path.parent() {
        safe_mkdir(parent)?;
    }

    // 3. Write to temporary file
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    ));

    std::fs::write(&tmp_path, contents).map_err(|e| {
        // Clean up partial temp file
        let _ = std::fs::remove_file(&tmp_path);
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!("Permission denied writing to '{}'. Check file permissions.", path.display())
            }
            _ if e.to_string().contains("No space left") => {
                format!("Disk full — cannot write to '{}'. Free some disk space.", path.display())
            }
            _ => {
                format!("Write failed for '{}': {}", path.display(), e)
            }
        }
    })?;

    // 4. Atomic rename
    std::fs::rename(&tmp_path, path).map_err(|e| {
        // If rename fails, try to clean up temp file
        let _ = std::fs::remove_file(&tmp_path);
        format!(
            "Cannot finalize write to '{}': {}. Data was not lost — check disk and permissions.",
            path.display(),
            e
        )
    })?;

    Ok(())
}

/// Create directory (and parents) with error handling — never panics.
pub fn safe_mkdir(path: &Path) -> Result<(), String> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        } else {
            return Err(format!(
                "Cannot create directory '{}': a file with that name already exists.",
                path.display()
            ));
        }
    }

    std::fs::create_dir_all(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            format!(
                "Permission denied creating directory '{}'. Check parent directory permissions.",
                path.display()
            )
        }
        _ => {
            format!("Cannot create directory '{}': {}", path.display(), e)
        }
    })
}

/// Read a file with error handling — never panics.
/// Returns the file contents or a user-friendly error.
pub fn safe_read(path: &Path) -> Result<Vec<u8>, String> {
    if !path.exists() {
        return Err(format!("File not found: '{}'", path.display()));
    }

    std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied reading '{}'. Check file permissions.", path.display())
        }
        _ => {
            format!("Cannot read '{}': {}", path.display(), e)
        }
    })
}
