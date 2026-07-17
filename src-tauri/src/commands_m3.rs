use crate::db;
use crate::error::{AppError, AppResult};
use crate::ops;
use crate::scan;
use crate::AppState;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::State;

fn root_of(conn: &Connection) -> AppResult<PathBuf> {
    Ok(PathBuf::from(
        db::get_setting(conn, "root")?.ok_or_else(|| AppError::msg("no root configured"))?,
    ))
}

// ---------- Space & health ----------

#[derive(Serialize)]
pub struct SpaceProject {
    pub id: i64,
    pub name: String,
    pub emoji: String,
    pub color: String,
    pub size_bytes: i64,
    pub file_count: i64,
    pub bins: Vec<(String, i64)>, // name, bytes
    pub days_since_touch: Option<i64>,
    pub empty_bins: Vec<String>,
}

#[derive(Serialize)]
pub struct BigFile {
    pub id: i64,
    pub project_name: String,
    pub name: String,
    pub rel_path: String,
    pub size: i64,
    pub abs_path: String,
}

#[derive(Serialize)]
pub struct SpaceReport {
    pub projects: Vec<SpaceProject>,
    pub largest: Vec<BigFile>,
    pub loose_root_files: i64,
    pub disk_free_bytes: i64,
    pub total_bytes: i64,
}

#[tauri::command]
pub fn space_report(state: State<AppState>) -> AppResult<SpaceReport> {
    let conn = state.conn.lock().unwrap();
    let now_ms = chrono::Utc::now().timestamp_millis();

    let mut projects = vec![];
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.emoji, p.color,
                COALESCE(SUM(f.size),0), COUNT(f.id), MAX(f.mtime)
         FROM projects p LEFT JOIN files f ON f.project_id = p.id
         GROUP BY p.id ORDER BY COALESCE(SUM(f.size),0) DESC",
    )?;
    type ProjectSizeRow = (i64, String, String, String, i64, i64, Option<i64>);
    let base: Vec<ProjectSizeRow> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (id, name, emoji, color, size_bytes, file_count, last_mtime) in base {
        let mut bin_stmt = conn.prepare(
            "SELECT b.name, COALESCE(SUM(f.size),0), COUNT(f.id)
             FROM bins b LEFT JOIN files f ON f.bin_id = b.id
             WHERE b.project_id = ?1 GROUP BY b.id ORDER BY 2 DESC",
        )?;
        let bin_rows: Vec<(String, i64, i64)> = bin_stmt
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        projects.push(SpaceProject {
            id,
            name,
            emoji,
            color,
            size_bytes,
            file_count,
            bins: bin_rows.iter().map(|(n, s, _)| (n.clone(), *s)).collect(),
            days_since_touch: last_mtime.map(|ms| ((now_ms - ms) / 86_400_000).max(0)),
            empty_bins: bin_rows
                .iter()
                .filter(|(_, _, c)| *c == 0)
                .map(|(n, _, _)| n.clone())
                .collect(),
        });
    }

    let root = root_of(&conn)?;
    let mut stmt = conn.prepare(
        "SELECT f.id, p.name, f.name, f.rel_path, f.size, p.path
         FROM files f JOIN projects p ON p.id = f.project_id
         ORDER BY f.size DESC LIMIT 25",
    )?;
    let largest: Vec<BigFile> = stmt
        .query_map([], |r| {
            let proj_path: String = r.get(5)?;
            let rel: String = r.get(3)?;
            Ok(BigFile {
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

    let loose_root_files = std::fs::read_dir(&root)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.path().is_file()
                        && !e.file_name().to_string_lossy().starts_with('.')
                })
                .count() as i64
        })
        .unwrap_or(0);

    let total_bytes: i64 =
        conn.query_row("SELECT COALESCE(SUM(size),0) FROM files", [], |r| r.get(0))?;

    Ok(SpaceReport {
        projects,
        largest,
        loose_root_files,
        disk_free_bytes: crate::commands_m2::free_space_of(&root.to_string_lossy()).unwrap_or(0),
        total_bytes,
    })
}

#[derive(Serialize)]
pub struct DupeGroup {
    pub hash: String,
    pub size: i64,
    pub files: Vec<BigFile>,
}

