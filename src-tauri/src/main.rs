// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use gtk::prelude::*;
use tauri::Manager;

fn main() {
    let mut builder = tauri::Builder::default();

    #[cfg(debug_assertions)]
    {
        let devtools = tauri_plugin_devtools::init();
        builder = builder.plugin(devtools);
    }

    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(50000 /* bytes */)
                .filter(|metadata| metadata.target().starts_with("slayfi"))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("logs".to_string()),
                    },
                ))
                .build(),
        );
    }

    builder
        .setup(|app| {
            // setting up gtk window
            let main_webview = app.get_webview_window("main").unwrap();
            let gtk_window = main_webview.gtk_window().unwrap();

            gtk_window.set_decorated(false);
            // setting this to false makes window float
            // TODO find better way to do this
            // for now I will use hyprland windowrules((
            gtk_window.set_resizable(true);

            gtk_window.set_width_request(1920);
            gtk_window.set_height_request(1080);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::exit,
            commands::start_program,
            commands::get_config,
            commands::set_application_size,
            commands::get_desktop_applications,
            commands::is_dev,
            commands::try_get_cached_applications
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
