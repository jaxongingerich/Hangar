use hangar_lib::{db, ops, scan};

fn setup(tag: &str) -> (rusqlite::Connection, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("hangar-m3-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let conn = db::open(&root.join("test.db")).unwrap();
    (conn, root)
}

#[test]
fn undo_reverses_a_move() {
    let (mut conn, root) = setup("undo");
    let proj = root.join("Widget");
    std::fs::create_dir_all(proj.join("CAD")).unwrap();
    std::fs::write(proj.join("case.step"), "solid").unwrap();
    scan::scan(&mut conn, &root).unwrap();

    let file_id: i64 = conn
        .query_row("SELECT id FROM files WHERE name='case.step'", [], |r| r.get(0))
        .unwrap();
    let cad: i64 = conn
        .query_row("SELECT id FROM bins WHERE name='CAD'", [], |r| r.get(0))
        .unwrap();
    ops::move_files(&conn, &root, &[file_id], Some(cad)).unwrap();
    assert!(proj.join("CAD/case.step").exists());

    let undone = ops::undo_last(&conn, &root).unwrap();
    assert!(undone.is_some(), "expected an undoable op");
    assert!(proj.join("case.step").exists(), "file should be back at root");
    assert!(!proj.join("CAD/case.step").exists());

    // Nothing more to undo (rename op journal was consumed).
    scan::scan(&mut conn, &root).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn journal_caps_at_50() {
    let (conn, root) = setup("cap");
    for i in 0..60 {
        ops::journal(&conn, "test", &format!("op {i}"), None).unwrap();
    }
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM op_journal", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 50);
    std::fs::remove_dir_all(&root).unwrap();
}
