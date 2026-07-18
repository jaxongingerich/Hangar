use crate::db;
use crate::error::{AppError, AppResult};
use crate::ops;
use crate::scan;
use crate::AppState;
use base64::Engine;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

fn root_of(conn: &Connection) -> AppResult<PathBuf> {
    Ok(PathBuf::from(
        db::get_setting(conn, "root")?.ok_or_else(|| AppError::msg("no root configured"))?,
    ))
}

// ---------- File notes ----------

#[tauri::command]
pub fn get_file_note(state: State<AppState>, file_id: i64) -> AppResult<Option<String>> {
    let conn = state.conn.lock().unwrap();
    let note = conn
        .query_row(
            "SELECT body_md FROM file_notes WHERE file_id = ?1 ORDER BY id DESC LIMIT 1",
            [file_id],
            |r| r.get(0),
        )
        .ok();
    Ok(note)
}

#[tauri::command]
pub fn set_file_note(state: State<AppState>, file_id: i64, body: String) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM file_notes WHERE file_id = ?1", [file_id])?;
    if !body.trim().is_empty() {
        conn.execute(
            "INSERT INTO file_notes (file_id, body_md) VALUES (?1, ?2)",
            params![file_id, body.trim()],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn noted_file_ids(state: State<AppState>, project_id: i64) -> AppResult<Vec<i64>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT fn.file_id FROM file_notes fn
         JOIN files f ON f.id = fn.file_id WHERE f.project_id = ?1",
    )?;
    let ids = stmt
        .query_map([project_id], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

// ---------- Clipboard paste-as-file ----------

#[tauri::command]
pub fn save_clipboard_file(
    state: State<AppState>,
    project_id: i64,
    bin_id: Option<i64>,
    kind: String, // "png" | "txt"
    data_base64: Option<String>,
    text: Option<String>,
) -> AppResult<String> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let proj = ops::project_path(&conn, project_id)?;
    let dest_dir = match bin_id {
        Some(bid) => {
            let rel: String = conn.query_row(
                "SELECT rel_path FROM bins WHERE id = ?1 AND project_id = ?2",
                params![bid, project_id],
                |r| r.get(0),
            )?;
            proj.join(rel)
        }
        None => proj.clone(),
    };
    ops::assert_under_root(&root, &dest_dir)?;
    std::fs::create_dir_all(&dest_dir)?;

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let name = match kind.as_str() {
        "png" => format!("Paste_{stamp}.png"),
        _ => format!("Paste_{stamp}.txt"),
    };
    let final_name = ops::dedupe_name(&dest_dir, &name);
    let dest = dest_dir.join(&final_name);

    if let Some(b64) = data_base64 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|e| AppError::msg(format!("bad clipboard data: {e}")))?;
        std::fs::write(&dest, bytes)?;
    } else if let Some(t) = text {
        std::fs::write(&dest, t)?;
    } else {
        return Err(AppError::msg("clipboard was empty"));
    }
    ops::auto_log(&conn, project_id, &format!("pasted clipboard → {final_name}"))?;
    scan::scan(&mut conn, &root)?;
    Ok(final_name)
}

// ---------- Smart collections ----------

#[derive(Serialize)]
pub struct CollectionRow {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub icon: Option<String>,
}

