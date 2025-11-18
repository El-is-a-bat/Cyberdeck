use crate::config;
use aho_corasick::AhoCorasick;
use freedesktop_file_parser::{EntryType, LocaleString};
#[cfg(debug_assertions)]
use log::{debug, info};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use ts_rs::TS;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/Application.ts")]
pub struct Application {
    pub name: String,
    pub comment: String,
    pub icon: String,
    pub exec: String,
    // pub launches_count: u32,
}

impl std::fmt::Display for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\n\tName:\t{},\n\tComment:\t{},\n\tIcon:\t{},\n\tExec:\t{}\n",
            self.name, self.comment, self.icon, self.exec
        )
    }
}

#[tauri::command]
pub fn exit(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
pub fn start_program(app_handle: tauri::AppHandle, exec: String) -> bool {
    // use nohup to detach the process and redirect output
    let shell_cmd = format!("nohup {exec} > /dev/null 2>&1 &");

    match Command::new("sh").arg("-c").arg(shell_cmd).spawn() {
        Ok(_) => {
            #[cfg(debug_assertions)]
            info!("Successfully started program: {exec}");
            app_handle.exit(0);
            true
        }
        Err(e) => {
            error!("Failed to start program {exec}: {e}");
            false
        }
    }
}

#[tauri::command]
pub async fn get_desktop_applications() -> Vec<Application> {
    let mut applications: Vec<Application> = vec![];

    let config_guard = match config::APP_CONFIG.lock() {
        Ok(conf) => conf.clone(),
        Err(e) => {
            error!("Error while locking config: {e}");
            return applications;
        }
    };

    let applications_paths = &config_guard.lookup_dirs;
    // get current desktop environment
    let desktop_environment = &config_guard.desktop_environment;
    // env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| String::from("Hyprland"));
    let terminal_app = &config_guard.terminal_app;
    // for manually searching for some KDE icons, as the freedesktop_file_parser chooses
    // the "hicolor" theme by default.
    let kde_icon_theme = &config_guard.kde_icon_theme;
    #[cfg(debug_assertions)]
    {
        info!("Current desktop environment: {desktop_environment}");
        info!("Current default terminal: {terminal_app}");
        info!("Current KDE icon theme: {kde_icon_theme}");
    }

    for applications_path in applications_paths {
        for entry in WalkDir::new(applications_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let file_path = entry.path().to_string_lossy();
            if !file_path.ends_with(".desktop") {
                #[cfg(debug_assertions)]
                debug!("Skipping '{file_path}': not a desktop file");
                continue;
            }

            #[cfg(debug_assertions)]
            debug!("Processing: {file_path}");

            match parse_application_from_file(
                file_path.to_string(),
                desktop_environment,
                terminal_app,
                kde_icon_theme,
            )
            .await
            {
                Some(parsed_app) => {
                    #[cfg(debug_assertions)]
                    debug!("Adding application: {parsed_app}");
                    applications.push(parsed_app);
                }
                None => continue,
            };
        }
    }

    #[cfg(debug_assertions)]
    info!(
        "Total applications found: {count}",
        count = applications.len()
    );
    match cache_apps(&applications).await {
        Ok(()) => {
            #[cfg(debug_assertions)]
            info!("Applications cached successfully")
        }
        Err(e) => {
            error!("Error occurred when writing cache to file: {e}")
        }
    }

    applications
}

#[tauri::command]
pub async fn try_get_cached_applications() -> Option<Vec<Application>> {
    match read_cached_apps().await {
        Ok(apps) => {
            #[cfg(debug_assertions)]
            info!("Successfully read cached applications");
            Some(apps)
        }
        Err(e) => {
            error!("Error while reading cached apps: {e}");
            None
        }
    }
}

#[tauri::command]
pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

async fn parse_application_from_file(
    file_path: String,
    desktop_environment: &String,
    terminal_app: &String,
    kde_icon_theme: &str,
) -> Option<Application> {
    let content = match std::fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            error!("Error reading file {file_path}: {e}");
            return None;
        }
    };

    // extract only the [Desktop Entry] section
    // upd: so far this is needed only for realvnc-vncviewer.desktop
    // because of `Error: Repetitive declaration of key "Name" and or entry or action`
    let desktop_entry_content = match content.split("[Desktop Entry]").nth(1) {
        Some(section) => {
            // find the next section header or end of file
            let next_section = section.find("\n[").unwrap_or(section.len());
            format!("[Desktop Entry]{}", &section[..next_section])
        }
        None => {
            #[cfg(debug_assertions)]
            debug!("No [Desktop Entry] section found in {file_path}");
            return None;
        }
    };

    let desktop_file = match freedesktop_file_parser::parse(&desktop_entry_content) {
        Ok(parsed) => parsed,
        Err(e) => {
            error!("Error parsing desktop file {file_path}: {e}");
            return None;
        }
    };

    let desktop_entry = desktop_file.entry;

    // skip if not an application entry
    if let EntryType::Application(application) = &desktop_entry.entry_type {
        // skip if no exec field
        let app_exec = match application.exec.clone() {
            Some(exec) => {
                let cleaned = clean_exec_command(exec, &desktop_entry.name.default).await;
                match application.terminal {
                    Some(is_terminal) => {
                        if is_terminal {
                            format!("{terminal_app} {cleaned}")
                        } else {
                            cleaned
                        }
                    }
                    None => cleaned,
                }
            }
            None => {
                #[cfg(debug_assertions)]
                {
                    debug!(
                        "Skipping {app_name}: No exec field",
                        app_name = desktop_entry.name.default
                    );
                }
                return None;
            }
        };

        // skip if application is hidden or shouldn't be displayed
        if desktop_entry.hidden.unwrap_or(false) || desktop_entry.no_display.unwrap_or(false) {
            #[cfg(debug_assertions)]
            {
                debug!(
                    "Skipping {app_name}: Hidden or no display",
                    app_name = desktop_entry.name.default
                );
            }
            return None;
        }

        // check if application should be shown in current desktop environment
        let only_show_in = desktop_entry.only_show_in.unwrap_or_default();
        let not_show_in = desktop_entry.not_show_in.unwrap_or_default();

        if !only_show_in.is_empty() && !only_show_in.contains(desktop_environment) {
            #[cfg(debug_assertions)]
            {
                debug!(
                    "Skipping {app_name}: Not compatible with current desktop environment",
                    app_name = desktop_entry.name.default
                );
            }
            return None;
        }

        if not_show_in.contains(desktop_environment) {
            #[cfg(debug_assertions)]
            {
                debug!(
                    "Skipping {app_name}: Explicitly not shown in current desktop environment",
                    app_name = desktop_entry.name.default
                );
            }
            return None;
        }

        Some(Application {
            name: desktop_entry.name.default.clone(),
            comment: desktop_entry
                .comment
                .unwrap_or(LocaleString {
                    default: String::from(""),
                    variants: HashMap::new(),
                })
                .default,
            icon: match desktop_entry.icon {
                Some(icon) => match icon.get_icon_path() {
                    Some(path) => path.to_string_lossy().into_owned(),
                    None => {
                        #[cfg(debug_assertions)]
                        warn!(
                            "No icon path found for {app_name}",
                            app_name = desktop_entry.name.default
                        );
                        if !kde_icon_theme.is_empty() {
                            match freedesktop_icons::lookup(&icon.content)
                                .with_size(48)
                                .with_theme(kde_icon_theme)
                                .find()
                            {
                                Some(icon_path) => icon_path.to_string_lossy().into_owned(),
                                None => String::from(""),
                            }
                        } else {
                            String::from("")
                        }
                    }
                },
                None => {
                    #[cfg(debug_assertions)]
                    warn!(
                        "No icon found for {app_name}",
                        app_name = desktop_entry.name.default
                    );
                    String::from("")
                }
            },
            exec: app_exec,
        })
    } else {
        #[cfg(debug_assertions)]
        {
            debug!("Skipping {file_path}: Not an application entry");
        }
        None
    }
}

