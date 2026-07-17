mod commands;
mod db;
mod error;
mod scan;
pub mod sidecar;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub conn: Mutex<Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let conn = db::open(&data_dir.join("hangar.db"))
                .map_err(|e| format!("failed to open database: {e}"))?;
            app.manage(AppState {
                conn: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_root,
            commands::default_root,
            commands::set_root,
            commands::rescan,
            commands::list_projects,
            commands::create_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
