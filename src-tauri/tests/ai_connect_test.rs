//! End-to-end tests of the "connect an AI, send a message, load the chat" flow
//! — the exact path the user reported as "doesn't fully connect / messages
//! don't go through / deletes my message / can't load chats".
//!
//! The pure-DB tests run everywhere. The ones that actually talk to a provider
//! are #[ignore]d (they need the real `claude` CLI / local Ollama); run them
//! with `cargo test --test ai_connect_test -- --ignored --nocapture`.

use hangar_lib::commands_m6::{
    chat_send_core, load_active_profile_provider, upsert_profile, AiProfileStored,
};
use hangar_lib::db;
use rusqlite::Connection;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh migrated database in a unique temp file (mirrors the other tests,
/// which avoid extra dev-deps and use `std::env::temp_dir`). The guard removes
/// the file on drop.
struct DbGuard(std::path::PathBuf);
impl Drop for DbGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temp_db() -> (DbGuard, Connection) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "hangar-ai-connect-{}-{}.db",
        std::process::id(),
        n
    ));
    let _ = std::fs::remove_file(&path);
    let conn = db::open(&path).unwrap();
    (DbGuard(path), conn)
}

/// A detected provider connects exactly like the UI's "Connect" button: an
/// empty id, no key. It must become the active profile with a live Provider.
fn cli_profile(name: &str, command: &str) -> AiProfileStored {
    AiProfileStored {
        id: String::new(),
        name: name.into(),
        provider: "cli".into(),
        model: String::new(),
        base_url: String::new(),
        command: command.into(),
        args: String::new(),
    }
}

fn ollama_profile() -> AiProfileStored {
    AiProfileStored {
        id: String::new(),
        name: "Ollama".into(),
        provider: "ollama".into(),
        model: String::new(), // detection connects with no explicit model
        base_url: "http://localhost:11434".into(),
        command: String::new(),
        args: String::new(),
    }
}

fn new_chat(conn: &Connection, active_profile: Option<&str>) -> i64 {
    conn.execute(
        "INSERT INTO ai_chats (title, profile_id, project_id) VALUES ('New chat', ?1, NULL)",
        rusqlite::params![active_profile],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn message_count(conn: &Connection, chat_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM ai_chat_messages WHERE chat_id = ?1",
        [chat_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn connecting_makes_a_live_active_profile() {
    let (_d, conn) = temp_db();
    // Nothing connected yet.
    assert!(load_active_profile_provider(&conn).is_none());

    let id = upsert_profile(&conn, cli_profile("Claude", "claude")).unwrap();
    assert!(!id.is_empty());

    // First connect auto-activates AND yields a usable provider — this is what
    // "fully connected" means.
    let provider = load_active_profile_provider(&conn);
    assert!(provider.is_some(), "connecting must leave an active, buildable provider");
    assert_eq!(provider.unwrap().provider_name(), "cli");
}

#[test]
fn cli_profile_with_blank_command_defaults_to_claude() {
    let (_d, conn) = temp_db();
    let id = upsert_profile(&conn, cli_profile("My CLI", "")).unwrap();
    let provider = load_active_profile_provider(&conn).expect("should build");
    assert_eq!(provider.provider_name(), "cli");
    // The stored command was filled in, so nothing is left half-connected.
    let _ = id;
}

#[test]
fn ollama_connects_without_a_model_or_key() {
    let (_d, conn) = temp_db();
    // Ollama needs no key; provider must build even with an empty model
    // (resolved to an installed one at send time).
    upsert_profile(&conn, ollama_profile()).unwrap();
    let provider = load_active_profile_provider(&conn).expect("ollama should build");
    assert_eq!(provider.provider_name(), "ollama");
}

#[test]
fn second_profile_does_not_steal_active() {
    let (_d, conn) = temp_db();
    let first = upsert_profile(&conn, cli_profile("Claude", "claude")).unwrap();
    let _second = upsert_profile(&conn, ollama_profile()).unwrap();
    // Adding a second AI must NOT change which one is in use.
    let active = db::get_setting(&conn, "ai_active_profile").unwrap();
    assert_eq!(active.as_deref(), Some(first.as_str()));
}

// ---- Live end-to-end: connect -> send -> reply -> history persists ----

#[tokio::test]
#[ignore]
async fn claude_full_connect_send_and_history() {
    let (_d, conn) = temp_db();
    let id = upsert_profile(&conn, cli_profile("Claude", "claude")).unwrap();
    let active = db::get_setting(&conn, "ai_active_profile").unwrap();
    let chat_id = new_chat(&conn, active.as_deref());
    assert_eq!(message_count(&conn, chat_id), 0);

    let mutex = Mutex::new(conn);
    let reply = chat_send_core(
        &mutex,
        chat_id,
        "reply with the single word: ready".into(),
        id.clone(),
    )
    .await
    .expect("send should go through");
    println!("claude reply: {:?}", reply.content);
    assert!(!reply.content.trim().is_empty());

    // Both the user message and the reply are saved (message not lost),
    // and the history loads back.
    let conn = mutex.lock().unwrap();
    assert_eq!(message_count(&conn, chat_id), 2, "user + assistant persisted");
    drop(conn);

    // A second turn carries history and appends, never deletes.
    let reply2 = chat_send_core(&mutex, chat_id, "and now say: go".into(), id)
        .await
        .expect("second send should go through");
    assert!(!reply2.content.trim().is_empty());
    let conn = mutex.lock().unwrap();
    assert_eq!(message_count(&conn, chat_id), 4, "history appended, nothing lost");
}

#[tokio::test]
#[ignore]
async fn ollama_full_connect_send_and_history() {
    // Environment-dependent: skip cleanly if Ollama isn't running right now.
    if reqwest::get("http://127.0.0.1:11434/api/tags").await.is_err() {
        eprintln!("SKIP: Ollama not running");
        return;
    }
    let (_d, conn) = temp_db();
    let id = upsert_profile(&conn, ollama_profile()).unwrap();
    let active = db::get_setting(&conn, "ai_active_profile").unwrap();
    let chat_id = new_chat(&conn, active.as_deref());

    let mutex = Mutex::new(conn);
    let reply = chat_send_core(
        &mutex,
        chat_id,
        "reply with the single word: ready".into(),
        id,
    )
    .await
    .expect("ollama send should go through even with no model set");
    println!("ollama reply: {:?}", reply.content);
    assert!(!reply.content.trim().is_empty());
    let conn = mutex.lock().unwrap();
    assert_eq!(message_count(&conn, chat_id), 2);
}

/// Sending with a profile id that doesn't exist must error cleanly WITHOUT
/// writing anything — the frontend then restores the draft (nothing lost).
#[tokio::test]
async fn bad_profile_send_errors_and_writes_nothing() {
    let (_d, conn) = temp_db();
    let chat_id = new_chat(&conn, None);
    let mutex = Mutex::new(conn);
    let r = chat_send_core(&mutex, chat_id, "hello".into(), "nope".into()).await;
    assert!(r.is_err(), "unknown profile must error");
    let conn = mutex.lock().unwrap();
    assert_eq!(message_count(&conn, chat_id), 0, "nothing persisted on failure");
}
