use hangar_lib::{commands_m2, db, ops, scan};

fn setup(tag: &str) -> (rusqlite::Connection, std::path::PathBuf, i64) {
    let root = std::env::temp_dir().join(format!("hangar-prog-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Proj")).unwrap();
    let mut conn = db::open(&root.join("test.db")).unwrap();
    scan::scan(&mut conn, &root).unwrap();
    let pid: i64 = conn
        .query_row("SELECT id FROM projects WHERE name='Proj'", [], |r| r.get(0))
        .unwrap();
    (conn, root, pid)
}

#[test]
fn weighted_progress_partial_credit() {
    let (conn, root, pid) = setup("weighted");

    // 3 milestones with weights 1/2/1: done, doing (2 of 4 tasks), todo.
    conn.execute_batch(&format!(
        "INSERT INTO milestones (project_id, title, state, weight, sort_order) VALUES
           ({pid}, 'A', 'done', 1, 0),
           ({pid}, 'B', 'doing', 2, 1),
           ({pid}, 'C', 'todo', 1, 2);"
    ))
    .unwrap();
    let mid_b: i64 = conn
        .query_row("SELECT id FROM milestones WHERE title='B'", [], |r| r.get(0))
        .unwrap();
    for i in 0..4 {
        conn.execute(
            "INSERT INTO tasks (project_id, milestone_id, title, done) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![pid, mid_b, format!("t{i}"), (i < 2) as i64],
        )
        .unwrap();
    }

    // earned = 1 (done) + 2 * 2/4 (doing partial) = 2 of 4 total → 50%
    let value = ops::weighted_progress(&conn, pid).unwrap().unwrap();
    assert_eq!(value, 50);

    // In milestones mode, recompute writes progress + history.
    conn.execute("UPDATE projects SET progress_mode='milestones' WHERE id=?1", [pid])
        .unwrap();
    ops::recompute_progress(&conn, pid).unwrap();
    let progress: i64 = conn
        .query_row("SELECT progress FROM projects WHERE id=?1", [pid], |r| r.get(0))
        .unwrap();
    assert_eq!(progress, 50);
    let hist: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM progress_history WHERE project_id=?1 AND source='milestones'",
            [pid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hist, 1);

    // Doing milestone with no tasks earns half credit.
    conn.execute("DELETE FROM tasks WHERE milestone_id=?1", [mid_b]).unwrap();
    let value = ops::weighted_progress(&conn, pid).unwrap().unwrap();
    assert_eq!(value, 50); // 1 + 2*0.5 = 2 of 4

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn health_derivation() {
    let (conn, root, pid) = setup("health");

    // Active project, no target, no history → on_track (fresh).
    conn.execute("UPDATE projects SET status='active' WHERE id=?1", [pid]).unwrap();
    let stats = commands_m2::compute_stats(&conn, pid).unwrap();
    assert_eq!(stats.health, "on_track");

    // Past target date with progress < 100 → late.
    conn.execute(
        "UPDATE projects SET target_date='2020-01-01', progress=40 WHERE id=?1",
        [pid],
    )
    .unwrap();
    let stats = commands_m2::compute_stats(&conn, pid).unwrap();
    assert_eq!(stats.health, "late");

    // Non-active projects are never flagged.
    conn.execute("UPDATE projects SET status='paused' WHERE id=?1", [pid]).unwrap();
    let stats = commands_m2::compute_stats(&conn, pid).unwrap();
    assert_eq!(stats.health, "on_track");

    std::fs::remove_dir_all(&root).unwrap();
}
