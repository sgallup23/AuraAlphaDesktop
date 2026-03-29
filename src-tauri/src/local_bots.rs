/// IPC commands for local bot management and broker credential operations.

use std::collections::HashMap;

use crate::{bot_manager, credential_store};

// ── Broker credential IPC ─────────────────────────────────────────────

/// IPC command: get list of all supported brokers with credential field definitions.
#[tauri::command]
pub async fn get_available_brokers() -> Result<Vec<credential_store::BrokerInfo>, String> {
    Ok(credential_store::get_broker_definitions())
}

/// IPC command: save broker credentials to encrypted store.
#[tauri::command]
pub async fn configure_broker(
    broker: String,
    credentials: HashMap<String, String>,
) -> Result<bool, String> {
    credential_store::save_credentials(&broker, &credentials)?;
    log::info!("Saved credentials for broker: {}", broker);
    Ok(true)
}

/// IPC command: delete stored credentials for a broker.
#[tauri::command]
pub async fn delete_broker_credentials(broker: String) -> Result<bool, String> {
    credential_store::delete_credentials(&broker)?;
    log::info!("Deleted credentials for broker: {}", broker);
    Ok(true)
}

/// IPC command: list brokers that have stored credentials.
#[tauri::command]
pub async fn list_configured_brokers() -> Result<Vec<String>, String> {
    Ok(credential_store::list_configured_brokers())
}

// ── Local bot IPC ─────────────────────────────────────────────────────

/// IPC command: start a trading bot with the given configuration.
#[tauri::command]
pub async fn start_bot(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    config: bot_manager::BotConfig,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(existing) = guard.get_mut(&config.bot_name) {
        if bot_manager::check_bot_alive(&mut existing.child) {
            return Ok(bot_manager::BotInfo {
                bot_name: config.bot_name.clone(),
                broker: existing.config.broker.clone(),
                pid: Some(existing.child.id()),
                running: true,
                config_path: Some(existing.config_path.to_string_lossy().to_string()),
                log_path: Some(existing.log_path.to_string_lossy().to_string()),
                started_at: Some(existing.started_at),
            });
        }
        guard.remove(&config.bot_name);
    }

    let project_dir =
        bot_manager::find_project_dir().ok_or("Project directory not found")?;
    let creds = credential_store::load_credentials(&config.broker).unwrap_or_default();
    let creds_json = serde_json::to_value(&creds).unwrap_or_default();
    let config_path = bot_manager::write_bot_config(&project_dir, &config, &creds_json)?;
    let broker_env: HashMap<String, String> = creds;
    let (child, log_path) =
        bot_manager::spawn_bot(&project_dir, &config, &config_path, &broker_env)?;

    let pid = child.id();
    let started_at = bot_manager::now_epoch();

    let info = bot_manager::BotInfo {
        bot_name: config.bot_name.clone(),
        broker: config.broker.clone(),
        pid: Some(pid),
        running: true,
        config_path: Some(config_path.to_string_lossy().to_string()),
        log_path: Some(log_path.to_string_lossy().to_string()),
        started_at: Some(started_at),
    };

    guard.insert(
        config.bot_name.clone(),
        bot_manager::BotProcess {
            child,
            config,
            config_path,
            log_path,
            started_at,
        },
    );

    log::info!("Started bot '{}' (PID {})", info.bot_name, pid);
    Ok(info)
}

/// IPC command: stop a running bot by name.
#[tauri::command]
pub async fn stop_bot(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(mut process) = guard.remove(&bot_name) {
        bot_manager::stop_bot_process(&mut process.child)?;
        log::info!("Stopped bot '{}'", bot_name);
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: process.config.broker,
            pid: None,
            running: false,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        })
    } else {
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: String::new(),
            pid: None,
            running: false,
            config_path: None,
            log_path: None,
            started_at: None,
        })
    }
}

/// IPC command: get status of a specific local bot.
#[tauri::command]
pub async fn get_local_bot_status(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
) -> Result<bot_manager::BotInfo, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(process) = guard.get_mut(&bot_name) {
        let running = bot_manager::check_bot_alive(&mut process.child);
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: process.config.broker.clone(),
            pid: if running { Some(process.child.id()) } else { None },
            running,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        })
    } else {
        Ok(bot_manager::BotInfo {
            bot_name,
            broker: String::new(),
            pid: None,
            running: false,
            config_path: None,
            log_path: None,
            started_at: None,
        })
    }
}

/// IPC command: list all local bots and their statuses.
#[tauri::command]
pub async fn list_local_bots(
    state: tauri::State<'_, bot_manager::BotManagerState>,
) -> Result<Vec<bot_manager::BotInfo>, String> {
    let mut guard = state.bots.lock().map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    for (name, process) in guard.iter_mut() {
        let running = bot_manager::check_bot_alive(&mut process.child);
        result.push(bot_manager::BotInfo {
            bot_name: name.clone(),
            broker: process.config.broker.clone(),
            pid: if running { Some(process.child.id()) } else { None },
            running,
            config_path: Some(process.config_path.to_string_lossy().to_string()),
            log_path: Some(process.log_path.to_string_lossy().to_string()),
            started_at: Some(process.started_at),
        });
    }

    Ok(result)
}

/// IPC command: read recent lines from a bot's log file.
#[tauri::command]
pub async fn get_bot_log(
    state: tauri::State<'_, bot_manager::BotManagerState>,
    bot_name: String,
    tail_lines: Option<usize>,
) -> Result<String, String> {
    let guard = state.bots.lock().map_err(|e| e.to_string())?;

    if let Some(process) = guard.get(&bot_name) {
        let content = std::fs::read_to_string(&process.log_path)
            .map_err(|e| format!("Cannot read log: {}", e))?;
        let lines: Vec<&str> = content.lines().collect();
        let n = tail_lines.unwrap_or(100);
        let start = if lines.len() > n { lines.len() - n } else { 0 };
        Ok(lines[start..].join("\n"))
    } else {
        Err(format!("Bot '{}' not found", bot_name))
    }
}
