//! Import existing AI conversations from tools already on this Mac.
//!
//! Two sources write their sessions to disk in a readable form:
//!
//! - **Claude Code** — `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl`, one JSON
//!   object per line with `type: "user" | "assistant"`.
//! - **Codex** — `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl`, whose
//!   `response_item` entries carry the actual messages. This covers the Codex
//!   *Desktop* app too: it writes the same rollout files (`originator` says
//!   "Codex Desktop"), which is why desktop conversations are importable at all.
//!
//! The Claude and ChatGPT desktop apps are deliberately absent — they keep
//! conversations server-side and store nothing locally that we could read, so
//! there is nothing to import for them.

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSession {
    /// Upstream session UUID — the dedupe key.
    pub id: String,
    /// Importer id: "claude-code" or "codex".
    pub source: String,
    pub title: String,
    pub path: String,
    /// Working directory the session ran in, used to match it to a project.
    pub cwd: Option<String>,
    pub message_count: usize,
    pub started_at: String,
    /// True when this session is already in the database.
    pub imported: bool,
}

#[derive(Debug, Clone)]
pub struct ImportedMessage {
    pub role: String,
    pub content: String,
    pub ts: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: usize,
    pub messages: usize,
    pub errors: Vec<String>,
}

fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Pull display text out of a content field that may be a bare string or the
/// block array both tools use. Tool-call and image blocks have no text and are
/// skipped — importing them would just be noise in a chat transcript.
fn extract_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Strip the wrapper blocks these tools inject into user turns. Real sessions
/// are full of `<local-command-caveat>`, `<system-reminder>`, and `<command-name>`
/// noise; without removing it, most imported chats end up titled with boilerplate
/// instead of what the conversation was about.
fn strip_injected_blocks(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        // Find the tag name, then skip through its closing tag if there is one.
        let name_end = after
            .find(|c: char| c == '>' || c.is_whitespace())
            .unwrap_or(after.len());
        let name = &after[..name_end];
        let close = format!("</{name}>");
        match after.find(&close) {
            Some(idx) => rest = &after[idx + close.len()..],
            None => match after.find('>') {
                // Self-contained or unclosed tag: drop just the tag itself.
                Some(idx) => rest = &after[idx + 1..],
                None => {
                    rest = "";
                }
            },
        }
    }
    out.push_str(rest);
    out
}

/// Trim a first message down to something usable as a chat title.
fn derive_title(messages: &[ImportedMessage], fallback: &str) -> String {
    let first = messages
        .iter()
        .find(|m| m.role == "user" && !m.content.trim().is_empty());
    let Some(m) = first else {
        return fallback.to_string();
    };
    let cleaned = strip_injected_blocks(&m.content);
    let line = cleaned
        .lines()
        .map(str::trim)
        // Drop the "User:" prefix some transcripts carry, and skip empty lines.
        .map(|l| l.strip_prefix("User:").unwrap_or(l).trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.is_empty() {
        return fallback.to_string();
    }
    let mut t: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        t.push('…');
    }
    t
}

// ---------------------------------------------------------------- Claude Code

