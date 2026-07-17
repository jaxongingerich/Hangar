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

/// Record an operation in the undo journal (50-step stack).
pub fn journal(
    conn: &Connection,
    kind: &str,
    description: &str,
    inverse: Option<serde_json::Value>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO op_journal (kind, description, inverse_json) VALUES (?1, ?2, ?3)",
        params![kind, description, inverse.map(|v| v.to_string())],
    )?;
    conn.execute(
        "DELETE FROM op_journal WHERE id NOT IN (SELECT id FROM op_journal ORDER BY id DESC LIMIT 50)",
        [],
    )?;
    Ok(())
}

/// Undo the most recent undoable operation. Returns its description.
pub fn undo_last(conn: &Connection, root: &Path) -> AppResult<Option<String>> {
    let row: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT id, description, inverse_json FROM op_journal
             WHERE undone = 0 AND inverse_json IS NOT NULL
             ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let Some((id, description, inverse_json)) = row else {
        return Ok(None);
    };
    let inverse: serde_json::Value = serde_json::from_str(&inverse_json)?;
    if let Some(renames) = inverse.get("renames").and_then(|v| v.as_array()) {
        for pair in renames {
            let (Some(from), Some(to)) = (
                pair.get(0).and_then(|v| v.as_str()),
                pair.get(1).and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let from = PathBuf::from(from);
            let to = PathBuf::from(to);
            assert_under_root(root, &from)?;
            assert_under_root(root, &to)?;
            if from.exists() && !to.exists() {
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&from, &to)?;
            }
        }
    }
    conn.execute("UPDATE op_journal SET undone = 1 WHERE id = ?1", [id])?;
    Ok(Some(description))
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
    journal(
        conn,
        "rename",
        &format!("Rename {old_name} → {new_name}"),
        Some(serde_json::json!({
            "renames": [[new_abs.to_string_lossy(), old_abs.to_string_lossy()]]
        })),
    )?;
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
    let mut rename_pairs: Vec<(String, String)> = vec![];
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
        rename_pairs.push((
            dest.to_string_lossy().to_string(),
            src.to_string_lossy().to_string(),
        ));
        moved += 1;
        last_project = Some(project_id);
    }
    if let Some(pid) = last_project {
        auto_log(conn, pid, &format!("moved {moved} file(s) → {dest_label}"))?;
    }
    if !rename_pairs.is_empty() {
        journal(
            conn,
            "move",
            &format!("Move {moved} file(s) → {dest_label}"),
            Some(serde_json::json!({ "renames": rename_pairs })),
        )?;
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

/// Weighted milestone progress: done milestones count fully, "doing"
/// milestones earn partial credit from their checked tasks.
pub fn weighted_progress(conn: &Connection, project_id: i64) -> AppResult<Option<i64>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.state, m.weight,
                (SELECT COUNT(*) FROM tasks t WHERE t.milestone_id = m.id),
                (SELECT COUNT(*) FROM tasks t WHERE t.milestone_id = m.id AND t.done = 1)
         FROM milestones m WHERE m.project_id = ?1",
    )?;
    let rows: Vec<(String, i64, i64, i64)> = stmt
        .query_map([project_id], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    if rows.is_empty() {
        return Ok(None);
    }
    let total: f64 = rows.iter().map(|(_, w, _, _)| *w.max(&1) as f64).sum();
    let earned: f64 = rows
        .iter()
        .map(|(state, w, tasks, done_tasks)| {
            let w = *w.max(&1) as f64;
            match state.as_str() {
                "done" => w,
                "doing" => {
                    if *tasks > 0 {
                        w * (*done_tasks as f64 / *tasks as f64)
                    } else {
                        w * 0.5
                    }
                }
                _ => 0.0,
            }
        })
        .sum();
    Ok(Some(((earned / total) * 100.0).round() as i64))
}

/// If the project derives progress from milestones, recompute and record it.
pub fn recompute_progress(conn: &Connection, project_id: i64) -> AppResult<()> {
    let (mode, current): (String, i64) = conn.query_row(
        "SELECT progress_mode, progress FROM projects WHERE id = ?1",
        [project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if mode != "milestones" {
        return Ok(());
    }
    if let Some(value) = weighted_progress(conn, project_id)? {
        if value != current {
            set_progress(conn, project_id, value, "milestones")?;
        }
    }
    Ok(())
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
