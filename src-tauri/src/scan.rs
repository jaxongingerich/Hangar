use crate::error::AppResult;
use crate::sidecar::{slugify, Sidecar, SIDECAR_DIR};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};
use walkdir::WalkDir;

pub const INBOX_DIR: &str = "_Inbox";
pub const ARCHIVE_DIR: &str = "_Archive";

/// Directories never worth indexing — build junk and VCS internals.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
    ".next",
    ".cache",
    "DerivedData",
];

#[derive(Debug, Serialize)]
pub struct ScanStats {
    pub projects: usize,
    pub files: usize,
    pub elapsed_ms: u128,
}

pub fn ensure_root_layout(root: &Path) -> AppResult<()> {
    std::fs::create_dir_all(root.join(INBOX_DIR))?;
    std::fs::create_dir_all(root.join(ARCHIVE_DIR))?;
    Ok(())
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Full scan: every top-level folder under `root` (except `_*` and hidden)
/// becomes a project. Disk is truth — DB rows are rebuilt to match.
pub fn scan(conn: &mut Connection, root: &Path) -> AppResult<ScanStats> {
    let started = Instant::now();
    ensure_root_layout(root)?;

    let token = format!("{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let mut project_count = 0usize;
    let mut file_count = 0usize;
    let mut live_paths: Vec<String> = Vec::new();

    let entries = std::fs::read_dir(root)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if is_hidden(&dir_name) || dir_name.starts_with('_') {
            continue;
        }
        let project_id = upsert_project(conn, &path, &dir_name)?;
        live_paths.push(path.to_string_lossy().to_string());
        file_count += index_project_files(conn, project_id, &path, &token)?;
        project_count += 1;
    }

    // Remove projects whose folders vanished from this root.
    let root_prefix = format!("{}/%", root.to_string_lossy());
    let stale: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id, path FROM projects WHERE path LIKE ?1")?;
        let rows = stmt.query_map([&root_prefix], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.filter_map(|r| r.ok())
            .filter(|(_, p)| !live_paths.contains(p))
            .map(|(id, _)| id)
            .collect()
    };
    for id in stale {
        conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    }

    Ok(ScanStats {
        projects: project_count,
        files: file_count,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn upsert_project(conn: &Connection, path: &Path, dir_name: &str) -> AppResult<i64> {
    let sc = Sidecar::load_or_init(path, dir_name)?;
    let path_str = path.to_string_lossy().to_string();

    let existing: Option<i64> = conn
        .query_row("SELECT id FROM projects WHERE path = ?1", [&path_str], |r| r.get(0))
        .ok();

    if let Some(id) = existing {
        conn.execute(
            "UPDATE projects SET name=?1, emoji=?2, color=?3, status=?4, progress=?5,
             progress_mode=?6, target_date=?7, pinned=?8, updated_at=datetime('now')
             WHERE id=?9",
            params![
                sc.name, sc.emoji, sc.color, sc.status, sc.progress, sc.progress_mode,
                sc.target_date, sc.pinned as i64, id
            ],
        )?;
        return Ok(id);
    }

    // New project: find a free slug.
    let base = slugify(&sc.name);
    let mut slug = base.clone();
    let mut n = 2;
    loop {
        let taken: bool = conn
            .query_row("SELECT 1 FROM projects WHERE slug = ?1", [&slug], |_| Ok(true))
            .unwrap_or(false);
        if !taken {
            break;
        }
        slug = format!("{}-{}", base, n);
        n += 1;
    }
    conn.execute(
        "INSERT INTO projects (slug, name, path, emoji, color, status, progress,
         progress_mode, target_date, pinned)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            slug, sc.name, path_str, sc.emoji, sc.color, sc.status, sc.progress,
            sc.progress_mode, sc.target_date, sc.pinned as i64
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn index_project_files(
    conn: &mut Connection,
    project_id: i64,
    project_dir: &Path,
    token: &str,
) -> AppResult<usize> {
    let tx = conn.transaction()?;
    let mut count = 0usize;

    // Bins = first-level subfolders (except .hangar / hidden).
    let mut live_bins: Vec<String> = Vec::new();
    let mut sort = 0;
    for entry in std::fs::read_dir(project_dir)?.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if p.is_dir() && !is_hidden(&name) && name != SIDECAR_DIR {
            tx.execute(
                "INSERT INTO bins (project_id, name, rel_path, sort_order)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, rel_path) DO UPDATE SET name=excluded.name",
                params![project_id, name, name, sort],
            )?;
            live_bins.push(name);
            sort += 1;
        }
    }
    // Drop bins whose folders are gone.
    {
        let mut stmt = tx.prepare("SELECT id, rel_path FROM bins WHERE project_id = ?1")?;
        let gone: Vec<i64> = stmt
            .query_map([project_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .filter(|(_, rp)| !live_bins.contains(rp))
            .map(|(id, _)| id)
            .collect();
        drop(stmt);
        for id in gone {
            tx.execute("DELETE FROM bins WHERE id = ?1", [id])?;
        }
    }

    // Map bin rel_path -> id for file assignment.
    let bin_map: Vec<(String, i64)> = {
        let mut stmt = tx.prepare("SELECT rel_path, id FROM bins WHERE project_id = ?1")?;
        let rows = stmt.query_map([project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let walker = WalkDir::new(project_dir).follow_links(false).into_iter();
    for entry in walker.filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !(e.file_type().is_dir() && (is_hidden(&name) || SKIP_DIRS.contains(&name.as_ref())))
    }) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_hidden(&name) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(project_dir) else { continue };
        let rel_path = rel.to_string_lossy().to_string();
        let ext = entry
            .path()
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());
        let md = entry.metadata().ok();
        let (size, mtime) = md.map(|m| (m.len() as i64, mtime_ms(&m))).unwrap_or((0, 0));
        let bin_id = bin_map
            .iter()
            .find(|(bp, _)| rel_path.starts_with(&format!("{}/", bp)))
            .map(|(_, id)| *id);

        tx.execute(
            "INSERT INTO files (project_id, bin_id, rel_path, name, ext, size, mtime, indexed_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(project_id, rel_path) DO UPDATE SET
               bin_id=excluded.bin_id, name=excluded.name, ext=excluded.ext,
               size=excluded.size, mtime=excluded.mtime, indexed_at=excluded.indexed_at",
            params![project_id, bin_id, rel_path, name, ext, size, mtime, token],
        )?;
        count += 1;
    }

    // Files not touched by this scan no longer exist on disk.
    tx.execute(
        "DELETE FROM files WHERE project_id = ?1 AND indexed_at != ?2",
        params![project_id, token],
    )?;

    tx.commit()?;
    Ok(count)
}
