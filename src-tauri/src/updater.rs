use crate::config::AppConfigState;

/// Compare semver strings. Returns true if current < minimum.
fn version_below(current: &str, minimum: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let c = parse(current);
    let m = parse(minimum);
    for i in 0..3 {
        let cv = c.get(i).copied().unwrap_or(0);
        let mv = m.get(i).copied().unwrap_or(0);
        if cv < mv {
            return true;
        }
        if cv > mv {
            return false;
        }
    }
    false
}

/// Check if app needs a forced update. Call this after config is initialized.
pub fn check_minimum_version(app_version: &str, config: &crate::config::AppConfig) -> Option<String> {
    if version_below(app_version, &config.minimum_version) {
        Some(format!(
            "This version ({}) is below the minimum required ({}). Please update.",
            app_version, config.minimum_version
        ))
    } else {
        None
    }
}

#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppConfigState>,
) -> Result<serde_json::Value, String> {
    let config = state.0.read().map_err(|e| e.to_string())?;
    let current = app.package_info().version.to_string();
    let needs_update = version_below(&current, &config.minimum_version);
    Ok(serde_json::json!({
        "current_version": current,
        "minimum_version": config.minimum_version,
        "update_required": needs_update,
        "update_endpoint": config.update_endpoint,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_below() {
        assert!(version_below("1.0.0", "2.0.0"));
        assert!(version_below("1.9.9", "2.0.0"));
        assert!(version_below("2.0.0", "2.0.1"));
        assert!(!version_below("2.0.0", "2.0.0"));
        assert!(!version_below("3.0.0", "2.0.0"));
        assert!(!version_below("2.1.0", "2.0.9"));
    }
}