/// Hash same-size files with blake3 and group true duplicates.
#[tauri::command]
pub fn find_duplicates(state: State<AppState>) -> AppResult<Vec<DupeGroup>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT f.id, p.name, f.name, f.rel_path, f.size, p.path
         FROM files f JOIN projects p ON p.id = f.project_id
         WHERE f.size > 4096 AND f.size IN (
           SELECT size FROM files WHERE size > 4096 GROUP BY size HAVING COUNT(*) > 1
         ) ORDER BY f.size DESC LIMIT 2000",
    )?;
    let candidates: Vec<BigFile> = stmt
        .query_map([], |r| {
            let proj_path: String = r.get(5)?;
            let rel: String = r.get(3)?;
            Ok(BigFile {
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

    let mut by_hash: HashMap<String, Vec<BigFile>> = HashMap::new();
    for file in candidates {
        let Ok(bytes) = std::fs::read(&file.abs_path) else { continue };
        let hash = blake3::hash(&bytes).to_hex().to_string();
        conn.execute(
            "UPDATE files SET blake3 = ?1 WHERE id = ?2",
            params![hash, file.id],
        )?;
        by_hash.entry(hash).or_default().push(file);
    }

    let mut groups: Vec<DupeGroup> = by_hash
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(hash, files)| DupeGroup {
            hash,
            size: files[0].size,
            files,
        })
        .collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.size * (g.files.len() as i64 - 1)));
    Ok(groups)
}

// ---------- Archive & restore ----------

fn zip_dir(src: &Path, zip_path: &Path, strip_prefix: &Path) -> AppResult<()> {
    let file = std::fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry.map_err(|e| AppError::msg(format!("walk failed: {e}")))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(strip_prefix)
            .map_err(|_| AppError::msg("zip path escape"))?
            .to_string_lossy()
            .replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        if entry.file_type().is_dir() {
            zip.add_directory(format!("{rel}/"), options)
                .map_err(|e| AppError::msg(format!("zip dir failed: {e}")))?;
        } else if entry.file_type().is_file() {
            zip.start_file(rel, options)
                .map_err(|e| AppError::msg(format!("zip file failed: {e}")))?;
            let mut f = std::fs::File::open(path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
    }
    zip.finish()
        .map_err(|e| AppError::msg(format!("zip finish failed: {e}")))?;
    Ok(())
}

#[tauri::command]
pub fn archive_project(state: State<AppState>, project_id: i64) -> AppResult<String> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let dir = ops::project_path(&conn, project_id)?;
    ops::assert_under_root(&root, &dir)?;
    let name = dir
        .file_name()
        .ok_or_else(|| AppError::msg("bad project dir"))?
        .to_string_lossy()
        .to_string();
    let archive_dir = root.join(scan::ARCHIVE_DIR);
    std::fs::create_dir_all(&archive_dir)?;
    let zip_path = archive_dir.join(format!(
        "{name}_{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M")
    ));
    zip_dir(&dir, &zip_path, &dir)?;
    trash::delete(&dir).map_err(|e| AppError::msg(format!("trash failed: {e}")))?;
    ops::journal(
        &conn,
        "archive",
        &format!("Archive {name}"),
        None,
    )?;
    scan::scan(&mut conn, &root)?;
    Ok(zip_path.to_string_lossy().to_string())
}

#[derive(Serialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub path: String,
    pub size: i64,
    pub created_ms: i64,
}

#[tauri::command]
pub fn list_archives(state: State<AppState>) -> AppResult<Vec<ArchiveEntry>> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let dir = root.join(scan::ARCHIVE_DIR);
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "zip").unwrap_or(false) {
                let md = entry.metadata().ok();
                out.push(ArchiveEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: path.to_string_lossy().to_string(),
                    size: md.as_ref().map(|m| m.len() as i64).unwrap_or(0),
                    created_ms: md
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                });
            }
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.created_ms));
    Ok(out)
}

