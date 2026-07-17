mod commands;
mod commands_m1;
pub mod commands_m2;
pub mod db;
pub mod error;
pub mod ops;
pub mod rules;
pub mod scan;
pub mod sidecar;
mod watch;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
}

/// (Re)start the FS watcher on the given root.
pub fn restart_watcher(app: &tauri::AppHandle, root: PathBuf) {
    let state = app.state::<AppState>();
    let db_path = state.db_path.clone();
    let handle = app.state::<watch::WatcherHandle>();
    match watch::start(app.clone(), db_path, root) {
        Ok(w) => {
            *handle.0.lock().unwrap() = Some(w);
        }
        Err(e) => tracing::warn!("could not start watcher: {e}"),
    }
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
            let db_path = data_dir.join("hangar.db");
            let conn = db::open(&db_path)
                .map_err(|e| format!("failed to open database: {e}"))?;
            let root = db::get_setting(&conn, "root").ok().flatten();
            app.manage(AppState {
                conn: Mutex::new(conn),
                db_path,
            });
            app.manage(watch::WatcherHandle(Mutex::new(None)));
            if std::env::var("HANGAR_NO_WATCH").is_err() {
                if let Some(root) = root {
                    restart_watcher(app.handle(), PathBuf::from(root));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_root,
            commands::default_root,
            commands::set_root,
            commands::rescan,
            commands::list_projects,
            commands::create_project,
            commands_m1::get_project,
            commands_m1::list_files,
            commands_m1::rename_file,
            commands_m1::move_files,
            commands_m1::trash_files,
            commands_m1::toggle_pin_file,
            commands_m1::quick_look,
            commands_m1::create_bin,
            commands_m1::rename_bin,
            commands_m1::trash_bin,
            commands_m1::list_inbox,
            commands_m1::file_inbox,
            commands_m1::list_rules,
            commands_m1::save_rule,
            commands_m1::delete_rule,
            commands_m1::test_rule,
            commands_m1::list_logs,
            commands_m1::add_log,
            commands_m1::set_progress,
            commands_m1::update_project,
            commands_m1::search,
            commands_m1::list_ideas,
            commands_m1::create_idea,
            commands_m1::delete_idea,
            commands_m2::list_milestones,
            commands_m2::add_milestone,
            commands_m2::set_milestone_state,
            commands_m2::update_milestone,
            commands_m2::delete_milestone,
            commands_m2::apply_milestone_template,
            commands_m2::set_progress_mode,
            commands_m2::list_tasks,
            commands_m2::add_task,
            commands_m2::toggle_task,
            commands_m2::update_task,
            commands_m2::delete_task,
            commands_m2::get_progress_stats,
            commands_m2::draft_status_report,
            commands_m2::list_orders,
            commands_m2::add_order,
            commands_m2::update_order_status,
            commands_m2::delete_order,
            commands_m2::spend_summary,
            commands_m2::list_links,
            commands_m2::add_link,
            commands_m2::delete_link,
            commands_m2::git_badge,
            commands_m2::start_timer,
            commands_m2::stop_timer,
            commands_m2::active_timer,
            commands_m2::today_data,
            commands_m2::portfolio,
            commands_m2::health_rollup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