#[tauri::command]
pub fn list_collections(state: State<AppState>) -> AppResult<Vec<CollectionRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, name, query_json, icon FROM collections ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CollectionRow {
                id: r.get(0)?,
                name: r.get(1)?,
                query: r.get(2)?,
                icon: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn save_collection(state: State<AppState>, name: String, query: String) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO collections (name, query_json) VALUES (?1, ?2)",
        params![name.trim(), query],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_collection(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

/// Run a collection query: free text over name (FTS-ish LIKE), plus
/// `ext:` and `>10mb` style filters embedded in the query text.
#[tauri::command]
pub fn run_collection(state: State<AppState>, query: String) -> AppResult<Vec<crate::commands_m3::BigFile>> {
    let conn = state.conn.lock().unwrap();
    let mut text_terms: Vec<String> = vec![];
    let mut ext: Option<String> = None;
    let mut min_size: i64 = 0;
    let mut days: Option<i64> = None;
    for token in query.split_whitespace() {
        let t = token.to_lowercase();
        if let Some(e) = t.strip_prefix("ext:") {
            ext = Some(e.trim_start_matches('.').to_string());
        } else if let Some(rest) = t.strip_prefix('>') {
            let (num, mult) = if let Some(n) = rest.strip_suffix("gb") {
                (n, 1_073_741_824i64)
            } else if let Some(n) = rest.strip_suffix("mb") {
                (n, 1_048_576i64)
            } else if let Some(n) = rest.strip_suffix("kb") {
                (n, 1024i64)
            } else {
                (rest, 1i64)
            };
            min_size = num.parse::<f64>().map(|v| (v * mult as f64) as i64).unwrap_or(0);
        } else if let Some(d) = t.strip_prefix("touched:") {
            days = d.trim_end_matches('d').parse().ok();
        } else {
            text_terms.push(t);
        }
    }
    let mut sql = String::from(
        "SELECT f.id, p.name, f.name, f.rel_path, f.size, p.path
         FROM files f JOIN projects p ON p.id = f.project_id WHERE 1=1",
    );
    let like = format!("%{}%", text_terms.join(" "));
    if !text_terms.is_empty() {
        sql.push_str(" AND f.name LIKE ?1");
    } else {
        sql.push_str(" AND ?1 = ?1");
    }
    if let Some(e) = &ext {
        sql.push_str(&format!(" AND f.ext = '{}'", e.replace('\'', "")));
    }
    if min_size > 0 {
        sql.push_str(&format!(" AND f.size >= {min_size}"));
    }
    if let Some(d) = days {
        let cutoff = chrono::Utc::now().timestamp_millis() - d * 86_400_000;
        sql.push_str(&format!(" AND f.mtime >= {cutoff}"));
    }
    sql.push_str(" ORDER BY f.size DESC LIMIT 200");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&like], |r| {
            let proj_path: String = r.get(5)?;
            let rel: String = r.get(3)?;
            Ok(crate::commands_m3::BigFile {
                id: r.get(0)?,
                project_name: r.get(1)?,
                name: r.get(2)?,
                rel_path: rel.clone(),
                size: r.get(4)?,
                abs_path: format!("{proj_path}/{rel}"),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ---------- Watched folders ----------

#[tauri::command]
pub fn get_watched_dirs(state: State<AppState>) -> AppResult<Vec<String>> {
    let conn = state.conn.lock().unwrap();
    Ok(db::get_setting(&conn, "watched_dirs")?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn set_watched_dirs(
    app: tauri::AppHandle,
    state: State<AppState>,
    dirs: Vec<String>,
) -> AppResult<()> {
    {
        let conn = state.conn.lock().unwrap();
        db::set_setting(&conn, "watched_dirs", &serde_json::to_string(&dirs)?)?;
    }
    crate::restart_sweepers(&app);
    Ok(())
}

#[tauri::command]
pub fn get_sweep_patterns(state: State<AppState>) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    Ok(db::get_setting(&conn, "sweep_patterns")?
        .unwrap_or_else(|| "*.zip,*.pdf,*.step,*.gbr,*.csv".into()))
}

#[tauri::command]
pub fn set_sweep_patterns(state: State<AppState>, patterns: String) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    db::set_setting(&conn, "sweep_patterns", &patterns)?;
    Ok(())
}

// ---------- Finder tags ----------

/// Read macOS Finder tags via mdls.
#[tauri::command]
pub fn get_finder_tags(path: String) -> AppResult<Vec<String>> {
    let out = std::process::Command::new("mdls")
        .args(["-name", "kMDItemUserTags", "-raw", &path])
        .output()
        .map_err(|e| AppError::msg(format!("mdls failed: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    if text.trim() == "(null)" {
        return Ok(vec![]);
    }
    // Output looks like: (\n    "Red",\n    Important\n)
    let tags = text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(tags)
}

/// Write macOS Finder tags via xattr + plutil.
#[tauri::command]
pub fn set_finder_tags(path: String, tags: Vec<String>) -> AppResult<()> {
    let items: String = tags
        .iter()
        .map(|t| format!("<string>{}</string>", t.replace('&', "&amp;").replace('<', "&lt;")))
        .collect();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><array>{items}</array></plist>"#
    );
    // plutil converts the XML plist to binary form on stdin → stdout.
    let mut child = std::process::Command::new("plutil")
        .args(["-convert", "binary1", "-o", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::msg(format!("plutil failed: {e}")))?;
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| AppError::msg("plutil stdin unavailable"))?
            .write_all(plist.as_bytes())?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| AppError::msg(format!("plutil failed: {e}")))?;
    if !out.status.success() {
        return Err(AppError::msg("plutil conversion failed"));
    }
    let hex: String = out.stdout.iter().map(|b| format!("{b:02x}")).collect();
    let status = std::process::Command::new("xattr")
        .args(["-wx", "com.apple.metadata:_kMDItemUserTags", &hex, &path])
        .status()
        .map_err(|e| AppError::msg(format!("xattr failed: {e}")))?;
    if !status.success() {
        return Err(AppError::msg("xattr write failed"));
    }
    Ok(())
}

// ---------- Backups ----------

#[derive(Serialize)]
pub struct BackupStatus {
    pub backup_dir: Option<String>,
    pub keep: i64,
    pub last_backup: Option<String>,
    pub backups: Vec<(String, i64)>, // name, size
}

#[tauri::command]
pub fn backup_status(state: State<AppState>) -> AppResult<BackupStatus> {
    let conn = state.conn.lock().unwrap();
    let backup_dir = db::get_setting(&conn, "backup_dir")?;
    let keep = db::get_setting(&conn, "backup_keep")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let last_backup = db::get_setting(&conn, "last_backup")?;
    let mut backups = vec![];
    if let Some(dir) = &backup_dir {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("hangar-backup-") && name.ends_with(".zip") {
                    backups.push((
                        name,
                        entry.metadata().map(|m| m.len() as i64).unwrap_or(0),
                    ));
                }
            }
        }
        backups.sort_by(|a, b| b.0.cmp(&a.0));
    }
    Ok(BackupStatus {
        backup_dir,
        keep,
        last_backup,
        backups,
    })
}

#[tauri::command]
pub fn set_backup_dir(state: State<AppState>, dir: Option<String>) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    match dir {
        Some(d) => db::set_setting(&conn, "backup_dir", &d)?,
        None => {
            conn.execute("DELETE FROM settings WHERE key = 'backup_dir'", [])?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn run_backup(state: State<AppState>) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let backup_dir = db::get_setting(&conn, "backup_dir")?
        .ok_or_else(|| AppError::msg("choose a backup destination first"))?;
    let keep: usize = db::get_setting(&conn, "backup_keep")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let backup_dir = PathBuf::from(backup_dir);
    std::fs::create_dir_all(&backup_dir)?;

    let out = backup_dir.join(format!(
        "hangar-backup-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    crate::commands_m3::zip_dir(&root, &out, &root)?;

    // Verify: re-read the finished zip and make sure it opens.
    let file = std::fs::File::open(&out)?;
    zip::ZipArchive::new(file).map_err(|e| AppError::msg(format!("backup verify failed: {e}")))?;

    // Prune old backups beyond `keep`.
    let mut existing: Vec<PathBuf> = std::fs::read_dir(&backup_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("hangar-backup-") && n.ends_with(".zip")
                })
                .unwrap_or(false)
        })
        .collect();
    existing.sort();
    while existing.len() > keep {
        let oldest = existing.remove(0);
        let _ = std::fs::remove_file(oldest);
    }

    db::set_setting(&conn, "last_backup", &chrono::Utc::now().to_rfc3339())?;
    Ok(out.to_string_lossy().to_string())
}

// ---------- Universal import ----------

/// Copy (never move) arbitrary files from anywhere on disk into a project
/// bin — or `_Inbox` when no project is given. Root-guarded destinations,
/// name-deduped, journaled.
#[tauri::command]
pub fn import_files(
    state: State<AppState>,
    paths: Vec<String>,
    project_id: Option<i64>,
    bin_id: Option<i64>,
) -> AppResult<usize> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let dest_dir = match project_id {
        Some(pid) => {
            let proj = ops::project_path(&conn, pid)?;
            match bin_id {
                Some(bid) => {
                    let rel: String = conn.query_row(
                        "SELECT rel_path FROM bins WHERE id = ?1 AND project_id = ?2",
                        rusqlite::params![bid, pid],
                        |r| r.get(0),
                    )?;
                    proj.join(rel)
                }
                None => proj,
            }
        }
        None => root.join(scan::INBOX_DIR),
    };
    ops::assert_under_root(&root, &dest_dir)?;
    std::fs::create_dir_all(&dest_dir)?;

    let mut imported = 0usize;
    for p in &paths {
        let src = PathBuf::from(p);
        let Some(name) = src.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if src.is_file() {
            let final_name = ops::dedupe_name(&dest_dir, &name);
            std::fs::copy(&src, dest_dir.join(&final_name))?;
            imported += 1;
        } else if src.is_dir() {
            // Copy the whole folder tree (still copy-never-move).
            let final_name = ops::dedupe_name(&dest_dir, &name);
            imported += copy_dir_recursive(&src, &dest_dir.join(&final_name))?;
        }
    }
    if imported > 0 {
        if let Some(pid) = project_id {
            ops::auto_log(&conn, pid, &format!("imported {imported} file(s)"))?;
        }
        ops::journal(
            &conn,
            "import",
            &format!("Import {imported} file(s) → {}", dest_dir.display()),
            None,
        )?;
        scan::scan(&mut conn, &root)?;
    }
    Ok(imported)
}

/// Copy a directory tree, returning how many files landed. Hidden files are
/// skipped so .DS_Store and friends don't pollute the library.
fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> AppResult<usize> {
    std::fs::create_dir_all(dest)?;
    let mut count = 0usize;
    for entry in std::fs::read_dir(src)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&name);
        if from.is_dir() {
            count += copy_dir_recursive(&from, &to)?;
        } else if from.is_file() {
            std::fs::copy(&from, &to)?;
            count += 1;
        }
    }
    Ok(count)
}

