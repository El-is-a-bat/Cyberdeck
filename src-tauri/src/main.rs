// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;

use gtk::prelude::*;
use gtk_layer_shell::{Edge, Layer, LayerShell};
use tauri::Manager;

fn main() {
    if let Ok(_config_guard) = config::APP_CONFIG.lock() {
        println!("App started with config");
    } else {
        println!("Failed to init config");
    }

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
                .format(|out, message, record| {
                    out.finish(format_args!("[{}] {}", record.level(), message))
                })
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
            let _ = main_webview.hide();

            let gtk_window = gtk::ApplicationWindow::new(
                &main_webview.gtk_window().unwrap().application().unwrap(),
            );
            // let gtk_window = main_webview.gtk_window().unwrap();

            let current_monitor_size = match main_webview.current_monitor() {
                Ok(Some(monitor)) => *monitor.size(),
                Ok(None) => tauri::PhysicalSize::new(1920, 1080),
                Err(_e) => tauri::PhysicalSize::new(1920, 1080),
            };

            let vbox = main_webview.default_vbox().unwrap();
            main_webview.gtk_window().unwrap().remove(&vbox);
            gtk_window.add(&vbox);

            gtk_window.init_layer_shell();

            let vbox = main_webview.default_vbox().unwrap();
            main_webview.gtk_window().unwrap().remove(&vbox);
            gtk_window.add(&vbox);

            gtk_window.set_app_paintable(true);

            gtk_window.set_layer(Layer::Top);

            gtk_window.set_anchor(Edge::Top, true);
            gtk_window.set_anchor(Edge::Left, true);

            gtk_window.set_decorated(false);
            // setting this to false makes window float
            // TODO find better way to do this
            // for now I will use hyprland windowrules((
            gtk_window.set_resizable(true);

            gtk_window.set_width_request(current_monitor_size.width.try_into().unwrap());
            gtk_window.set_height_request(current_monitor_size.height.try_into().unwrap());

            gtk_window.set_can_focus(true);
            gtk_window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::OnDemand);
            gtk_window.show_all();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::exit,
            commands::start_program,
            commands::get_desktop_applications,
            commands::is_dev,
            commands::try_get_cached_applications,
            config::get_slayfi_config,
            config::get_client_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