#[tauri::command]
pub fn restore_archive(state: State<AppState>, zip_path: String) -> AppResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let zip_path = PathBuf::from(zip_path);
    ops::assert_under_root(&root, &zip_path)?;
    let stem = zip_path
        .file_stem()
        .ok_or_else(|| AppError::msg("bad archive name"))?
        .to_string_lossy()
        .to_string();
    // Strip the _YYYYMMDD-HHMM suffix we added at archive time.
    let name = stem
        .rsplit_once('_')
        .map(|(n, ts)| {
            if ts.len() == 13 && ts.chars().filter(|c| c.is_ascii_digit()).count() == 12 {
                n.to_string()
            } else {
                stem.clone()
            }
        })
        .unwrap_or(stem.clone());
    let dest = root.join(&name);
    if dest.exists() {
        return Err(AppError::msg(format!("\"{name}\" already exists in the root")));
    }
    let file = std::fs::File::open(&zip_path)?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| AppError::msg(format!("open zip failed: {e}")))?;
    zip.extract(&dest)
        .map_err(|e| AppError::msg(format!("extract failed: {e}")))?;
    scan::scan(&mut conn, &root)?;
    Ok(())
}

// ---------- Snapshots & diff ----------

#[derive(Serialize)]
pub struct SnapshotRow {
    pub id: i64,
    pub bin_id: Option<i64>,
    pub bin_name: Option<String>,
    pub label: String,
    pub zip_path: String,
    pub file_count: i64,
    pub created_at: String,
}

#[tauri::command]
pub fn snapshot_bin(state: State<AppState>, bin_id: i64, label: String) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let (project_id, rel_path, bin_name): (i64, String, String) = conn.query_row(
        "SELECT project_id, rel_path, name FROM bins WHERE id = ?1",
        [bin_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let proj = ops::project_path(&conn, project_id)?;
    let bin_dir = proj.join(&rel_path);
    ops::assert_under_root(&root, &bin_dir)?;

    // Manifest: rel_path → (blake3, size).
    let mut manifest: HashMap<String, (String, i64)> = HashMap::new();
    for entry in walkdir::WalkDir::new(&bin_dir).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&bin_dir) else { continue };
        let Ok(bytes) = std::fs::read(entry.path()) else { continue };
        manifest.insert(
            rel.to_string_lossy().to_string(),
            (blake3::hash(&bytes).to_hex().to_string(), bytes.len() as i64),
        );
    }
    if manifest.is_empty() {
        return Err(AppError::msg("bin is empty — nothing to snapshot"));
    }

    let snap_dir = proj.join(".hangar").join("snapshots");
    std::fs::create_dir_all(&snap_dir)?;
    let safe_label = label.trim().replace('/', "-");
    let zip_path = snap_dir.join(format!(
        "{bin_name}_{safe_label}_{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    zip_dir(&bin_dir, &zip_path, &bin_dir)?;

    conn.execute(
        "INSERT INTO snapshots (project_id, bin_id, label, zip_path, file_manifest_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            project_id,
            bin_id,
            safe_label,
            zip_path.to_string_lossy().to_string(),
            serde_json::to_string(&manifest)?
        ],
    )?;
    let id = conn.last_insert_rowid();
    ops::auto_log(
        &conn,
        project_id,
        &format!("snapshot {bin_name} → {safe_label} ({} files)", manifest.len()),
    )?;
    Ok(id)
}

