use aho_corasick::AhoCorasick;
use chrono::Local;
use freedesktop_file_parser::{EntryType, LocaleString};
#[cfg(debug_assertions)]
use log::{debug, info};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tauri::{Manager, PhysicalSize, Size};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub comment: String,
    pub icon: String,
    pub exec: String,
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
    let shell_cmd = format!("nohup {} > /dev/null 2>&1 &", exec);

    match Command::new("sh").arg("-c").arg(shell_cmd).spawn() {
        Ok(_) => {
            #[cfg(debug_assertions)]
            info!("Successfully started program: {}", exec);
            app_handle.exit(0);
            true
        }
        Err(e) => {
            error!("Failed to start program {}: {}", exec, e);
            false
        }
    }
}

#[tauri::command]
pub fn list_desktop_applications() -> Vec<Application> {
    let applications_path = "/usr/share/applications/";
    let mut applications: Vec<Application> = vec![];

    // get current desktop environment
    let desktop_environment =
        env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| String::from("Hyprland"));
    let terminal_app = "kitty";
    // for manually searching for some KDE icons, as the freedesktop_file_parser chooses
    // the "hicolor" theme by default.
    let kde_icon_theme = get_kde_icon_theme().unwrap_or_else(|| String::from(""));
    #[cfg(debug_assertions)]
    {
        info!("Current desktop environment: {}", desktop_environment);
        info!("Current default terminal: {}", terminal_app);
        info!("Current KDE icon theme: {}", kde_icon_theme);
    }

    for entry in WalkDir::new(applications_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path().to_string_lossy();
        #[cfg(debug_assertions)]
        debug!("Processing: {}", file_path);

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(content) => content,
            Err(e) => {
                log_to_file(&format!("Error reading file {}: {}", file_path, e));
                error!("Error reading file {}: {}", file_path, e);
                continue;
            }
        };

        // extract only the [Desktop Entry] section
        let desktop_entry_content = match content.split("[Desktop Entry]").nth(1) {
            Some(section) => {
                // Find the next section header or end of file
                let next_section = section.find("\n[").unwrap_or(section.len());
                format!("[Desktop Entry]{}", &section[..next_section])
            }
            None => {
                log_to_file(&format!(
                    "No [Desktop Entry] section found in {}",
                    file_path
                ));
                #[cfg(debug_assertions)]
                debug!("No [Desktop Entry] section found in {}", file_path);
                continue;
            }
        };

        let desktop_file = match freedesktop_file_parser::parse(&desktop_entry_content) {
            Ok(parsed) => parsed,
            Err(e) => {
                log_to_file(&format!("Error parsing desktop file {}: {}", file_path, e));
                error!("Error parsing desktop file {}: {}", file_path, e);
                continue;
            }
        };

        let desktop_entry = desktop_file.entry;

        // skip if not an application entry
        if let EntryType::Application(application) = &desktop_entry.entry_type {
            // skip if no exec field
            let app_exec = match application.exec.clone() {
                Some(exec) => {
                    let cleaned = clean_exec_command(exec, &desktop_entry.name.default);
                    match application.terminal {
                        Some(is_terminal) => {
                            if is_terminal {
                                format!("{} {}", terminal_app, cleaned)
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
                        log_to_file(&format!(
                            "Skipping {}: No exec field",
                            desktop_entry.name.default
                        ));
                        debug!("Skipping {}: No exec field", desktop_entry.name.default);
                    }
                    continue;
                }
            };

            // skip if application is hidden or shouldn't be displayed
            if desktop_entry.hidden.unwrap_or(false) || desktop_entry.no_display.unwrap_or(false) {
                #[cfg(debug_assertions)]
                {
                    log_to_file(&format!(
                        "Skipping {}: Hidden or no display",
                        desktop_entry.name.default
                    ));
                    debug!(
                        "Skipping {}: Hidden or no display",
                        desktop_entry.name.default
                    );
                }
                continue;
            }

            // check if application should be shown in current desktop environment
            let only_show_in = desktop_entry.only_show_in.unwrap_or_default();
            let not_show_in = desktop_entry.not_show_in.unwrap_or_default();

            if !only_show_in.is_empty() && !only_show_in.contains(&desktop_environment) {
                #[cfg(debug_assertions)]
                {
                    log_to_file(&format!(
                        "Skipping {}: Not compatible with current desktop environment",
                        desktop_entry.name.default
                    ));
                    debug!(
                        "Skipping {}: Not compatible with current desktop environment",
                        desktop_entry.name.default
                    );
                }
                continue;
            }

            if not_show_in.contains(&desktop_environment) {
                #[cfg(debug_assertions)]
                {
                    log_to_file(&format!(
                        "Skipping {}: Explicitly not shown in current desktop environment",
                        desktop_entry.name.default
                    ));
                    debug!(
                        "Skipping {}: Explicitly not shown in current desktop environment",
                        desktop_entry.name.default
                    );
                }
                continue;
            }

            let app = Application {
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
                            {
                                log_to_file(&format!(
                                    "No icon path found for {}",
                                    desktop_entry.name.default
                                ));
                                warn!("No icon path found for {}", desktop_entry.name.default);
                            }
                            if !kde_icon_theme.is_empty() {
                                match freedesktop_icons::lookup(&icon.content)
                                    .with_size(48)
                                    .with_theme(&kde_icon_theme)
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
                        log_to_file(&format!("No icon found for {}", desktop_entry.name.default));
                        warn!("No icon found for {}", desktop_entry.name.default);
                        String::from("")
                    }
                },
                exec: app_exec,
            };

            #[cfg(debug_assertions)]
            debug!("Added application: {}", app);
            applications.push(app);
        } else {
            #[cfg(debug_assertions)]
            {
                log_to_file(&format!("Skipping {}: Not an application entry", file_path));
                debug!("Skipping {}: Not an application entry", file_path);
            }
            continue;
        }
    }

    #[cfg(debug_assertions)]
    info!("Total applications found: {}", applications.len());
    match cache_apps(&applications) {
        Ok(()) => {
            #[cfg(debug_assertions)]
            info!("Applications cached successfully")
        }
        Err(e) => {
            error!("Error occurred when writing cache to file: {}", e)
        }
    }

    applications
}

