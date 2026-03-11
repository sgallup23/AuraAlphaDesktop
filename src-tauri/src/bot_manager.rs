//! Bot Manager — spawns and monitors Python trading bot processes.
//!
//! Each bot runs as a subprocess: `python3 -m bots.equity.engine --config <path>`
//! The manager tracks PIDs, forwards stdout/stderr to logs, and handles
//! graceful shutdown via SIGTERM.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Configuration for a single bot instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub bot_name: String,
    pub broker: String,
    pub strategies: Vec<String>,
    pub allocation: BotAllocation,
    pub risk: BotRisk,
    pub signals_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotAllocation {
    pub max_positions: u32,
    pub position_size_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotRisk {
    pub max_drawdown_pct: f64,
    pub daily_loss_limit: f64,
}

/// Runtime info for a managed bot process
#[derive(Clone, Debug, Serialize)]
pub struct BotInfo {
    pub bot_name: String,
    pub broker: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub config_path: Option<String>,
    pub log_path: Option<String>,
    pub started_at: Option<u64>,
}

/// Managed state: holds all running bot processes
pub struct BotManagerState {
    pub bots: Mutex<HashMap<String, BotProcess>>,
}

pub struct BotProcess {
    pub child: Child,
    pub config: BotConfig,
    pub config_path: PathBuf,
    pub log_path: PathBuf,
    pub started_at: u64,
}

impl BotManagerState {
    pub fn new() -> Self {
        Self {
            bots: Mutex::new(HashMap::new()),
        }
    }
}

/// Find the prodesk project directory
pub fn find_project_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("TRADING_DESK").join("prodesk"),
        home.join("prodesk"),
        home.join("AuraAlpha").join("prodesk"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("bots").join("equity").join("engine.py").exists())
}

/// Find a Python interpreter (prefer project venv)
pub fn find_python(project_dir: &Path) -> String {
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

/// Write bot config JSON to a temp file and return the path
pub fn write_bot_config(
    project_dir: &Path,
    config: &BotConfig,
    credentials: &serde_json::Value,
) -> Result<PathBuf, String> {
    let config_dir = project_dir.join("shared_state").join("bot_configs");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Cannot create config dir: {}", e))?;

    let config_path = config_dir.join(format!("{}.json", config.bot_name));

    // Build full config with credentials injected
    let mut full_config = serde_json::to_value(config).map_err(|e| e.to_string())?;
    if let Some(obj) = full_config.as_object_mut() {
        obj.insert("credentials".to_string(), credentials.clone());
    }

    let content =
        serde_json::to_string_pretty(&full_config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Cannot write config: {}", e))?;

    Ok(config_path)
}

/// Spawn a bot process
pub fn spawn_bot(
    project_dir: &Path,
    config: &BotConfig,
    config_path: &Path,
    broker_env: &HashMap<String, String>,
) -> Result<(Child, PathBuf), String> {
    let python = find_python(project_dir);

    // Create log file
    let log_dir = project_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("bot_{}.log", config.bot_name));
    let log_file = std::fs::File::create(&log_path)
        .map_err(|e| format!("Cannot create log file: {}", e))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("Cannot clone log file: {}", e))?;

    let mut cmd = Command::new(&python);
    cmd.arg("-m")
        .arg("bots.equity.engine")
        .arg("--config")
        .arg(config_path.to_string_lossy().as_ref())
        .current_dir(project_dir)
        .env("BROKER", &config.broker)
        .env("BOT_NAME", &config.bot_name)
        .env(
            "SIGNALS_URL",
            &config.signals_url,
        )
        .stdout(log_file)
        .stderr(log_err);

    // Inject broker-specific credentials as env vars
    for (key, value) in broker_env {
        cmd.env(key, value);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn bot: {}", e))?;

    Ok((child, log_path))
}

/// Check if a bot process is still running
pub fn check_bot_alive(child: &mut Child) -> bool {
    match child.try_wait() {
        Ok(None) => true,  // still running
        _ => false,        // exited or error
    }
}

/// Gracefully stop a bot (SIGTERM on Unix, kill on Windows)
pub fn stop_bot_process(child: &mut Child) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Send SIGTERM for graceful shutdown
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        // Wait up to 5 seconds for graceful exit
        for _ in 0..50 {
            if let Ok(Some(_)) = child.try_wait() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // Force kill if still running
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(())
}

/// Get current timestamp as Unix epoch seconds
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
