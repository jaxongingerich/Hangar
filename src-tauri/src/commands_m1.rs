use crate::db;
use crate::error::{AppError, AppResult};
use crate::ops;
use crate::rules::{rule_matches, DEFAULT_RULES};
use crate::scan;
use crate::sidecar::{append_log_md, Sidecar};
use crate::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

fn root_of(conn: &Connection) -> AppResult<PathBuf> {
    Ok(PathBuf::from(
        db::get_setting(conn, "root")?.ok_or_else(|| AppError::msg("no root configured"))?,
    ))
}

// ---------- Project detail ----------

#[derive(Serialize)]
pub struct BinInfo {
    pub id: i64,
    pub name: String,
    pub rel_path: String,
    pub file_count: i64,
    pub size_bytes: i64,
}

#[derive(Serialize)]
pub struct ProjectDetail {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub emoji: String,
    pub color: String,
    pub status: String,
    pub progress: i64,
    pub progress_mode: String,
    pub target_date: Option<String>,
    pub pinned: bool,
    pub bins: Vec<BinInfo>,
    pub root_file_count: i64,
}

#[tauri::command]
pub fn get_project(state: State<AppState>, id: i64) -> AppResult<ProjectDetail> {
    let conn = state.conn.lock().unwrap();
    let mut detail = conn.query_row(
        "SELECT id, name, path, emoji, color, status, progress, progress_mode,
                target_date, pinned
         FROM projects WHERE id = ?1",
        [id],
        |r| {
            Ok(ProjectDetail {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                emoji: r.get(3)?,
                color: r.get(4)?,
                status: r.get(5)?,
                progress: r.get(6)?,
                progress_mode: r.get(7)?,
                target_date: r.get(8)?,
                pinned: r.get::<_, i64>(9)? != 0,
                bins: vec![],
                root_file_count: 0,
            })
        },
    )?;
    let mut stmt = conn.prepare(
        "SELECT b.id, b.name, b.rel_path,
                COUNT(f.id), COALESCE(SUM(f.size), 0)
         FROM bins b LEFT JOIN files f ON f.bin_id = b.id
         WHERE b.project_id = ?1
         GROUP BY b.id ORDER BY b.sort_order, b.name",
    )?;
    detail.bins = stmt
        .query_map([id], |r| {
            Ok(BinInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                rel_path: r.get(2)?,
                file_count: r.get(3)?,
                size_bytes: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    detail.root_file_count = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE project_id = ?1 AND bin_id IS NULL",
        [id],
        |r| r.get(0),
    )?;
    Ok(detail)
}

// ---------- Files ----------

#[derive(Serialize)]
pub struct FileRow {
    pub id: i64,
    pub bin_id: Option<i64>,
    pub rel_path: String,
    pub name: String,
    pub ext: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub pinned: bool,
    pub abs_path: String,
}

#[tauri::command]
pub fn list_files(
    state: State<AppState>,
    project_id: i64,
    bin_id: Option<i64>,
    root_only: Option<bool>,
) -> AppResult<Vec<FileRow>> {
    let conn = state.conn.lock().unwrap();
    let proj_path = ops::project_path(&conn, project_id)?;
    let (sql, param): (&str, Option<i64>) = if let Some(bid) = bin_id {
        (
            "SELECT id, bin_id, rel_path, name, ext, size, mtime, pinned
             FROM files WHERE project_id = ?1 AND bin_id = ?2
             ORDER BY pinned DESC, name COLLATE NOCASE",
            Some(bid),
        )
    } else if root_only.unwrap_or(false) {
        (
            "SELECT id, bin_id, rel_path, name, ext, size, mtime, pinned
             FROM files WHERE project_id = ?1 AND bin_id IS NULL
             ORDER BY pinned DESC, name COLLATE NOCASE",
            None,
        )
    } else {
        (
            "SELECT id, bin_id, rel_path, name, ext, size, mtime, pinned
             FROM files WHERE project_id = ?1
             ORDER BY pinned DESC, name COLLATE NOCASE",
            None,
        )
    };
    let mut stmt = conn.prepare(sql)?;
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<FileRow> {
        let rel: String = r.get(2)?;
        Ok(FileRow {
            id: r.get(0)?,
            bin_id: r.get(1)?,
            rel_path: rel.clone(),
            name: r.get(3)?,
            ext: r.get(4)?,
            size: r.get(5)?,
            mtime: r.get(6)?,
            pinned: r.get::<_, i64>(7)? != 0,
            abs_path: proj_path.join(&rel).to_string_lossy().to_string(),
        })
    };
    let rows = match param {
        Some(p) => stmt
            .query_map(params![project_id, p], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
        None => stmt
            .query_map([project_id], map_row)?
            .filter_map(|r| r.ok())
            .collect(),
    };
    Ok(rows)
}

#[tauri::command]
pub fn rename_file(state: State<AppState>, file_id: i64, new_name: String) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    ops::rename_file(&conn, &root, file_id, &new_name)
}

#[tauri::command]
pub fn move_files(
    state: State<AppState>,
    file_ids: Vec<i64>,
    dest_bin_id: Option<i64>,
) -> AppResult<usize> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    ops::move_files(&conn, &root, &file_ids, dest_bin_id)
}

#[tauri::command]
pub fn trash_files(state: State<AppState>, file_ids: Vec<i64>) -> AppResult<usize> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    ops::trash_files(&conn, &root, &file_ids)
}