#[tauri::command]
pub fn get_config() -> String {
    "{\"apps_per_page\": 5}".into()
}

#[tauri::command]
pub fn set_application_size(app_handle: tauri::AppHandle, height: u32, width: u32) {
    if let Some(window) = app_handle.get_window("main") {
        let _ = window.set_size(Size::Physical(PhysicalSize { width, height }));
    }
    println!("Width: {}", width);
    println!("Height: {}", height);
}

#[tauri::command]
pub fn try_get_cached_applications() -> Vec<Application> {
    match read_cached_apps() {
        Ok(apps) => apps,
        Err(e) => {
            log_to_file(&format!("Erroe while reading cached apps: {}", e));
            error!("Error while reading cached apps: {}", e);

            list_desktop_applications()
        }
    }
}

#[tauri::command]
pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

fn log_to_file(message: &str) {
    let error_log_path = format!(
        "{}/.local/share/slayfi/error.log",
        env::var("HOME").unwrap_or_else(|_| String::from("/home"))
    );
    if let Ok(mut error_log) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&error_log_path)
    {
        let error_msg = format!(
            "[{}] {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            message
        );
        let _ = error_log.write_all(error_msg.as_bytes());
    }
}

fn clean_exec_command(exec: String, app_name: &str) -> String {
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

fn get_kde_icon_theme() -> Option<String> {
    let output_result = Command::new("kreadconfig5")
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
                    "kreadconfig5 failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                None
            }
        }
        Err(_e) => {
            #[cfg(debug_assertions)]
            error!("Failed to execute kreadconfig5: {}", _e);
            None
        }
    }
}

pub fn cache_apps(apps: &Vec<Application>) -> std::io::Result<()> {
    let cache_path = format!(
        "{}/.local/share/slayfi/apps_cache.json",
        env::var("HOME").unwrap_or_else(|_| String::from("/home"))
    );
    if let Some(parent) = Path::new(&cache_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(apps)
        .map_err(|e| std::io::Error::other(format!("Serialization error: {}", e)))?;
    fs::write(cache_path, json)
}

pub fn read_cached_apps() -> std::io::Result<Vec<Application>> {
    let cache_path = format!(
        "{}/.local/share/slayfi/apps_cache.json",
        env::var("HOME").unwrap_or_else(|_| String::from("/home"))
    );
    let data = fs::read_to_string(cache_path)?;
    let apps: Vec<Application> = serde_json::from_str(&data)
        .map_err(|e| std::io::Error::other(format!("Deserialization error: {}", e)))?;
    Ok(apps)
}