// ---------- Gerber preview support ----------

#[derive(Serialize)]
pub struct GerberFile {
    pub filename: String,
    pub content: String,
}

/// Read gerber/drill text files from a bin for in-webview board rendering.
#[tauri::command]
pub fn read_bin_gerbers(state: State<AppState>, bin_id: i64) -> AppResult<Vec<GerberFile>> {
    const MAX_FILE: u64 = 8 * 1024 * 1024;
    const GERBER_EXTS: &[&str] = &[
        "gbr", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gko", "gm1", "drl", "xln", "txt",
    ];
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let (project_id, rel_path): (i64, String) = conn.query_row(
        "SELECT project_id, rel_path FROM bins WHERE id = ?1",
        [bin_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let dir = ops::project_path(&conn, project_id)?.join(rel_path);
    ops::assert_under_root(&root, &dir)?;
    let mut out = vec![];
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !GERBER_EXTS.contains(&ext.as_str()) {
            continue;
        }
        if entry.metadata().map(|m| m.len() > MAX_FILE).unwrap_or(true) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            out.push(GerberFile { filename: name, content });
        }
        if out.len() >= 24 {
            break;
        }
    }
    Ok(out)
}

// ---------- Global timeline ----------

#[derive(Serialize)]
pub struct TimelineRow {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub project_emoji: String,
    pub ts: String,
    pub kind: String,
    pub body_md: String,
}

#[tauri::command]
pub fn global_timeline(state: State<AppState>, limit: Option<i64>) -> AppResult<Vec<TimelineRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT l.id, l.project_id, p.name, p.emoji, l.ts, l.kind, l.body_md
         FROM logs l JOIN projects p ON p.id = l.project_id
         ORDER BY l.ts DESC, l.id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit.unwrap_or(200)], |r| {
            Ok(TimelineRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                project_name: r.get(2)?,
                project_emoji: r.get(3)?,
                ts: r.get(4)?,
                kind: r.get(5)?,
                body_md: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
