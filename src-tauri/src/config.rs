use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlayfiConfig {
    pub apps_per_page: u16,
    pub terminal_app: String,
    pub desktop_environment: String,
    pub kde_icon_theme: String,
}

impl Default for SlayfiConfig {
    fn default() -> Self {
        SlayfiConfig {
            apps_per_page: 5,
            terminal_app: "kitty".to_string(),
            desktop_environment: "Hyprland".to_string(),
            kde_icon_theme: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/ClientConfig.ts")]
pub struct SlayfiClientConfig {
    pub apps_per_page: u16,
}

pub static APP_CONFIG: Lazy<Mutex<SlayfiConfig>> = Lazy::new(|| {
    let config = load_or_create_config().unwrap_or_else(|e| -> SlayfiConfig {
        error!("Failed to load config: {e}");
        info!("Using default config");
        SlayfiConfig::default()
    });
    Mutex::new(config)
});

fn get_slayfi_config_path() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|e| format!("HOME environment variable not set: {e}"))?;
    let path = format!("{home}/.config/slayfi/config.json");
    Ok(PathBuf::from(path))
}

pub fn load_or_create_config() -> Result<SlayfiConfig, String> {
    let config_path = get_slayfi_config_path()?;
    info!("Attempting to load config from {config_path:?}");
    if config_path.exists() {
        let config_string = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to parse config at {config_path:?}: {e}"))?;
        serde_json::from_str(&config_string)
            .map_err(|e| format!("Failed to parse config from JSON at {config_path:?}: {e}"))
    } else {
        let mut default_config = SlayfiConfig::default();
        default_config.kde_icon_theme = match get_kde_icon_theme() {
            Some(icon_theme) => icon_theme,
            None => default_config.kde_icon_theme,
        };

        println!("Config file does not exist. Creating default at {config_path:?}",);

        if let Some(parent) = config_path.parent() {
            match fs::create_dir_all(parent) {
                Ok(path) => info!("Created config directory: {path:?}"),
                Err(e) => error!("Error while creating config directory: {e}"),
            }
        }
        match fs::write(
            config_path,
            serde_json::to_string_pretty(&default_config).expect("Failed to serialize struct"),
        ) {
            Ok(_) => info!("Config written to file succesfully"),
            Err(e) => error!("Error while writing to config file {e}"),
        }
        Ok(default_config)
    }
}

fn get_kde_icon_theme() -> Option<String> {
    let output_result = std::process::Command::new("kreadconfig5")
        .args(["--file", "kdeglobals", "--group", "Icons", "--key", "Theme"])
        .output();
    match output_result {
        Ok(output) => {
            if output.status.success() {
                let theme = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !theme.is_empty() {
                    Some(theme)
                } else {
                    #[cfg(debug_assertions)]
                    error!("kreadconfig5 returned empty theme");
                    None
                }
            } else {
                #[cfg(debug_assertions)]
                error!(
                    "kreadconfig5 failed: {stderr}",
                    stderr = String::from_utf8_lossy(&output.stderr)
                );
                None
            }
        }
        Err(_e) => {
            #[cfg(debug_assertions)]
            error!("Failed to execute kreadconfig5: {_e}");
            None
        }
    }
}

#[tauri::command]
pub fn get_slayfi_config() -> Result<SlayfiConfig, String> {
    let config_guard = APP_CONFIG
        .lock()
        .map_err(|e| format!("Failed to lock config {e}"))?;
    Ok(config_guard.clone())
}

#[tauri::command]
pub fn get_client_config() -> Result<SlayfiClientConfig, String> {
    Ok(SlayfiClientConfig {
        apps_per_page: APP_CONFIG
            .lock()
            .map_err(|e| format!("Failed to lock config: {e}"))?
            .apps_per_page,
    })
}
