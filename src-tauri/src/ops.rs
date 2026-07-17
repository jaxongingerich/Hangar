use crate::error::{AppError, AppResult};
use crate::sidecar::{append_log_md, Sidecar};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

/// Every mutation must stay inside the configured root. Disk is truth, but
/// only Hangar's truth — never touch paths outside it.
pub fn assert_under_root(root: &Path, path: &Path) -> AppResult<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(AppError::msg(format!(
            "refusing to touch {} — outside the Hangar root",
            path.display()
        )))
    }
}

pub fn project_path(conn: &Connection, project_id: i64) -> AppResult<PathBuf> {
    let path: String = conn.query_row(
        "SELECT path FROM projects WHERE id = ?1",
        [project_id],
        |r| r.get(0),
    )?;
    Ok(PathBuf::from(path))
}

/// Insert an auto log row and mirror it to `.hangar/log.md`.
pub fn auto_log(conn: &Connection, project_id: i64, body: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'auto', ?2)",
        params![project_id, body],
    )?;
    if let Ok(dir) = project_path(conn, project_id) {
        let _ = append_log_md(
            &dir,
            &format!("- {} · {}", chrono::Local::now().format("%Y-%m-%d %H:%M"), body),
        );
    }
    Ok(())
}

/// Pick a collision-free name in `dir` by appending " (n)" before the extension.
pub fn dedupe_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 2..1000 {
        let candidate = format!("{stem} ({n}){ext}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{stem}-{}{ext}", chrono::Utc::now().timestamp())
}

pub fn rename_file(conn: &Connection, root: &Path, file_id: i64, new_name: &str) -> AppResult<()> {
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.contains('/') {
        return Err(AppError::msg("invalid file name"));
    }
    let (project_id, rel_path): (i64, String) = conn.query_row(
        "SELECT project_id, rel_path FROM files WHERE id = ?1",
        [file_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let proj = project_path(conn, project_id)?;
    let old_abs = proj.join(&rel_path);
    assert_under_root(root, &old_abs)?;
    let parent = old_abs
        .parent()
        .ok_or_else(|| AppError::msg("file has no parent"))?;
    let new_abs = parent.join(new_name);
    if new_abs.exists() {
        return Err(AppError::msg(format!("\"{new_name}\" already exists here")));
    }
    std::fs::rename(&old_abs, &new_abs)?;

    let new_rel = new_abs
        .strip_prefix(&proj)
        .map_err(|_| AppError::msg("path escaped project"))?
        .to_string_lossy()
        .to_string();
    let ext = new_abs
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    let old_name = rel_path.rsplit('/').next().unwrap_or(&rel_path).to_string();
    conn.execute(
        "UPDATE files SET rel_path = ?1, name = ?2, ext = ?3 WHERE id = ?4",
        params![new_rel, new_name, ext, file_id],
    )?;
    auto_log(conn, project_id, &format!("renamed {} → {}", old_name, new_name))?;
    Ok(())
}

/// Move files into a bin of the same project (bin_id None = project root).
pub fn move_files(
    conn: &Connection,
    root: &Path,
    file_ids: &[i64],
    dest_bin_id: Option<i64>,
) -> AppResult<usize> {
    let mut moved = 0usize;
    let mut last_project = None;
    let mut dest_label = "project root".to_string();
    for &file_id in file_ids {
        let (project_id, rel_path): (i64, String) = conn.query_row(
            "SELECT project_id, rel_path FROM files WHERE id = ?1",
            [file_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let proj = project_path(conn, project_id)?;
        let dest_rel: String = match dest_bin_id {
            Some(bin_id) => {
                let (bin_proj, bin_rel): (i64, String) = conn.query_row(
                    "SELECT project_id, rel_path FROM bins WHERE id = ?1",
                    [bin_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                if bin_proj != project_id {
                    return Err(AppError::msg("destination bin belongs to another project"));
                }
                dest_label = bin_rel.clone();
                bin_rel
            }
            None => String::new(),
        };
        let src = proj.join(&rel_path);
        assert_under_root(root, &src)?;
        let dest_dir = if dest_rel.is_empty() {
            proj.clone()
        } else {
            proj.join(&dest_rel)
        };
        std::fs::create_dir_all(&dest_dir)?;
        let file_name = src
            .file_name()
            .ok_or_else(|| AppError::msg("bad file path"))?
            .to_string_lossy()
            .to_string();
        if src.parent() == Some(dest_dir.as_path()) {
            continue; // already there
        }
        let final_name = dedupe_name(&dest_dir, &file_name);
        let dest = dest_dir.join(&final_name);
        std::fs::rename(&src, &dest)?;
        let new_rel = dest
            .strip_prefix(&proj)
            .map_err(|_| AppError::msg("path escaped project"))?
            .to_string_lossy()
            .to_string();
        conn.execute(
            "UPDATE files SET rel_path = ?1, name = ?2, bin_id = ?3 WHERE id = ?4",
            params![new_rel, final_name, dest_bin_id, file_id],
        )?;
        moved += 1;
        last_project = Some(project_id);
    }
    if let Some(pid) = last_project {
        auto_log(conn, pid, &format!("moved {moved} file(s) → {dest_label}"))?;
    }
    Ok(moved)
}

/// Non-destructive delete: macOS Trash, never a hard delete.
pub fn trash_files(conn: &Connection, root: &Path, file_ids: &[i64]) -> AppResult<usize> {
    let mut trashed = 0usize;
    let mut last_project = None;
    for &file_id in file_ids {
        let (project_id, rel_path): (i64, String) = conn.query_row(
            "SELECT project_id, rel_path FROM files WHERE id = ?1",
            [file_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let proj = project_path(conn, project_id)?;
        let abs = proj.join(&rel_path);
        assert_under_root(root, &abs)?;
        trash::delete(&abs).map_err(|e| AppError::msg(format!("trash failed: {e}")))?;
        conn.execute("DELETE FROM files WHERE id = ?1", [file_id])?;
        trashed += 1;
        last_project = Some(project_id);
    }
    if let Some(pid) = last_project {
        auto_log(conn, pid, &format!("moved {trashed} file(s) to Trash"))?;
    }
    Ok(trashed)
}

pub fn set_progress(conn: &Connection, project_id: i64, value: i64, source: &str) -> AppResult<()> {
    let value = value.clamp(0, 100);
    conn.execute(
        "UPDATE projects SET progress = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![value, project_id],
    )?;
    conn.execute(
        "INSERT INTO progress_history (project_id, value, source) VALUES (?1, ?2, ?3)",
        params![project_id, value, source],
    )?;
    // Sidecar first-class: keep project.json in sync.
    let dir = project_path(conn, project_id)?;
    if let Some(mut sc) = Sidecar::load(&dir) {
        sc.progress = value;
        sc.updated_at = Some(chrono::Utc::now().to_rfc3339());
        sc.save(&dir)?;
    }
    auto_log(conn, project_id, &format!("progress → {value}%"))?;
    Ok(())
}