/// Returns (messages, cwd, native_title). `native_title` is the name Claude Code
/// itself shows for the session — it writes an `ai-title` record into the JSONL —
/// so imported chats read exactly as they do in the tool they came from.
pub fn parse_claude_code(
    path: &Path,
) -> AppResult<(Vec<ImportedMessage>, Option<String>, Option<String>)> {
    let text = std::fs::read_to_string(path)?;
    let mut messages = Vec::new();
    let mut cwd = None;
    let mut native_title = None;

    for line in text.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // tolerate partial/corrupt trailing lines
        };
        if cwd.is_none() {
            if let Some(c) = v.get("cwd").and_then(|c| c.as_str()) {
                cwd = Some(c.to_string());
            }
        }
        // Claude Code's own name for the chat — later records win, since the
        // title is refined as the conversation grows.
        if v.get("type").and_then(|t| t.as_str()) == Some("ai-title") {
            if let Some(t) = v.get("aiTitle").and_then(|t| t.as_str()) {
                if !t.trim().is_empty() {
                    native_title = Some(t.trim().to_string());
                }
            }
            continue;
        }
        let role = match v.get("type").and_then(|t| t.as_str()) {
            Some(r @ ("user" | "assistant")) => r,
            _ => continue,
        };
        let content = v
            .get("message")
            .and_then(|m| m.get("content"))
            .map(extract_text)
            .unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        messages.push(ImportedMessage {
            role: role.to_string(),
            content,
            ts: v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    Ok((messages, cwd, native_title))
}

// ---------------------------------------------------------------------- Codex

pub fn parse_codex(
    path: &Path,
) -> AppResult<(Vec<ImportedMessage>, Option<String>, Option<String>)> {
    let text = std::fs::read_to_string(path)?;
    let mut messages = Vec::new();
    let mut cwd = None;

    for line in text.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let payload = match v.get("payload") {
            Some(p) => p,
            None => continue,
        };

        if kind == "session_meta" || kind == "turn_context" {
            if cwd.is_none() {
                if let Some(c) = payload.get("cwd").and_then(|c| c.as_str()) {
                    cwd = Some(c.to_string());
                }
            }
            continue;
        }
        if kind != "response_item" {
            continue;
        }
        if payload.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        // 'developer' carries injected system prompts, not conversation.
        let role = match payload.get("role").and_then(|r| r.as_str()) {
            Some(r @ ("user" | "assistant")) => r,
            _ => continue,
        };
        let content = payload.get("content").map(extract_text).unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }
        messages.push(ImportedMessage {
            role: role.to_string(),
            content,
            ts: v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    // Codex writes no title record of its own, so callers fall back to deriving
    // one from the first user message.
    Ok((messages, cwd, None))
}

// --------------------------------------------------- cloud data-export files
//
// The Claude and ChatGPT desktop apps keep conversations server-side, and
// neither vendor offers an API to read your chat history. The supported route
// is each service's official data export, which mails you a `conversations.json`.
// Parsing that file is how cloud conversations get into Hangar — no credentials
// involved, nothing that breaks when a cookie rotates.

/// Claude export: `[{ uuid, name, created_at, chat_messages: [...] }]`
pub fn parse_claude_export(v: &serde_json::Value) -> Vec<(String, String, Vec<ImportedMessage>)> {
    let mut out = Vec::new();
    for conv in v.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let id = conv
            .get("uuid")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let mut messages = Vec::new();
        for m in conv
            .get("chat_messages")
            .and_then(|c| c.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
        {
            let role = match m.get("sender").and_then(|s| s.as_str()) {
                Some("human") => "user",
                Some("assistant") => "assistant",
                _ => continue,
            };
            // Newer exports use a `content` block array; older ones a flat `text`.
            let content = m
                .get("content")
                .map(extract_text)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| m.get("text").and_then(|t| t.as_str()).map(String::from))
                .unwrap_or_default();
            if content.trim().is_empty() {
                continue;
            }
            messages.push(ImportedMessage {
                role: role.to_string(),
                content,
                ts: m
                    .get("created_at")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
        if messages.is_empty() {
            continue;
        }
        let title = conv
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|n| !n.trim().is_empty())
            .map(String::from)
            .unwrap_or_else(|| derive_title(&messages, "Claude conversation"));
        out.push((id, title, messages));
    }
    out
}

/// ChatGPT export: `[{ id, title, mapping: { <node>: { message: {...} } } }]`.
/// The mapping is a tree; ordering by `create_time` reconstructs the transcript
/// well enough for reading, which is all an imported chat needs to be.
pub fn parse_chatgpt_export(v: &serde_json::Value) -> Vec<(String, String, Vec<ImportedMessage>)> {
    let mut out = Vec::new();
    for conv in v.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let id = conv
            .get("id")
            .or_else(|| conv.get("conversation_id"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let mapping = match conv.get("mapping").and_then(|m| m.as_object()) {
            Some(m) => m,
            None => continue,
        };
        let mut rows: Vec<(f64, ImportedMessage)> = Vec::new();
        for node in mapping.values() {
            let msg = match node.get("message") {
                Some(m) if !m.is_null() => m,
                _ => continue,
            };
            let role = match msg
                .get("author")
                .and_then(|a| a.get("role"))
                .and_then(|r| r.as_str())
            {
                Some(r @ ("user" | "assistant")) => r,
                _ => continue, // skips system/tool nodes
            };
            let content = msg
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| {
                            p.as_str().map(String::from).or_else(|| {
                                p.get("text").and_then(|t| t.as_str()).map(String::from)
                            })
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            if content.trim().is_empty() {
                continue;
            }
            let ct = msg.get("create_time").and_then(|t| t.as_f64()).unwrap_or(0.0);
            rows.push((
                ct,
                ImportedMessage {
                    role: role.to_string(),
                    content,
                    ts: unix_to_iso(ct),
                },
            ));
        }
        if rows.is_empty() {
            continue;
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let messages: Vec<ImportedMessage> = rows.into_iter().map(|(_, m)| m).collect();
        let title = conv
            .get("title")
            .and_then(|n| n.as_str())
            .filter(|n| !n.trim().is_empty())
            .map(String::from)
            .unwrap_or_else(|| derive_title(&messages, "ChatGPT conversation"));
        out.push((id, title, messages));
    }
    out
}

fn unix_to_iso(secs: f64) -> String {
    if secs <= 0.0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

/// Import a `conversations.json` from a Claude or ChatGPT data export. The
/// format is detected from its shape, so the user just picks the file.
pub fn import_export_file(
    conn: &mut Connection,
    path: &Path,
    profile_id: Option<&str>,
) -> AppResult<ImportSummary> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        AppError::msg(format!("couldn't read {}: {e}", path.display()))
    })?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| AppError::msg(format!("that isn't a valid export file: {e}")))?;

    let arr = v.as_array().ok_or_else(|| {
        AppError::msg("expected a conversations.json containing a list of conversations")
    })?;
    let first = arr.first();
    let (source, convs) = if first.map(|c| c.get("mapping").is_some()).unwrap_or(false) {
        ("chatgpt-export", parse_chatgpt_export(&v))
    } else if first
        .map(|c| c.get("chat_messages").is_some())
        .unwrap_or(false)
    {
        ("claude-export", parse_claude_export(&v))
    } else {
        return Err(AppError::msg(
            "unrecognized export format — pick the conversations.json from a Claude or ChatGPT data export",
        ));
    };

    let mut summary = ImportSummary::default();
    for (id, title, messages) in convs {
        let started = messages.first().map(|m| m.ts.clone()).unwrap_or_default();
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "INSERT OR IGNORE INTO ai_chats
               (title, profile_id, project_id, source, external_id, source_path, created_at, updated_at)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![
                title,
                profile_id,
                source,
                id,
                path.to_string_lossy(),
                if started.is_empty() { None } else { Some(&started) },
            ],
        )?;
        if changed == 0 {
            tx.rollback()?;
            summary.skipped += 1;
            continue;
        }
        let chat_id = tx.last_insert_rowid();
        for m in &messages {
            tx.execute(
                "INSERT INTO ai_chat_messages (chat_id, role, content, provider, model, ts)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
                rusqlite::params![
                    chat_id,
                    m.role,
                    m.content,
                    source,
                    if m.ts.is_empty() { None } else { Some(&m.ts) },
                ],
            )?;
            summary.messages += 1;
        }
        tx.commit()?;
        summary.imported += 1;
    }
    Ok(summary)
}

// ------------------------------------------------------------------ discovery

fn jsonl_files_under(dir: &Path, prefix: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = match std::fs::read_dir(&d) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if prefix.map(|pre| name.starts_with(pre)).unwrap_or(true) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Extract the session UUID from a filename: Claude Code names files
/// `<uuid>.jsonl`; Codex names them `rollout-<timestamp>-<uuid>.jsonl`.
fn session_id_from_name(path: &Path, source: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if source == "codex" {
        // Take the trailing 5 dash-separated UUID groups.
        let parts: Vec<&str> = stem.split('-').collect();
        if parts.len() >= 5 {
            return parts[parts.len() - 5..].join("-");
        }
    }
    stem
}

pub fn discover_sessions(conn: &Connection) -> AppResult<Vec<DiscoveredSession>> {
    let existing = existing_external_ids(conn)?;
    let mut out = Vec::new();

    let sources: [(&str, PathBuf, Option<&str>); 3] = [
        ("claude-code", home().join(".claude/projects"), None),
        ("codex", home().join(".codex/sessions"), Some("rollout-")),
        (
            "codex",
            home().join(".codex/archived_sessions"),
            Some("rollout-"),
        ),
    ];

    for (source, dir, prefix) in sources {
        if !dir.exists() {
            continue;
        }
        for path in jsonl_files_under(&dir, prefix) {
            let (messages, cwd, native_title) = match source {
                "claude-code" => parse_claude_code(&path),
                _ => parse_codex(&path),
            }
            .unwrap_or_else(|_| (Vec::new(), None, None));

            if messages.is_empty() {
                continue; // empty or unreadable session — nothing to show
            }
            let id = session_id_from_name(&path, source);
            let started_at = messages
                .first()
                .map(|m| m.ts.clone())
                .unwrap_or_default();
            out.push(DiscoveredSession {
                imported: existing.contains(&(source.to_string(), id.clone())),
                // Show the name the source tool shows, so chats are recognisable.
                title: native_title
                    .unwrap_or_else(|| derive_title(&messages, "Imported chat")),
                id,
                source: source.to_string(),
                path: path.to_string_lossy().to_string(),
                cwd,
                message_count: messages.len(),
                started_at,
            });
        }
    }

    // Newest first so the most relevant conversations are at the top.
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(out)
}

fn existing_external_ids(conn: &Connection) -> AppResult<std::collections::HashSet<(String, String)>> {
    let mut stmt =
        conn.prepare("SELECT source, external_id FROM ai_chats WHERE external_id IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.flatten().collect())
}

// --------------------------------------------------------------------- import

/// Match a session's working directory to an existing Hangar project by path.
fn project_for_cwd(conn: &Connection, cwd: Option<&str>) -> Option<i64> {
    let cwd = cwd?;
    let mut stmt = conn.prepare("SELECT id, path FROM projects").ok()?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .ok()?;
    let mut best: Option<(i64, usize)> = None;
    for (id, path) in rows.flatten() {
        if !path.is_empty() && cwd.starts_with(&path) {
            // Longest matching prefix wins, so nested projects resolve correctly.
            if best.map(|(_, len)| path.len() > len).unwrap_or(true) {
                best = Some((id, path.len()));
            }
        }
    }
    best.map(|(id, _)| id)
}

pub fn import_sessions(
    conn: &mut Connection,
    sessions: &[DiscoveredSession],
    profile_id: Option<&str>,
) -> AppResult<ImportSummary> {
    let mut summary = ImportSummary::default();

    for s in sessions {
        let path = PathBuf::from(&s.path);
        let parsed = match s.source.as_str() {
            "claude-code" => parse_claude_code(&path),
            "codex" => parse_codex(&path),
            other => Err(AppError::msg(format!("unknown import source: {other}"))),
        };
        let (messages, cwd, _native) = match parsed {
            Ok(v) => v,
            Err(e) => {
                summary.errors.push(format!("{}: {e}", s.title));
                continue;
            }
        };
        if messages.is_empty() {
            summary.skipped += 1;
            continue;
        }

        let project_id = project_for_cwd(conn, cwd.as_deref());
        let tx = conn.transaction()?;

        // The UNIQUE index on (source, external_id) makes this a no-op on
        // re-import, which is what keeps the button safe to press repeatedly.
        let changed = tx.execute(
            "INSERT OR IGNORE INTO ai_chats
               (title, profile_id, project_id, source, external_id, source_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![
                s.title,
                profile_id,
                project_id,
                s.source,
                s.id,
                s.path,
                if s.started_at.is_empty() { None } else { Some(&s.started_at) },
            ],
        )?;
        if changed == 0 {
            tx.rollback()?;
            summary.skipped += 1;
            continue;
        }
        let chat_id = tx.last_insert_rowid();

        for m in &messages {
            tx.execute(
                "INSERT INTO ai_chat_messages (chat_id, role, content, provider, model, ts)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
                rusqlite::params![
                    chat_id,
                    m.role,
                    m.content,
                    s.source,
                    if m.ts.is_empty() { None } else { Some(&m.ts) },
                ],
            )?;
            summary.messages += 1;
        }
        tx.commit()?;
        summary.imported += 1;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_string_and_blocks() {
        assert_eq!(extract_text(&serde_json::json!("hi")), "hi");
        assert_eq!(
            extract_text(&serde_json::json!([
                {"type": "text", "text": "one"},
                {"type": "tool_use", "id": "x"},
                {"type": "text", "text": "two"}
            ])),
            "one\ntwo"
        );
        assert_eq!(extract_text(&serde_json::json!({})), "");
    }

    #[test]
    fn derives_title_from_first_user_message() {
        let msgs = vec![
            ImportedMessage {
                role: "user".into(),
                content: "<caveat>noise</caveat>\nfix the build".into(),
                ts: String::new(),
            },
        ];
        assert_eq!(derive_title(&msgs, "fallback"), "fix the build");
        assert_eq!(derive_title(&[], "fallback"), "fallback");
    }

    #[test]
    fn title_ignores_injected_wrapper_blocks() {
        // Taken from real Claude Code sessions on disk, which otherwise produced
        // titles like "<local-command-caveat>Caveat: The messages below…".
        let msgs = vec![ImportedMessage {
            role: "user".into(),
            content: "<local-command-caveat>Caveat: The messages below were generated by the user while running local commands.</local-command-caveat>\nsummarize this project".into(),
            ts: String::new(),
        }];
        assert_eq!(derive_title(&msgs, "fallback"), "summarize this project");

        // "User:" prefixes appear in some transcripts and shouldn't survive.
        let prefixed = vec![ImportedMessage {
            role: "user".into(),
            content: "User: reply with the single word: ready".into(),
            ts: String::new(),
        }];
        assert_eq!(
            derive_title(&prefixed, "fallback"),
            "reply with the single word: ready"
        );

        // Nothing but injected noise falls back rather than titling with junk.
        let only_noise = vec![ImportedMessage {
            role: "user".into(),
            content: "<system-reminder>internal</system-reminder>".into(),
            ts: String::new(),
        }];
        assert_eq!(derive_title(&only_noise, "fallback"), "fallback");
    }

    #[test]
    fn pulls_uuid_out_of_codex_filename() {
        let p = PathBuf::from("rollout-2026-07-16T20-30-41-019f6d7b-8ef1-77c1-9660-966ab333a084.jsonl");
        assert_eq!(
            session_id_from_name(&p, "codex"),
            "019f6d7b-8ef1-77c1-9660-966ab333a084"
        );
        let c = PathBuf::from("9270589c-1fb4-449b-a701-80955a864781.jsonl");
        assert_eq!(
            session_id_from_name(&c, "claude-code"),
            "9270589c-1fb4-449b-a701-80955a864781"
        );
    }

    #[test]
    fn parses_chatgpt_export_ordering_and_skips_system() {
        let v = serde_json::json!([{
            "id": "conv-1",
            "title": "My chat",
            "mapping": {
                "c": {"message": {"author": {"role": "assistant"}, "create_time": 3.0,
                       "content": {"parts": ["second"]}}},
                "a": {"message": {"author": {"role": "system"}, "create_time": 1.0,
                       "content": {"parts": ["hidden"]}}},
                "b": {"message": {"author": {"role": "user"}, "create_time": 2.0,
                       "content": {"parts": ["first"]}}}
            }
        }]);
        let out = parse_chatgpt_export(&v);
        assert_eq!(out.len(), 1);
        let (id, title, msgs) = &out[0];
        assert_eq!(id, "conv-1");
        assert_eq!(title, "My chat");
        assert_eq!(msgs.len(), 2, "system role must be skipped");
        assert_eq!(msgs[0].content, "first", "must sort by create_time");
        assert_eq!(msgs[1].content, "second");
    }

    #[test]
    fn parses_claude_export_both_content_shapes() {
        let v = serde_json::json!([{
            "uuid": "u-1",
            "name": "Design talk",
            "chat_messages": [
                {"sender": "human", "text": "old style", "created_at": "2026-01-01T00:00:00Z"},
                {"sender": "assistant", "content": [{"type": "text", "text": "new style"}],
                 "created_at": "2026-01-01T00:00:01Z"}
            ]
        }]);
        let out = parse_claude_export(&v);
        assert_eq!(out.len(), 1);
        let (_, title, msgs) = &out[0];
        assert_eq!(title, "Design talk");
        assert_eq!(msgs[0].content, "old style");
        assert_eq!(msgs[1].content, "new style");
    }

    #[test]
    fn prefers_claude_codes_own_ai_title() {
        let dir = std::env::temp_dir().join("hangar_import_title_test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.jsonl");
        std::fs::write(&f, concat!(
            r#"{"type":"user","timestamp":"2026-01-01T00:00:00Z","message":{"content":"do a thing"}}"#, "\n",
            r#"{"type":"ai-title","aiTitle":"Polish hanger app design","sessionId":"x"}"#, "\n",
            r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01Z","message":{"content":"ok"}}"#, "\n",
        )).unwrap();
        let (msgs, _cwd, title) = parse_claude_code(&f).unwrap();
        assert_eq!(msgs.len(), 2, "ai-title record must not become a message");
        assert_eq!(title.as_deref(), Some("Polish hanger app design"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_claude_code_jsonl() {
        let dir = std::env::temp_dir().join("hangar_import_cc_test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("s.jsonl");
        std::fs::write(
            &f,
            r#"{"type":"user","cwd":"/Users/x/proj","timestamp":"2026-01-01T00:00:00Z","message":{"content":"hello"}}
{"type":"queue-operation","content":"ignored"}
{"type":"assistant","timestamp":"2026-01-01T00:00:05Z","message":{"content":[{"type":"text","text":"hi back"}]}}
not json at all
"#,
        )
        .unwrap();
        let (msgs, cwd, title) = parse_claude_code(&f).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].content, "hi back");
        assert_eq!(cwd.as_deref(), Some("/Users/x/proj"));
        assert_eq!(title, None, "no ai-title record in this fixture");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_codex_rollout_and_skips_developer_role() {
        let dir = std::env::temp_dir().join("hangar_import_codex_test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("rollout-2026-01-01T00-00-00-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        std::fs::write(
            &f,
            r#"{"type":"session_meta","timestamp":"2026-01-01T00:00:00Z","payload":{"cwd":"/Users/x/code"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system prompt"}]}}
{"type":"response_item","timestamp":"2026-01-01T00:00:02Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"question"}]}}
{"type":"event_msg","payload":{"type":"task_started"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:03Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}}
"#,
        )
        .unwrap();
        let (msgs, cwd, _t) = parse_codex(&f).unwrap();
        assert_eq!(msgs.len(), 2, "developer role must be skipped");
        assert_eq!(msgs[0].content, "question");
        assert_eq!(msgs[1].content, "answer");
        assert_eq!(cwd.as_deref(), Some("/Users/x/code"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