#[tauri::command]
pub fn list_snapshots(state: State<AppState>, project_id: i64) -> AppResult<Vec<SnapshotRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT s.id, s.bin_id, b.name, s.label, s.zip_path, s.file_manifest_json, s.created_at
         FROM snapshots s LEFT JOIN bins b ON b.id = s.bin_id
         WHERE s.project_id = ?1 ORDER BY s.id DESC",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            let manifest: String = r.get(5)?;
            let count = serde_json::from_str::<HashMap<String, (String, i64)>>(&manifest)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            Ok(SnapshotRow {
                id: r.get(0)?,
                bin_id: r.get(1)?,
                bin_name: r.get(2)?,
                label: r.get(3)?,
                zip_path: r.get(4)?,
                file_count: count,
                created_at: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[derive(Serialize)]
pub struct SnapshotDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

#[tauri::command]
pub fn diff_snapshots(state: State<AppState>, a_id: i64, b_id: i64) -> AppResult<SnapshotDiff> {
    let conn = state.conn.lock().unwrap();
    let load = |id: i64| -> AppResult<HashMap<String, (String, i64)>> {
        let json: String = conn.query_row(
            "SELECT file_manifest_json FROM snapshots WHERE id = ?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&json)?)
    };
    let a = load(a_id)?;
    let b = load(b_id)?;
    let mut diff = SnapshotDiff {
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    for (path, (hash, _)) in &b {
        match a.get(path) {
            None => diff.added.push(path.clone()),
            Some((old_hash, _)) if old_hash != hash => diff.changed.push(path.clone()),
            _ => {}
        }
    }
    for path in a.keys() {
        if !b.contains_key(path) {
            diff.removed.push(path.clone());
        }
    }
    diff.added.sort();
    diff.removed.sort();
    diff.changed.sort();
    Ok(diff)
}

// ---------- JLCPCB export ----------

const JLC_LAYERS: &[(&str, &[&str])] = &[
    ("Top copper", &["gtl"]),
    ("Bottom copper", &["gbl"]),
    ("Top silkscreen", &["gto"]),
    ("Bottom silkscreen", &["gbo"]),
    ("Top soldermask", &["gts"]),
    ("Bottom soldermask", &["gbs"]),
    ("Board outline", &["gko", "gm1"]),
    ("Drill", &["drl", "xln", "txt"]),
];

#[derive(Serialize)]
pub struct JlcValidation {
    pub present: Vec<String>,
    pub missing: Vec<String>,
    pub zip_path: Option<String>,
}

#[tauri::command]
pub fn export_jlcpcb(
    state: State<AppState>,
    project_id: i64,
    bin_id: Option<i64>,
    snapshot_id: Option<i64>,
    dry_run: bool,
) -> AppResult<JlcValidation> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let proj = ops::project_path(&conn, project_id)?;
    ops::assert_under_root(&root, &proj)?;
    let project_name: String = conn.query_row(
        "SELECT name FROM projects WHERE id = ?1",
        [project_id],
        |r| r.get(0),
    )?;

    // Source: a snapshot's manifest+zip, or a live bin.
    let file_names: Vec<String>;
    let source: Option<PathBuf>;
    if let Some(sid) = snapshot_id {
        let (manifest_json, zip_path): (String, String) = conn.query_row(
            "SELECT file_manifest_json, zip_path FROM snapshots WHERE id = ?1",
            [sid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let manifest: HashMap<String, (String, i64)> = serde_json::from_str(&manifest_json)?;
        file_names = manifest.keys().cloned().collect();
        source = Some(PathBuf::from(zip_path));
    } else {
        let bid = bin_id.ok_or_else(|| AppError::msg("need a bin or a snapshot"))?;
        let rel: String = conn.query_row(
            "SELECT rel_path FROM bins WHERE id = ?1 AND project_id = ?2",
            params![bid, project_id],
            |r| r.get(0),
        )?;
        let bin_dir = proj.join(rel);
        file_names = walkdir::WalkDir::new(&bin_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        source = Some(bin_dir);
    }

    let has_ext = |exts: &[&str]| {
        file_names.iter().any(|n| {
            let lower = n.to_lowercase();
            exts.iter().any(|e| lower.ends_with(&format!(".{e}")))
        })
    };
    let mut present = vec![];
    let mut missing = vec![];
    for (label, exts) in JLC_LAYERS {
        if has_ext(exts) {
            present.push(label.to_string());
        } else {
            missing.push(label.to_string());
        }
    }

    let mut zip_out = None;
    if !dry_run {
        let source = source.ok_or_else(|| AppError::msg("no source"))?;
        let jlc_dir = proj.join("JLCPCB");
        std::fs::create_dir_all(&jlc_dir)?;
        let out = jlc_dir.join(format!(
            "{}_JLC_{}.zip",
            project_name.replace(' ', "-"),
            chrono::Local::now().format("%Y%m%d-%H%M")
        ));
        if source.extension().map(|e| e == "zip").unwrap_or(false) {
            // Snapshot export: the snapshot zip already is the fab package.
            std::fs::copy(&source, &out)?;
        } else {
            zip_dir(&source, &out, &source)?;
        }
        ops::auto_log(
            &conn,
            project_id,
            &format!("exported JLCPCB package ({} layers present)", present.len()),
        )?;
        zip_out = Some(out.to_string_lossy().to_string());
    }

    Ok(JlcValidation {
        present,
        missing,
        zip_path: zip_out,
    })
}

// ---------- BOM normalize ----------

/// Normalize a BOM CSV toward the JLC/LCSC column format:
/// Comment, Designator, Footprint, "LCSC Part #".
#[tauri::command]
pub fn normalize_bom(state: State<AppState>, file_id: i64) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let (project_id, rel_path): (i64, String) = conn.query_row(
        "SELECT project_id, rel_path FROM files WHERE id = ?1",
        [file_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let proj = ops::project_path(&conn, project_id)?;
    let src = proj.join(&rel_path);
    ops::assert_under_root(&root, &src)?;
    let text = std::fs::read_to_string(&src)?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| AppError::msg("empty BOM file"))?;

    let cols: Vec<String> = split_csv(header);
    let find = |aliases: &[&str]| -> Option<usize> {
        cols.iter().position(|c| {
            let c = c.to_lowercase();
            aliases.iter().any(|a| c.contains(a))
        })
    };
    let comment = find(&["comment", "value", "val", "part name", "name"]);
    let designator = find(&["designator", "ref", "reference"]);
    let footprint = find(&["footprint", "package", "case"]);
    let lcsc = find(&["lcsc", "jlc", "part #", "part number", "supplier part"]);

    if designator.is_none() {
        return Err(AppError::msg(
            "couldn't find a Designator/Reference column — is this a BOM?",
        ));
    }

    let mut out = String::from("Comment,Designator,Footprint,\"LCSC Part #\"\n");
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_csv(line);
        let get = |idx: Option<usize>| {
            idx.and_then(|i| fields.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        out.push_str(&format!(
            "{},{},{},{}\n",
            csv_quote(&get(comment)),
            csv_quote(&get(designator)),
            csv_quote(&get(footprint)),
            csv_quote(&get(lcsc)),
        ));
    }

    let dest = src.with_file_name(format!(
        "{}_JLC.csv",
        src.file_stem().unwrap_or_default().to_string_lossy()
    ));
    std::fs::write(&dest, out)?;
    ops::auto_log(&conn, project_id, "normalized BOM toward JLC format")?;
    Ok(dest.to_string_lossy().to_string())
}

fn split_csv(line: &str) -> Vec<String> {
    let mut fields = vec![];
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    fields.push(cur.trim().to_string());
    fields
}

fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------- Parts library ----------

#[derive(Serialize)]
pub struct ComponentRow {
    pub id: i64,
    pub mpn: String,
    pub lcsc: Option<String>,
    pub description: Option<String>,
    pub package: Option<String>,
    pub value: Option<String>,
    pub used_in: Vec<String>,
}

#[tauri::command]
pub fn list_components(state: State<AppState>, query: Option<String>) -> AppResult<Vec<ComponentRow>> {
    let conn = state.conn.lock().unwrap();
    let like = format!("%{}%", query.unwrap_or_default());
    let mut stmt = conn.prepare(
        "SELECT id, mpn, lcsc, description, package, value FROM components
         WHERE mpn LIKE ?1 OR lcsc LIKE ?1 OR description LIKE ?1 OR value LIKE ?1
         ORDER BY mpn LIMIT 200",
    )?;
    let mut rows: Vec<ComponentRow> = stmt
        .query_map([&like], |r| {
            Ok(ComponentRow {
                id: r.get(0)?,
                mpn: r.get(1)?,
                lcsc: r.get(2)?,
                description: r.get(3)?,
                package: r.get(4)?,
                value: r.get(5)?,
                used_in: vec![],
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    let mut use_stmt = conn.prepare(
        "SELECT p.name FROM component_uses cu JOIN projects p ON p.id = cu.project_id
         WHERE cu.component_id = ?1",
    )?;
    for row in &mut rows {
        row.used_in = use_stmt
            .query_map([row.id], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
    }
    Ok(rows)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn save_component(
    state: State<AppState>,
    id: Option<i64>,
    mpn: String,
    lcsc: Option<String>,
    description: Option<String>,
    package: Option<String>,
    value: Option<String>,
) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    match id {
        Some(cid) => {
            conn.execute(
                "UPDATE components SET mpn=?1, lcsc=?2, description=?3, package=?4, value=?5 WHERE id=?6",
                params![mpn.trim(), lcsc, description, package, value, cid],
            )?;
            Ok(cid)
        }
        None => {
            conn.execute(
                "INSERT INTO components (mpn, lcsc, description, package, value) VALUES (?1,?2,?3,?4,?5)",
                params![mpn.trim(), lcsc, description, package, value],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn delete_component(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM components WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn use_component(
    state: State<AppState>,
    component_id: i64,
    project_id: i64,
    qty: i64,
    ref_des: Option<String>,
) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO component_uses (component_id, project_id, qty, ref_des)
         VALUES (?1, ?2, ?3, ?4)",
        params![component_id, project_id, qty, ref_des.unwrap_or_default()],
    )?;
    Ok(())
}

// ---------- Undo ----------

#[tauri::command]
pub fn undo_last_op(state: State<AppState>) -> AppResult<Option<String>> {
    let mut conn = state.conn.lock().unwrap();
    let root = root_of(&conn)?;
    let result = ops::undo_last(&conn, &root)?;
    if result.is_some() {
        scan::scan(&mut conn, &root)?;
    }
    Ok(result)
}

// ---------- One-pager export ----------

/// Generate a print-ready HTML one-pager next to the project (open → ⌘P → PDF).
#[tauri::command]
pub fn export_one_pager(state: State<AppState>, project_id: i64) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    let (name, emoji, progress, status, target): (String, String, i64, String, Option<String>) =
        conn.query_row(
            "SELECT name, emoji, progress, status, target_date FROM projects WHERE id = ?1",
            [project_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let stats = crate::commands_m2::compute_stats(&conn, project_id)?;

    let milestones: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT title, state FROM milestones WHERE project_id = ?1 ORDER BY sort_order",
        )?;
        let v = stmt
            .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    let recent_logs: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT ts, body_md FROM logs WHERE project_id = ?1 ORDER BY ts DESC LIMIT 15",
        )?;
        let v = stmt
            .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    let (spend_total, open_orders): (i64, i64) = (
        conn.query_row(
            "SELECT COALESCE(SUM(cost_cents),0) FROM orders WHERE project_id = ?1",
            [project_id],
            |r| r.get(0),
        )?,
        conn.query_row(
            "SELECT COUNT(*) FROM orders WHERE project_id = ?1 AND status IN ('ordered','shipped')",
            [project_id],
            |r| r.get(0),
        )?,
    );

    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let milestone_html: String = milestones
        .iter()
        .map(|(t, s)| {
            let mark = match s.as_str() {
                "done" => "✅",
                "doing" => "🔵",
                _ => "⚪",
            };
            format!("<li>{mark} {}</li>", esc(t))
        })
        .collect();
    let log_html: String = recent_logs
        .iter()
        .map(|(ts, body)| format!("<li><code>{}</code> {}</li>", &ts[..16.min(ts.len())], esc(body)))
        .collect();

    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{name} — one pager</title>
<style>
body {{ font-family: -apple-system, sans-serif; max-width: 720px; margin: 40px auto; color: #111; }}
h1 {{ letter-spacing: -0.02em; }} code {{ font-family: ui-monospace, monospace; font-size: 11px; color: #666; }}
.meta {{ display: flex; gap: 24px; font-family: ui-monospace, monospace; font-size: 13px; margin: 16px 0; }}
.bar {{ height: 8px; background: #eee; border-radius: 4px; overflow: hidden; margin: 12px 0 24px; }}
.fill {{ height: 100%; background: #16b895; width: {progress}%; }}
ul {{ padding-left: 18px; line-height: 1.7; font-size: 13px; }} h2 {{ font-size: 15px; margin-top: 28px; }}
</style></head><body>
<h1>{emoji} {name}</h1>
<div class="meta">
  <span>{progress}% complete</span><span>status: {status}</span>
  <span>health: {}</span><span>target: {}</span>
  <span>spend: ${:.2}</span><span>{open_orders} open orders</span>
</div>
<div class="bar"><div class="fill"></div></div>
<h2>Milestones</h2><ul>{milestone_html}</ul>
<h2>Recent activity</h2><ul>{log_html}</ul>
<p><code>Generated by Hangar · {}</code></p>
</body></html>"#,
        stats.health,
        target.as_deref().unwrap_or("—"),
        spend_total as f64 / 100.0,
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        name = esc(&name),
        emoji = emoji,
        progress = progress,
        status = status,
    );

    let dir = ops::project_path(&conn, project_id)?;
    let out = dir.join(format!("{}_one-pager.html", name.replace(' ', "-")));
    std::fs::write(&out, html)?;
    Ok(out.to_string_lossy().to_string())
}
