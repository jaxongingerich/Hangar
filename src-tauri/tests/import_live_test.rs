//! Live tests against the real session files on this Mac.
//!
//! These prove the importers work on actual Claude Code / Codex output rather
//! than on hand-written fixtures. They skip cleanly when the directories don't
//! exist, so they're safe to run anywhere.
//!
//! Run with: cargo test --test import_live_test -- --ignored --nocapture

use hangar_lib::{db, import};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct DbGuard(PathBuf);
impl DbGuard {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("hangar_import_live_{tag}_{n}.db"));
        std::fs::remove_file(&p).ok();
        DbGuard(p)
    }
}
impl Drop for DbGuard {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

#[test]
#[ignore]
fn discovers_real_sessions_on_this_mac() {
    let g = DbGuard::new("discover");
    let conn = db::open(&g.0).expect("open db");

    let sessions = import::discover_sessions(&conn).expect("discover");
    println!("discovered {} sessions", sessions.len());

    let cc = sessions.iter().filter(|s| s.source == "claude-code").count();
    let cx = sessions.iter().filter(|s| s.source == "codex").count();
    println!("  claude-code: {cc}");
    println!("  codex:       {cx}");

    for s in sessions.iter().take(8) {
        println!(
            "  [{}] {:>4} msgs  {}  {}",
            s.source,
            s.message_count,
            &s.started_at.chars().take(19).collect::<String>(),
            s.title
        );
    }

    if home().join(".claude/projects").exists() {
        assert!(cc > 0, "Claude Code sessions exist on disk but none parsed");
    }
    if home().join(".codex/sessions").exists() {
        assert!(cx > 0, "Codex sessions exist on disk but none parsed");
    }
    // Every discovered session must carry the fields the UI depends on.
    for s in &sessions {
        assert!(!s.id.is_empty(), "session id must not be empty");
        assert!(!s.title.trim().is_empty(), "title must not be empty");
        assert!(s.message_count > 0, "empty sessions must be filtered out");
    }
}

#[test]
#[ignore]
fn imports_real_sessions_and_is_idempotent() {
    let g = DbGuard::new("import");
    let mut conn = db::open(&g.0).expect("open db");

    let sessions = import::discover_sessions(&conn).expect("discover");
    if sessions.is_empty() {
        println!("no sessions on this machine — skipping");
        return;
    }
    let take: Vec<_> = sessions.into_iter().take(5).collect();
    let expected_msgs: usize = take.iter().map(|s| s.message_count).sum();

    let first = import::import_sessions(&mut conn, &take, Some("p-test")).expect("import");
    println!(
        "first import: {} chats, {} messages, {} skipped, errors={:?}",
        first.imported, first.messages, first.skipped, first.errors
    );
    assert_eq!(first.imported, take.len(), "all sessions should import");
    assert_eq!(first.messages, expected_msgs, "message counts must match");
    assert!(first.errors.is_empty(), "no errors expected");

    // Re-importing the same sessions must not duplicate anything — this is the
    // whole point of the UNIQUE(source, external_id) index.
    let second = import::import_sessions(&mut conn, &take, Some("p-test")).expect("reimport");
    println!(
        "second import: {} imported, {} skipped",
        second.imported, second.skipped
    );
    assert_eq!(second.imported, 0, "re-import must import nothing");
    assert_eq!(second.skipped, take.len(), "re-import must skip everything");

    let chats: i64 = conn
        .query_row("SELECT COUNT(*) FROM ai_chats", [], |r| r.get(0))
        .unwrap();
    let msgs: i64 = conn
        .query_row("SELECT COUNT(*) FROM ai_chat_messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(chats as usize, take.len(), "no duplicate chats");
    assert_eq!(msgs as usize, expected_msgs, "no duplicate messages");

    // Discovery must now report them as already imported.
    let after = import::discover_sessions(&conn).expect("rediscover");
    let marked = after.iter().filter(|s| s.imported).count();
    assert_eq!(marked, take.len(), "imported sessions must be flagged");
    println!("verified: {chats} chats / {msgs} messages, no duplicates");
}
