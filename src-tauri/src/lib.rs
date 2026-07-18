mod commands;
mod commands_m1;
pub mod commands_m2;
pub mod commands_m3;
pub mod commands_m4;
pub mod commands_m5;
pub mod commands_m6;
pub mod ai;
pub mod import;
pub mod mcp;
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

/// (Re)start sweepers for every watched external folder.
pub fn restart_sweepers(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let (dirs, patterns, root) = {
        let conn = state.conn.lock().unwrap();
        let dirs: Vec<String> = db::get_setting(&conn, "watched_dirs")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        let patterns = db::get_setting(&conn, "sweep_patterns")
            .ok()
            .flatten()
            .unwrap_or_else(|| "*.zip,*.pdf,*.step,*.gbr,*.csv".into());
        let root = db::get_setting(&conn, "root").ok().flatten();
        (dirs, patterns, root)
    };
    let Some(root) = root else { return };
    let inbox = PathBuf::from(root).join(scan::INBOX_DIR);
    let handles = app.state::<watch::SweeperHandles>();
    let mut sweepers = handles.0.lock().unwrap();
    sweepers.clear();
    for dir in dirs {
        let path = PathBuf::from(&dir);
        if !path.is_dir() {
            continue;
        }
        match watch::start_sweeper(app.clone(), path, inbox.clone(), patterns.clone()) {
            Ok(w) => sweepers.push(w),
            Err(e) => tracing::warn!("could not sweep {dir}: {e}"),
        }
    }
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;
    use tauri::Emitter;

    let open = MenuItemBuilder::with_id("open", "Open Hangar").build(app)?;
    let new_idea = MenuItemBuilder::with_id("new_idea", "New idea…").build(app)?;
    let new_project = MenuItemBuilder::with_id("new_project", "New project…").build(app)?;
    let stop_timer = MenuItemBuilder::with_id("stop_timer", "Stop timer").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Hangar").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&open, &new_idea, &new_project])
        .separator()
        .item(&stop_timer)
        .separator()
        .item(&quit)
        .build()?;

    let show_main = |app: &tauri::AppHandle| {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
            let _ = win.set_focus();
        }
    };

    let mut tray = TrayIconBuilder::with_id("hangar-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Hangar");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone()).icon_as_template(false);
    }
    tray.on_menu_event(move |app, event| match event.id().as_ref() {
        "open" => show_main(app),
        "new_idea" => {
            show_main(app);
            let _ = app.emit("tray-new-idea", ());
        }
        "new_project" => {
            show_main(app);
            let _ = app.emit("tray-new-project", ());
        }
        "stop_timer" => {
            let state = app.state::<AppState>();
            let conn = state.conn.lock().unwrap();
            let _ = conn.execute(
                "UPDATE time_entries SET ended_at = datetime('now') WHERE ended_at IS NULL",
                [],
            );
            let _ = app.emit("fs-changed", ());
        }
        "quit" => app.exit(0),
        _ => {}
    })
    .build(app)?;
    Ok(())
}

#[tauri::command]
fn mcp_info(state: tauri::State<AppState>) -> crate::error::AppResult<serde_json::Value> {
    let conn = state.conn.lock().unwrap();
    let token = mcp::ensure_token(&conn)?;
    let url = format!("http://127.0.0.1:{}/mcp", mcp::MCP_PORT);
    Ok(serde_json::json!({
        "url": url,
        "token": token,
        "install_cmd": format!(
            "claude mcp add --transport http hangar {url} --header \"Authorization: Bearer {token}\""
        )
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
            app.manage(watch::SweeperHandles(Mutex::new(Vec::new())));
            if std::env::var("HANGAR_NO_WATCH").is_err() {
                if let Some(root) = root {
                    restart_watcher(app.handle(), PathBuf::from(root));
                }
                restart_sweepers(app.handle());
            }

            // MCP server so Claude Code / Desktop can drive Hangar.
            // Off by a Settings toggle for standalone installs that don't use it.
            {
                let state = app.state::<AppState>();
                let conn = state.conn.lock().unwrap();
                let enabled = db::get_setting(&conn, "mcp_enabled")
                    .ok()
                    .flatten()
                    .map(|v| v != "0")
                    .unwrap_or(true);
                if enabled {
                    if let Ok(token) = mcp::ensure_token(&conn) {
                        mcp::start(app.handle().clone(), state.db_path.clone(), token);
                    }
                }
            }

            // Menu-bar quick capture.
            setup_tray(app.handle())?;
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
            commands_m3::space_report,
            commands_m3::find_duplicates,
            commands_m3::archive_project,
            commands_m3::list_archives,
            commands_m3::restore_archive,
            commands_m3::snapshot_bin,
            commands_m3::list_snapshots,
            commands_m3::diff_snapshots,
            commands_m3::export_jlcpcb,
            commands_m3::normalize_bom,
            commands_m3::list_components,
            commands_m3::save_component,
            commands_m3::delete_component,
            commands_m3::use_component,
            commands_m3::undo_last_op,
            commands_m3::export_one_pager,
            commands_m4::get_file_note,
            commands_m4::set_file_note,
            commands_m4::noted_file_ids,
            commands_m4::save_clipboard_file,
            commands_m4::list_collections,
            commands_m4::save_collection,
            commands_m4::delete_collection,
            commands_m4::run_collection,
            commands_m4::get_watched_dirs,
            commands_m4::set_watched_dirs,
            commands_m4::get_sweep_patterns,
            commands_m4::set_sweep_patterns,
            commands_m4::get_finder_tags,
            commands_m4::set_finder_tags,
            commands_m4::backup_status,
            commands_m4::set_backup_dir,
            commands_m4::run_backup,
            commands_m4::global_timeline,
            commands_m4::read_bin_gerbers,
            commands_m4::import_files,
            commands_m5::ai_get_config,
            commands_m5::ai_set_config,
            commands_m5::ai_set_key,
            commands_m5::ai_test,
            commands_m5::ai_ollama_models,
            commands_m5::ai_usage,
            commands_m5::ai_organize_inbox,
            commands_m5::ai_summarize,
            commands_m5::ai_auto_milestones,
            commands_m5::ai_status_report,
            commands_m5::ai_weekly_digest,
            commands_m5::ai_smart_rename,
            commands_m5::ai_project_chat,
            commands_m6::ai_list_profiles,
            commands_m6::ai_save_profile,
            commands_m6::ai_delete_profile,
            commands_m6::ai_activate_profile,
            commands_m6::ai_set_profile_key,
            commands_m6::ai_list_chats,
            commands_m6::ai_new_chat,
            commands_m6::ai_update_chat,
            commands_m6::ai_delete_chat,
            commands_m6::ai_chat_history,
            commands_m6::ai_chat_send,
            commands_m6::read_text_file,
            commands_m6::suggest_imports,
            commands_m6::ai_detect_providers,
            commands_m6::ai_discover_sessions,
            commands_m6::ai_delete_imported,
            commands_m6::ai_import_sessions,
            commands_m6::ai_import_export_file,
            commands_m6::ai_cli_bridge_status,
            commands_m6::ai_install_cli_bridge,
            commands_m6::mcp_get_enabled,
            commands_m6::mcp_set_enabled,
            mcp_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
