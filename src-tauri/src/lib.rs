// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod metrics;
mod tray;

use log::{error, info};
use std::sync::Mutex;
use tauri::{Manager, WindowEvent};

pub struct AppState {
    pub machine_id: Mutex<Option<String>>,
    pub tailscale_ip: Mutex<Option<String>>,
    pub is_running: Mutex<bool>,
    pub tokens_per_second: Mutex<f64>,
    pub gpu_usage: Mutex<f64>,
    pub today_earnings: Mutex<f64>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            machine_id: Mutex::new(None),
            tailscale_ip: Mutex::new(None),
            is_running: Mutex::new(false),
            tokens_per_second: Mutex::new(0.0),
            gpu_usage: Mutex::new(0.0),
            today_earnings: Mutex::new(0.0),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("TokenMeow Client starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            info!("Setting up TokenMeow Client...");
            let state = app.state::<AppState>();
            *state.is_running.lock().unwrap() = true;

            tray::setup_tray(app)?;
            commands::start_background_tasks(app.handle().clone())?;

            info!("TokenMeow Client setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_gpu,
            commands::install_docker,
            commands::start_tailscale,
            commands::start_vllm_mlx,
            commands::start_vllm_docker,
            commands::get_metrics,
            commands::update_machine_status,
            commands::set_machine_id,
            commands::get_tailscale_ip,
            commands::test_api_endpoint,
            commands::get_app_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
