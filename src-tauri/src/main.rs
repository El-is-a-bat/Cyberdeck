// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use gtk::prelude::*;
use log::LevelFilter;
use tauri::Manager;

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .filter_level(LevelFilter::Debug)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // setting up gtk window
            let main_webview = app.get_webview_window("main").unwrap();
            let gtk_window = main_webview.gtk_window().unwrap();

            gtk_window.set_decorated(false);
            // setting this to false makes window float
            // TODO find better way to do this
            // for now I will use hyprland windowrules((
            gtk_window.set_resizable(true);

            gtk_window.set_width_request(400);
            gtk_window.set_height_request(270);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::exit,
            commands::start_program,
            commands::get_config,
            commands::set_application_size,
            commands::list_desktop_applications,
            commands::is_dev,
            commands::try_get_cached_applications
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
