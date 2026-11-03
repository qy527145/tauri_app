mod commands;
mod config;
mod error;
mod logging;
mod state;

#[cfg(desktop)]
use tauri::Manager;

use crate::config::Config;
use crate::state::AppState;

/// 主窗口 label，与 `tauri.conf.json` 和 `capabilities/default.json` 保持一致。
pub const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = Config::from_env();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            // 单实例必须最先注册：重复启动时唤起已有窗口，而不是再开一个进程。
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }))
            // 记住窗口位置与尺寸，下次启动原样恢复。
            .plugin(tauri_plugin_window_state::Builder::new().build());
    }

    builder
        .plugin(logging::plugin(&config.log_filter))
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(commands::handler())
        .setup(|app| {
            let package = app.package_info();
            log::info!("{} v{} started", package.name, package.version);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