#[tauri::command]
pub fn toggle_pin_file(state: State<AppState>, file_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE files SET pinned = 1 - pinned WHERE id = ?1",
        [file_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn quick_look(path: String) -> AppResult<()> {
    std::process::Command::new("qlmanage")
        .args(["-p", &path])
        .spawn()
        .map_err(|e| AppError::msg(format!("quick look failed: {e}")))?;
    Ok(())
}

// ---------- Bins ----------

#[tauri::command]
pub fn create_bin(state: State<AppState>, project_id: i64, name: String) -> AppResult<i64> {
    let name = name.trim().to_string();
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return Err(AppError::msg("invalid bin name"));
    }
    let conn = state.conn.lock().unwrap();
    let proj = ops::project_path(&conn, project_id)?;
    let dir = proj.join(&name);
    if dir.exists() {
        return Err(AppError::msg(format!("\"{name}\" already exists")));
    }
    std::fs::create_dir_all(&dir)?;
    conn.execute(
        "INSERT INTO bins (project_id, name, rel_path, sort_order)
         VALUES (?1, ?2, ?2, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM bins WHERE project_id = ?1))",
        params![project_id, name],
    )?;
    ops::auto_log(&conn, project_id, &format!("created bin {name}"))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn rename_bin(state: State<AppState>, bin_id: i64, new_name: String) -> AppResult<()> {
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() || new_name.contains('/') || new_name.starts_with('.') {
        return Err(AppError::msg("invalid bin name"));
    }
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let (project_id, rel_path): (i64, String) = conn.query_row(
        "SELECT project_id, rel_path FROM bins WHERE id = ?1",
        [bin_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let proj = ops::project_path(&conn, project_id)?;
    let old_dir = proj.join(&rel_path);
    ops::assert_under_root(&root, &old_dir)?;
    let new_dir = proj.join(&new_name);
    if new_dir.exists() {
        return Err(AppError::msg(format!("\"{new_name}\" already exists")));
    }
    std::fs::rename(&old_dir, &new_dir)?;
    ops::auto_log(&conn, project_id, &format!("renamed bin {rel_path} → {new_name}"))?;
    // Bin rename shifts every child rel_path — a rescan settles it all.
    scan::scan(&mut conn, &root)?;
    Ok(())
}

#[tauri::command]
pub fn trash_bin(state: State<AppState>, bin_id: i64) -> AppResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let (project_id, rel_path): (i64, String) = conn.query_row(
        "SELECT project_id, rel_path FROM bins WHERE id = ?1",
        [bin_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let proj = ops::project_path(&conn, project_id)?;
    let dir = proj.join(&rel_path);
    ops::assert_under_root(&root, &dir)?;
    trash::delete(&dir).map_err(|e| AppError::msg(format!("trash failed: {e}")))?;
    ops::auto_log(&conn, project_id, &format!("moved bin {rel_path} to Trash"))?;
    scan::scan(&mut conn, &root)?;
    Ok(())
}

// ---------- Inbox & rules ----------

#[derive(Serialize)]
pub struct InboxItem {
    pub path: String,
    pub name: String,
    pub size: i64,
    pub mtime: i64,
    pub suggested_bin_id: Option<i64>,
    pub suggested_bin_name: Option<String>,
}

#[tauri::command]
pub fn list_inbox(state: State<AppState>, project_id: Option<i64>) -> AppResult<Vec<InboxItem>> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let inbox = root.join(scan::INBOX_DIR);
    std::fs::create_dir_all(&inbox)?;

    // Bins of the chosen target project, for suggestions.
    let bins: Vec<(i64, String)> = match project_id {
        Some(pid) => {
            let mut stmt =
                conn.prepare("SELECT id, name FROM bins WHERE project_id = ?1")?;
            let v = stmt
                .query_map([pid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            v
        }
        None => vec![],
    };
    // User rules for that project (or global), most specific first.
    let user_rules: Vec<(String, String, Option<i64>)> = {
        let mut stmt = conn.prepare(
            "SELECT pattern, match, dest_bin_id FROM rules
             WHERE enabled = 1 AND (project_id IS NULL OR project_id = ?1)
             ORDER BY project_id IS NULL",
        )?;
        let v = stmt
            .query_map([project_id.unwrap_or(-1)], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        v
    };

    let suggest = |file_name: &str| -> (Option<i64>, Option<String>) {
        for (pattern, kind, dest_bin) in &user_rules {
            if rule_matches(kind, pattern, file_name) {
                if let Some(bid) = dest_bin {
                    if let Some((id, name)) = bins.iter().find(|(id, _)| id == bid) {
                        return (Some(*id), Some(name.clone()));
                    }
                }
            }
        }
        for (pattern, kind, bin_name) in DEFAULT_RULES {
            if rule_matches(kind, pattern, file_name) {
                if let Some((id, name)) = bins
                    .iter()
                    .find(|(_, n)| n.eq_ignore_ascii_case(bin_name))
                {
                    return (Some(*id), Some(name.clone()));
                }
            }
        }
        (None, None)
    };

    let mut items = vec![];
    for entry in std::fs::read_dir(&inbox)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !path.is_file() {
            continue;
        }
        let md = entry.metadata().ok();
        let (size, mtime) = md
            .map(|m| {
                let mt = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                (m.len() as i64, mt)
            })
            .unwrap_or((0, 0));
        let (bid, bname) = suggest(&name);
        items.push(InboxItem {
            path: path.to_string_lossy().to_string(),
            name,
            size,
            mtime,
            suggested_bin_id: bid,
            suggested_bin_name: bname,
        });
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.mtime));
    Ok(items)
}

#[derive(Deserialize)]
pub struct InboxFiling {
    pub path: String,
    pub project_id: i64,
    pub bin_id: Option<i64>,
}

#[tauri::command]
pub fn file_inbox(state: State<AppState>, items: Vec<InboxFiling>) -> AppResult<usize> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let mut filed = 0usize;
    for item in &items {
        let src = PathBuf::from(&item.path);
        ops::assert_under_root(&root, &src)?;
        if !src.exists() {
            continue;
        }
        let proj = ops::project_path(&conn, item.project_id)?;
        let dest_dir = match item.bin_id {
            Some(bid) => {
                let rel: String = conn.query_row(
                    "SELECT rel_path FROM bins WHERE id = ?1 AND project_id = ?2",
                    params![bid, item.project_id],
                    |r| r.get(0),
                )?;
                proj.join(rel)
            }
            None => proj.clone(),
        };
        std::fs::create_dir_all(&dest_dir)?;
        let name = src
            .file_name()
            .ok_or_else(|| AppError::msg("bad inbox path"))?
            .to_string_lossy()
            .to_string();
        let final_name = ops::dedupe_name(&dest_dir, &name);
        std::fs::rename(&src, dest_dir.join(&final_name))?;
        ops::auto_log(&conn, item.project_id, &format!("filed {final_name} from Inbox"))?;
        filed += 1;
    }
    scan::scan(&mut conn, &root)?;
    Ok(filed)
}

#[derive(Serialize)]
pub struct RuleRow {
    pub id: i64,
    pub project_id: Option<i64>,
    pub pattern: String,
    pub match_kind: String,
    pub dest_bin_id: Option<i64>,
    pub dest_bin_name: Option<String>,
    pub enabled: bool,
}

#[tauri::command]
pub fn list_rules(state: State<AppState>) -> AppResult<Vec<RuleRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT r.id, r.project_id, r.pattern, r.match, r.dest_bin_id, b.name, r.enabled
         FROM rules r LEFT JOIN bins b ON b.id = r.dest_bin_id
         ORDER BY r.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(RuleRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                pattern: r.get(2)?,
                match_kind: r.get(3)?,
                dest_bin_id: r.get(4)?,
                dest_bin_name: r.get(5)?,
                enabled: r.get::<_, i64>(6)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn save_rule(
    state: State<AppState>,
    id: Option<i64>,
    project_id: Option<i64>,
    pattern: String,
    match_kind: String,
    dest_bin_id: Option<i64>,
    enabled: bool,
) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    match id {
        Some(rid) => {
            conn.execute(
                "UPDATE rules SET project_id=?1, pattern=?2, match=?3, dest_bin_id=?4, enabled=?5
                 WHERE id=?6",
                params![project_id, pattern, match_kind, dest_bin_id, enabled as i64, rid],
            )?;
            Ok(rid)
        }
        None => {
            conn.execute(
                "INSERT INTO rules (project_id, pattern, match, dest_bin_id, enabled)
                 VALUES (?1,?2,?3,?4,?5)",
                params![project_id, pattern, match_kind, dest_bin_id, enabled as i64],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn delete_rule(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM rules WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn test_rule(pattern: String, match_kind: String, samples: Vec<String>) -> Vec<bool> {
    samples
        .iter()
        .map(|s| rule_matches(&match_kind, &pattern, s))
        .collect()
}

// ---------- Logs ----------

#[derive(Serialize)]
pub struct LogRow {
    pub id: i64,
    pub ts: String,
    pub kind: String,
    pub body_md: String,
}

#[tauri::command]
pub fn list_logs(state: State<AppState>, project_id: i64) -> AppResult<Vec<LogRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, ts, kind, body_md FROM logs WHERE project_id = ?1
         ORDER BY ts DESC, id DESC LIMIT 500",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(LogRow {
                id: r.get(0)?,
                ts: r.get(1)?,
                kind: r.get(2)?,
                body_md: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_log(state: State<AppState>, project_id: i64, body: String) -> AppResult<()> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err(AppError::msg("log entry is empty"));
    }
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'note', ?2)",
        params![project_id, body],
    )?;
    let dir = ops::project_path(&conn, project_id)?;
    append_log_md(
        &dir,
        &format!("- {} · {}", chrono::Local::now().format("%Y-%m-%d %H:%M"), body),
    )?;
    Ok(())
}

// ---------- Progress & project meta ----------

#[tauri::command]
pub fn set_progress(state: State<AppState>, project_id: i64, value: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    ops::set_progress(&conn, project_id, value, "manual")
}

#[derive(Deserialize)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub emoji: Option<String>,
    pub color: Option<String>,
    pub status: Option<String>,
    pub target_date: Option<Option<String>>,
    pub pinned: Option<bool>,
}

#[tauri::command]
pub fn update_project(
    state: State<AppState>,
    project_id: i64,
    patch: ProjectPatch,
) -> AppResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let dir = ops::project_path(&conn, project_id)?;
    let mut sc = Sidecar::load_or_init(&dir, &dir.file_name().unwrap_or_default().to_string_lossy())?;

    // Folder rename first (disk is truth), then metadata.
    let mut dir = dir;
    if let Some(new_name) = patch.name.as_deref().map(str::trim) {
        if !new_name.is_empty() && new_name != sc.name {
            let new_dir = root.join(new_name);
            if new_dir.exists() {
                return Err(AppError::msg(format!("\"{new_name}\" already exists")));
            }
            std::fs::rename(&dir, &new_dir)?;
            conn.execute(
                "UPDATE projects SET path = ?1 WHERE id = ?2",
                params![new_dir.to_string_lossy().to_string(), project_id],
            )?;
            sc.name = new_name.to_string();
            dir = new_dir;
        }
    }
    if let Some(emoji) = &patch.emoji {
        sc.emoji = emoji.clone();
    }
    if let Some(color) = &patch.color {
        sc.color = color.clone();
    }
    if let Some(status) = &patch.status {
        sc.status = status.clone();
    }
    if let Some(td) = &patch.target_date {
        sc.target_date = td.clone();
    }
    if let Some(pinned) = patch.pinned {
        sc.pinned = pinned;
    }
    sc.updated_at = Some(chrono::Utc::now().to_rfc3339());
    sc.save(&dir)?;
    conn.execute(
        "UPDATE projects SET name=?1, emoji=?2, color=?3, status=?4, target_date=?5,
         pinned=?6, updated_at=datetime('now') WHERE id=?7",
        params![
            sc.name, sc.emoji, sc.color, sc.status, sc.target_date,
            sc.pinned as i64, project_id
        ],
    )?;
    if patch.status.is_some() {
        ops::auto_log(&conn, project_id, &format!("status → {}", sc.status))?;
    }
    scan::scan(&mut conn, &root)?;
    Ok(())
}

// ---------- Search ----------

#[derive(Serialize)]
pub struct SearchHit {
    pub kind: String, // project | file | log
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub subtitle: String,
}

#[tauri::command]
pub fn search(state: State<AppState>, query: String) -> AppResult<Vec<SearchHit>> {
    let q = query.trim().replace('"', "");
    if q.is_empty() {
        return Ok(vec![]);
    }
    let conn = state.conn.lock().unwrap();
    let mut hits: Vec<SearchHit> = vec![];

    let like = format!("%{q}%");
    let mut stmt = conn.prepare(
        "SELECT id, name, path FROM projects WHERE name LIKE ?1 LIMIT 8",
    )?;
    hits.extend(
        stmt.query_map([&like], |r| {
            Ok(SearchHit {
                kind: "project".into(),
                id: r.get(0)?,
                project_id: r.get(0)?,
                title: r.get(1)?,
                subtitle: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok()),
    );

    let fts = format!("\"{q}\"*");
    let mut stmt = conn.prepare(
        "SELECT f.id, f.project_id, f.name, p.name || ' · ' || f.rel_path
         FROM files_fts ft JOIN files f ON f.id = ft.rowid
         JOIN projects p ON p.id = f.project_id
         WHERE files_fts MATCH ?1 LIMIT 20",
    )?;
    hits.extend(
        stmt.query_map([&fts], |r| {
            Ok(SearchHit {
                kind: "file".into(),
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                subtitle: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok()),
    );

    let mut stmt = conn.prepare(
        "SELECT l.id, l.project_id, substr(l.body_md, 1, 80), p.name || ' · log'
         FROM logs_fts lt JOIN logs l ON l.id = lt.rowid
         JOIN projects p ON p.id = l.project_id
         WHERE logs_fts MATCH ?1 LIMIT 10",
    )?;
    hits.extend(
        stmt.query_map([&fts], |r| {
            Ok(SearchHit {
                kind: "log".into(),
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                subtitle: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok()),
    );

    Ok(hits)
}

// ---------- Idea backlog ----------

#[derive(Serialize)]
pub struct IdeaRow {
    pub id: i64,
    pub name: String,
    pub note: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn list_ideas(state: State<AppState>) -> AppResult<Vec<IdeaRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt =
        conn.prepare("SELECT id, name, note, created_at FROM ideas ORDER BY id DESC")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(IdeaRow {
                id: r.get(0)?,
                name: r.get(1)?,
                note: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_idea(state: State<AppState>, name: String, note: Option<String>) -> AppResult<i64> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::msg("idea needs a name"));
    }
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO ideas (name, note) VALUES (?1, ?2)",
        params![name, note],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_idea(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM ideas WHERE id = ?1", [id])?;
    Ok(())
}
