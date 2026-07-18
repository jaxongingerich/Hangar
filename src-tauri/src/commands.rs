use crate::db;
use crate::error::{AppError, AppResult};
use crate::scan::{self, ScanStats};
use crate::sidecar::{append_log_md, Sidecar};
use crate::AppState;
use rusqlite::params;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProjectCard {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub path: String,
    pub emoji: String,
    pub color: String,
    pub status: String,
    pub progress: i64,
    pub pinned: bool,
    pub target_date: Option<String>,
    pub file_count: i64,
    pub size_bytes: i64,
    pub last_touch_ms: Option<i64>,
    /// Files modified per day, oldest→today, 14 entries.
    pub spine: Vec<i64>,
}

#[tauri::command]
pub fn get_root(state: State<AppState>) -> AppResult<Option<String>> {
    let conn = state.conn.lock().unwrap();
    db::get_setting(&conn, "root")
}

#[tauri::command]
pub fn default_root() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join("Projects")
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
pub fn set_root(
    app: tauri::AppHandle,
    state: State<AppState>,
    path: String,
) -> AppResult<ScanStats> {
    let root = PathBuf::from(&path);
    std::fs::create_dir_all(&root)?;
    let stats = {
        let mut conn = state.conn.lock().unwrap();
        db::set_setting(&conn, "root", &path)?;
        scan::scan(&mut conn, &root)?
    };
    crate::restart_watcher(&app, root);
    Ok(stats)
}

#[tauri::command]
pub fn rescan(state: State<AppState>) -> AppResult<ScanStats> {
    let mut conn = state.conn.lock().unwrap();
    let root = db::get_setting(&conn, "root")?
        .ok_or_else(|| AppError::msg("no root configured"))?;
    scan::scan(&mut conn, &PathBuf::from(root))
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> AppResult<Vec<ProjectCard>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT p.id, p.slug, p.name, p.path, p.emoji, p.color, p.status, p.progress,
                p.pinned, p.target_date,
                COUNT(f.id), COALESCE(SUM(f.size), 0), MAX(f.mtime)
         FROM projects p
         LEFT JOIN files f ON f.project_id = p.id
         WHERE p.status != 'archived'
         GROUP BY p.id
         ORDER BY p.pinned DESC, MAX(f.mtime) DESC NULLS LAST",
    )?;
    let mut cards: Vec<ProjectCard> = stmt
        .query_map([], |r| {
            Ok(ProjectCard {
                id: r.get(0)?,
                slug: r.get(1)?,
                name: r.get(2)?,
                path: r.get(3)?,
                emoji: r.get(4)?,
                color: r.get(5)?,
                status: r.get(6)?,
                progress: r.get(7)?,
                pinned: r.get::<_, i64>(8)? != 0,
                target_date: r.get(9)?,
                file_count: r.get(10)?,
                size_bytes: r.get(11)?,
                last_touch_ms: r.get(12)?,
                spine: vec![],
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Activity spine: files modified per day over the last 14 days.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let day_ms: i64 = 86_400_000;
    let cutoff = now_ms - 14 * day_ms;
    let mut spine_stmt =
        conn.prepare("SELECT mtime FROM files WHERE project_id = ?1 AND mtime > ?2")?;
    for card in &mut cards {
        let mut buckets = vec![0i64; 14];
        let mtimes = spine_stmt.query_map(params![card.id, cutoff], |r| r.get::<_, i64>(0))?;
        for mt in mtimes.filter_map(|r| r.ok()) {
            let age_days = ((now_ms - mt) / day_ms).clamp(0, 13) as usize;
            buckets[13 - age_days] += 1;
        }
        card.spine = buckets;
    }
    Ok(cards)
}

// Default bins are intentionally general — every project gets these. Specialized
// hardware bins (Gerbers, BOM, JLCPCB…) are one click away via "New bin" or the
// Hardware template, so projects don't start cluttered with folders they may
// never use.
const GENERAL_BINS: &[&str] = &["Docs", "Files", "Photos", "Notes", "Exports"];
const HARDWARE_BINS: &[&str] = &[
    "Docs", "Files", "Photos", "Firmware", "CAD", "Gerbers", "BOM",
];
const SOFTWARE_BINS: &[&str] = &["Docs", "Design", "Assets", "Research", "Exports"];

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    name: String,
    template: Option<String>,
) -> AppResult<ScanStats> {
    let mut conn = state.conn.lock().unwrap();
    let root = db::get_setting(&conn, "root")?
        .ok_or_else(|| AppError::msg("no root configured"))?;
    let root = PathBuf::from(root);

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::msg("project name cannot be empty"));
    }
    let dir = root.join(trimmed);
    if dir.exists() {
        return Err(AppError::msg(format!("\"{}\" already exists", trimmed)));
    }
    std::fs::create_dir_all(&dir)?;

    let bins: &[&str] = match template.as_deref() {
        Some("hardware") => HARDWARE_BINS,
        Some("software") => SOFTWARE_BINS,
        Some("mixed") => &[
            "Docs", "Files", "Photos", "Notes", "Exports",
            "Firmware", "CAD", "Gerbers", "BOM", "Design", "Assets",
        ],
        // "general" and anything unspecified get the clean, general set.
        _ => GENERAL_BINS,
    };
    for bin in bins {
        std::fs::create_dir_all(dir.join(bin))?;
    }
    Sidecar::new(trimmed).save(&dir)?;
    append_log_md(
        &dir,
        &format!(
            "- {} · created project \"{}\" ({} template)",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            trimmed,
            template.as_deref().unwrap_or("hardware")
        ),
    )?;
    scan::scan(&mut conn, &root)
}