async fn clean_exec_command(exec: String, app_name: &str) -> String {
    // is a separate function because at first I decided to remove some args,
    // like `%U` and `%f`, but then decided not to use them at all because of
    // some weird results like:
    //      `vlc --started-from-file` or
    //      `cursor --no-sandbox`
    // (both without `%U` at the end)

    // update 2: WinBox has some weird exec => `/usr/bin/env --unset=QT_QPA_PLATFORM /usr/bin/Winbox`
    // so getting the first split element (to skip args) won't help
    //
    // this function now splits the exec string by whitespace, then iterates through the elements;
    // when an element (case-insensitive, basename only) matches the app name, it includes all
    // elements up to and including that one, and ignores the rest.
    let parts: Vec<&str> = exec.split_whitespace().collect();
    let app_name_lower = app_name.to_lowercase();
    let mut result = Vec::new();
    for part in &parts {
        result.push(*part);
        let base = Path::new(part)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase());
        if let Some(base) = base {
            if base == app_name_lower {
                break;
            }
        }
    }

    // update 3
    // some apps have names which different from their exec (for example "VNC viever" is
    // `vncviewer` and "LibreOffice Calc" is `libreoffice --calc`)
    if parts.len() == result.len() {
        let params = ["%U", "%u", "%F", "%f", "%i", "%c", "%k"];
        let replacements = &[""; 7];
        let ac = AhoCorasick::new(params).unwrap();
        let cleaned = ac.replace_all(&exec, replacements);
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        result.join(" ")
    }
}

pub async fn cache_apps(apps: &Vec<Application>) -> std::io::Result<()> {
    let cache_path = format!(
        "{}/.local/share/slayfi/apps_cache.json",
        env::var("HOME").unwrap_or_else(|_| String::from("/home"))
    );
    if let Some(parent) = Path::new(&cache_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(apps)
        .map_err(|e| std::io::Error::other(format!("Serialization error: {e}")))?;
    fs::write(cache_path, json)
}

pub async fn read_cached_apps() -> std::io::Result<Vec<Application>> {
    let cache_path = format!(
        "{}/.local/share/slayfi/apps_cache.json",
        env::var("HOME").unwrap_or_else(|_| String::from("/home"))
    );
    let data = fs::read_to_string(cache_path)?;
    let apps: Vec<Application> = serde_json::from_str(&data)
        .map_err(|e| std::io::Error::other(format!("Deserialization error: {e}")))?;
    Ok(apps)
}
