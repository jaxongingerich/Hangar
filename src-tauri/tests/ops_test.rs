use hangar_lib::{db, ops, scan};
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("hangar-ops-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn scan_indexes_projects_bins_files() {
    let root = temp_root("scan");
    let proj = root.join("TestBoard");
    std::fs::create_dir_all(proj.join("Gerbers")).unwrap();
    std::fs::create_dir_all(proj.join("Firmware/src")).unwrap();
    std::fs::write(proj.join("Gerbers/top.gbr"), "G04*").unwrap();
    std::fs::write(proj.join("Firmware/main.ino"), "void setup(){}").unwrap();
    std::fs::write(proj.join("Firmware/src/util.h"), "// h").unwrap();
    std::fs::write(proj.join("readme.md"), "# hi").unwrap();

    let mut conn = db::open(&root.join("test.db")).unwrap();
    let stats = scan::scan(&mut conn, &root).unwrap();
    assert_eq!(stats.projects, 1);
    assert_eq!(stats.files, 4, "expected 4 files, stats: {stats:?}");

    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_count, 4);

    // Bin assignment: top.gbr belongs to the Gerbers bin.
    let bin_name: String = conn
        .query_row(
            "SELECT b.name FROM files f JOIN bins b ON b.id = f.bin_id WHERE f.name = 'top.gbr'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bin_name, "Gerbers");

    // Rescan is idempotent.
    let stats2 = scan::scan(&mut conn, &root).unwrap();
    assert_eq!(stats2.files, 4);
    let file_count2: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_count2, 4);

    // Deleting a file on disk removes it on the next scan.
    std::fs::remove_file(proj.join("readme.md")).unwrap();
    scan::scan(&mut conn, &root).unwrap();
    let file_count3: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_count3, 3);

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn move_and_rename_files() {
    let root = temp_root("move");
    let proj = root.join("Widget");
    std::fs::create_dir_all(proj.join("CAD")).unwrap();
    std::fs::create_dir_all(proj.join("Docs")).unwrap();
    std::fs::write(proj.join("case.step"), "solid").unwrap();

    let mut conn = db::open(&root.join("test.db")).unwrap();
    scan::scan(&mut conn, &root).unwrap();

    let (file_id, cad_bin): (i64, i64) = {
        let fid: i64 = conn
            .query_row("SELECT id FROM files WHERE name='case.step'", [], |r| r.get(0))
            .unwrap();
        let bid: i64 = conn
            .query_row("SELECT id FROM bins WHERE name='CAD'", [], |r| r.get(0))
            .unwrap();
        (fid, bid)
    };

    // Move the loose file into CAD.
    let moved = ops::move_files(&conn, &root, &[file_id], Some(cad_bin)).unwrap();
    assert_eq!(moved, 1);
    assert!(proj.join("CAD/case.step").exists());
    assert!(!proj.join("case.step").exists());

    // Rename it.
    ops::rename_file(&conn, &root, file_id, "enclosure-v2.step").unwrap();
    assert!(proj.join("CAD/enclosure-v2.step").exists());
    let (rel, name): (String, String) = conn
        .query_row(
            "SELECT rel_path, name FROM files WHERE id=?1",
            [file_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(rel, "CAD/enclosure-v2.step");
    assert_eq!(name, "enclosure-v2.step");

    // Auto logs were written for both ops.
    let log_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM logs WHERE kind='auto'", [], |r| r.get(0))
        .unwrap();
    assert!(log_count >= 2);

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn progress_writes_history_and_sidecar() {
    let root = temp_root("progress");
    std::fs::create_dir_all(root.join("Thing")).unwrap();
    let mut conn = db::open(&root.join("test.db")).unwrap();
    scan::scan(&mut conn, &root).unwrap();

    let pid: i64 = conn
        .query_row("SELECT id FROM projects WHERE name='Thing'", [], |r| r.get(0))
        .unwrap();
    ops::set_progress(&conn, pid, 40, "manual").unwrap();

    let (progress, hist): (i64, i64) = (
        conn.query_row("SELECT progress FROM projects WHERE id=?1", [pid], |r| r.get(0))
            .unwrap(),
        conn.query_row(
            "SELECT COUNT(*) FROM progress_history WHERE project_id=?1",
            [pid],
            |r| r.get(0),
        )
        .unwrap(),
    );
    assert_eq!(progress, 40);
    assert_eq!(hist, 1);

    let sc = hangar_lib::sidecar::Sidecar::load(&root.join("Thing")).unwrap();
    assert_eq!(sc.progress, 40);

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn root_guard_blocks_outside_paths() {
    let root = temp_root("guard");
    assert!(ops::assert_under_root(&root, &root.join("ok.txt")).is_ok());
    assert!(ops::assert_under_root(&root, &PathBuf::from("/etc/passwd")).is_err());
}
