//! AI hub: multiple provider profiles, persistent chats, import suggestions,
//! and the MCP server switch.

use crate::ai::{ChatMessage, CliFlavor, Provider};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------- Provider profiles ----------
//
// Profiles are stored as JSON in the settings table under `ai_profiles`.
// The active profile also mirrors into the legacy `ai_provider` /
// `ai_model` / `ai_base_url` keys so every existing AI feature keeps working.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProfileStored {
    pub id: String,
    pub name: String,
    /// "anthropic" | "ollama" | "openai" | "cli"
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: String,
    /// For provider == "cli": the binary to run (e.g. "claude").
    #[serde(default)]
    pub command: String,
    /// For provider == "cli" with a non-"claude" command: extra flags to pass
    /// before the prompt, space-separated. No shell involved — plain argv.
    #[serde(default)]
    pub args: String,
}

#[derive(Serialize)]
pub struct AiProfileInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub command: String,
    pub has_key: bool,
    pub needs_key: bool,
    pub active: bool,
}

fn profile_keyring(id: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(crate::ai::KEYRING_SERVICE, &format!("key_{id}"))
        .map_err(|e| AppError::msg(format!("keychain unavailable: {e}")))
}

fn legacy_key() -> Option<String> {
    keyring::Entry::new(crate::ai::KEYRING_SERVICE, crate::ai::KEYRING_USER)
        .ok()?
        .get_password()
        .ok()
}

fn profile_key(id: &str) -> Option<String> {
    profile_keyring(id).ok()?.get_password().ok()
}

fn load_profiles(conn: &Connection) -> AppResult<Vec<AiProfileStored>> {
    let mut profiles: Vec<AiProfileStored> = db::get_setting(conn, "ai_profiles")?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    // First run: seed from the legacy single-provider config, if any.
    if profiles.is_empty() {
        let provider = db::get_setting(conn, "ai_provider")?.unwrap_or_else(|| "none".into());
        if provider != "none" && !provider.is_empty() {
            let name = match provider.as_str() {
                "anthropic" => "Claude",
                "ollama" => "Ollama",
                _ => "Custom endpoint",
            };
            profiles.push(AiProfileStored {
                id: "default".into(),
                name: name.into(),
                provider,
                model: db::get_setting(conn, "ai_model")?.unwrap_or_default(),
                base_url: db::get_setting(conn, "ai_base_url")?.unwrap_or_default(),
                command: String::new(),
                args: String::new(),
            });
            save_profiles(conn, &profiles)?;
            db::set_setting(conn, "ai_active_profile", "default")?;
        }
    }
    Ok(profiles)
}

fn save_profiles(conn: &Connection, profiles: &[AiProfileStored]) -> AppResult<()> {
    db::set_setting(conn, "ai_profiles", &serde_json::to_string(profiles)?)
}

fn active_profile_id(conn: &Connection) -> Option<String> {
    db::get_setting(conn, "ai_active_profile").ok().flatten()
}

/// Build a Provider from a stored profile. Used by chats (any profile) and by
/// the legacy single-provider path (active profile).
pub fn provider_from_profile(p: &AiProfileStored) -> AppResult<Provider> {
    match p.provider.as_str() {
        "anthropic" => {
            let key = profile_key(&p.id).or_else(legacy_key).ok_or_else(|| {
                AppError::msg(format!("no API key saved for \"{}\" — add one in the AI tab", p.name))
            })?;
            Ok(Provider::Anthropic {
                key,
                model: if p.model.is_empty() { "claude-sonnet-4-6".into() } else { p.model.clone() },
            })
        }
        "ollama" => Ok(Provider::Ollama {
            base: if p.base_url.is_empty() { "http://localhost:11434".into() } else { p.base_url.clone() },
            model: if p.model.is_empty() { "llama3.2".into() } else { p.model.clone() },
        }),
        "openai" => {
            if p.base_url.is_empty() {
                return Err(AppError::msg(format!(
                    "profile \"{}\" needs a base URL (e.g. https://api.openai.com/v1)",
                    p.name
                )));
            }
            Ok(Provider::OpenAiCompat {
                base: p.base_url.clone(),
                key: profile_key(&p.id),
                model: p.model.clone(),
            })
        }
        "cli" => {
            if p.command.trim().is_empty() {
                return Err(AppError::msg(format!(
                    "profile \"{}\" has no command set",
                    p.name
                )));
            }
            let flavor = if p.command.trim() == "claude" {
                CliFlavor::ClaudeCode
            } else {
                CliFlavor::Custom
            };
            Ok(Provider::Cli {
                command: p.command.trim().to_string(),
                model: if p.model.is_empty() { None } else { Some(p.model.clone()) },
                extra_args: p.args.split_whitespace().map(String::from).collect(),
                flavor,
            })
        }
        other => Err(AppError::msg(format!("unknown provider kind: {other}"))),
    }
}

/// Whether a profile needs a stored API key at all — local servers and CLI
/// profiles authenticate some other way (nothing, or the CLI's own login).
fn needs_key(provider: &str) -> bool {
    provider != "ollama" && provider != "cli"
}

/// Provider for the active profile, if profiles are in use.
pub fn load_active_profile_provider(conn: &Connection) -> Option<Provider> {
    let id = active_profile_id(conn)?;
    let profiles = load_profiles(conn).ok()?;
    let p = profiles.into_iter().find(|p| p.id == id)?;
    provider_from_profile(&p).ok()
}

fn profile_by_id(conn: &Connection, id: &str) -> AppResult<AiProfileStored> {
    load_profiles(conn)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::msg("AI profile not found"))
}

#[tauri::command]
pub fn ai_list_profiles(state: State<AppState>) -> AppResult<Vec<AiProfileInfo>> {
    let conn = state.conn.lock().unwrap();
    let active = active_profile_id(&conn);
    Ok(load_profiles(&conn)?
        .into_iter()
        .map(|p| AiProfileInfo {
            has_key: profile_key(&p.id).is_some()
                || (p.provider == "anthropic" && legacy_key().is_some()),
            needs_key: needs_key(&p.provider),
            active: active.as_deref() == Some(p.id.as_str()),
            command: p.command,
            id: p.id,
            name: p.name,
            provider: p.provider,
            model: p.model,
            base_url: p.base_url,
        })
        .collect())
}

#[tauri::command]
pub fn ai_save_profile(state: State<AppState>, profile: AiProfileStored) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    upsert_profile(&conn, profile)
}

/// Create or update a profile, returning its id. A brand-new first profile is
/// activated automatically so "connect" always leaves something in use — this
/// is the guts of the connect flow and is covered directly by tests.
pub fn upsert_profile(conn: &Connection, mut profile: AiProfileStored) -> AppResult<String> {
    if profile.name.trim().is_empty() {
        return Err(AppError::msg("give the profile a name"));
    }
    // A CLI profile with no command defaults to `claude` (the common case).
    if profile.provider == "cli" && profile.command.trim().is_empty() {
        profile.command = "claude".into();
    }
    if profile.id.is_empty() {
        profile.id = format!("p{}", chrono::Utc::now().timestamp_millis());
    }
    let id = profile.id.clone();
    let mut profiles = load_profiles(conn)?;
    match profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(existing) => *existing = profile,
        None => profiles.push(profile),
    }
    save_profiles(conn, &profiles)?;
    // First profile becomes active automatically.
    if active_profile_id(conn).filter(|s| !s.is_empty()).is_none() {
        activate(conn, &id)?;
    }
    Ok(id)
}

#[tauri::command]
pub fn ai_delete_profile(state: State<AppState>, id: String) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let mut profiles = load_profiles(&conn)?;
    profiles.retain(|p| p.id != id);
    save_profiles(&conn, &profiles)?;
    if let Ok(entry) = profile_keyring(&id) {
        let _ = entry.delete_credential();
    }
    if active_profile_id(&conn).as_deref() == Some(id.as_str()) {
        match profiles.first() {
            Some(p) => activate(&conn, &p.id)?,
            None => {
                db::set_setting(&conn, "ai_active_profile", "")?;
                db::set_setting(&conn, "ai_provider", "none")?;
            }
        }
    }
    Ok(())
}

fn activate(conn: &Connection, id: &str) -> AppResult<()> {
    let p = profile_by_id(conn, id)?;
    db::set_setting(conn, "ai_active_profile", id)?;
    // Mirror into the legacy keys so every existing AI feature follows along.
    db::set_setting(conn, "ai_provider", &p.provider)?;
    db::set_setting(conn, "ai_model", &p.model)?;
    db::set_setting(conn, "ai_base_url", &p.base_url)?;
    Ok(())
}

#[tauri::command]
pub fn ai_activate_profile(state: State<AppState>, id: String) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    activate(&conn, &id)
}

#[tauri::command]
pub fn ai_set_profile_key(state: State<AppState>, id: String, key: String) -> AppResult<()> {
    let _conn = state.conn.lock().unwrap();
    let entry = profile_keyring(&id)?;
    if key.trim().is_empty() {
        let _ = entry.delete_credential();
    } else {
        entry
            .set_password(key.trim())
            .map_err(|e| AppError::msg(format!("keychain write failed: {e}")))?;
    }
    Ok(())
}

// ---------- Chats ----------

#[derive(Serialize)]
pub struct ChatRow {
    pub id: i64,
    pub title: String,
    pub profile_id: Option<String>,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub message_count: i64,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ChatMessageRow {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub ts: String,
}

#[tauri::command]
pub fn ai_list_chats(state: State<AppState>) -> AppResult<Vec<ChatRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title, c.profile_id, c.project_id, p.name,
                (SELECT COUNT(*) FROM ai_chat_messages m WHERE m.chat_id = c.id),
                c.updated_at
         FROM ai_chats c LEFT JOIN projects p ON p.id = c.project_id
         ORDER BY c.updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ChatRow {
                id: r.get(0)?,
                title: r.get(1)?,
                profile_id: r.get(2)?,
                project_id: r.get(3)?,
                project_name: r.get(4)?,
                message_count: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn ai_new_chat(state: State<AppState>, project_id: Option<i64>) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    let profile = active_profile_id(&conn).filter(|s| !s.is_empty());
    conn.execute(
        "INSERT INTO ai_chats (title, profile_id, project_id) VALUES ('New chat', ?1, ?2)",
        params![profile, project_id],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn ai_update_chat(
    state: State<AppState>,
    chat_id: i64,
    title: Option<String>,
    project_id: Option<i64>,
    clear_project: Option<bool>,
) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    if let Some(t) = title {
        conn.execute("UPDATE ai_chats SET title = ?2 WHERE id = ?1", params![chat_id, t])?;
    }
    if clear_project == Some(true) {
        conn.execute("UPDATE ai_chats SET project_id = NULL WHERE id = ?1", [chat_id])?;
    } else if let Some(pid) = project_id {
        conn.execute("UPDATE ai_chats SET project_id = ?2 WHERE id = ?1", params![chat_id, pid])?;
    }
    Ok(())
}

#[tauri::command]
pub fn ai_delete_chat(state: State<AppState>, chat_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM ai_chats WHERE id = ?1", [chat_id])?;
    Ok(())
}

#[tauri::command]
pub fn ai_chat_history(state: State<AppState>, chat_id: i64) -> AppResult<Vec<ChatMessageRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, role, content, provider, model, ts
         FROM ai_chat_messages WHERE chat_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([chat_id], |r| {
            Ok(ChatMessageRow {
                id: r.get(0)?,
                role: r.get(1)?,
                content: r.get(2)?,
                provider: r.get(3)?,
                model: r.get(4)?,
                ts: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// One chat turn. Runs the chosen profile over the whole conversation (plus
/// project context if the chat is linked to a project), and persists BOTH the
/// user message and the reply only once the AI answers successfully — so a
/// failed send never loses or half-writes anything. Switching profiles
/// mid-chat carries the full history to the new AI.
#[tauri::command]
pub async fn ai_chat_send(
    state: State<'_, AppState>,
    chat_id: i64,
    content: String,
    profile_id: String,
) -> AppResult<ChatMessageRow> {
    chat_send_core(&state.conn, chat_id, content, profile_id).await
}

/// The real send logic, independent of Tauri so it can be tested end-to-end:
/// build the conversation, call the provider, and persist BOTH messages only on
/// success. Takes just the connection mutex — nothing else from AppState.
pub async fn chat_send_core(
    conn_mutex: &std::sync::Mutex<Connection>,
    chat_id: i64,
    content: String,
    profile_id: String,
) -> AppResult<ChatMessageRow> {
    if content.trim().is_empty() {
        return Err(AppError::msg("empty message"));
    }
    let (provider, prov_name, model_name, system, history) = {
        let conn = conn_mutex.lock().unwrap();
        let profile = profile_by_id(&conn, &profile_id)?;
        let provider = provider_from_profile(&profile)?;

        let project_id: Option<i64> = conn
            .query_row("SELECT project_id FROM ai_chats WHERE id = ?1", [chat_id], |r| r.get(0))
            .optional()?
            .flatten();
        let mut system = String::from(
            "You are Hangar's assistant for a solo hardware/software maker. \
             Be direct, concrete and brief. When files are pasted into the \
             conversation, use their contents.",
        );
        if let Some(pid) = project_id {
            if let Ok(brief) = crate::commands_m5::project_brief(&conn, pid) {
                system.push_str("\n\nCurrent project context:\n");
                system.push_str(&brief);
            }
        }

        // Build the history in memory (persisted rows + the new message).
        // Nothing is written yet — that only happens if the AI answers.
        let mut history: Vec<ChatMessage> = {
            let mut stmt = conn.prepare(
                "SELECT role, content FROM ai_chat_messages WHERE chat_id = ?1 ORDER BY id",
            )?;
            let rows = stmt
                .query_map([chat_id], |r| {
                    Ok(ChatMessage { role: r.get(0)?, content: r.get(1)? })
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };
        history.push(ChatMessage { role: "user".into(), content: content.clone() });
        (
            provider.clone(),
            provider.provider_name().to_string(),
            provider.model_name().to_string(),
            system,
            history,
        )
    };

    let result = provider.chat(&system, &history).await;
    let conn = conn_mutex.lock().unwrap();
    match result {
        Ok(resp) => {
            conn.execute(
                "INSERT INTO ai_runs (provider, model, action, status, tokens_in, tokens_out)
                 VALUES (?1, ?2, 'chat', 'ok', ?3, ?4)",
                params![prov_name, model_name, resp.tokens_in, resp.tokens_out],
            )?;
            // Persist the user message now that we have a reply.
            conn.execute(
                "INSERT INTO ai_chat_messages (chat_id, role, content) VALUES (?1, 'user', ?2)",
                params![chat_id, content],
            )?;
            conn.execute(
                "INSERT INTO ai_chat_messages (chat_id, role, content, provider, model)
                 VALUES (?1, 'assistant', ?2, ?3, ?4)",
                params![chat_id, resp.text, prov_name, model_name],
            )?;
            let id = conn.last_insert_rowid();
            // Name the chat from its first message.
            let title: String = conn
                .query_row("SELECT title FROM ai_chats WHERE id = ?1", [chat_id], |r| r.get(0))?;
            if title == "New chat" {
                let short: String = content.trim().chars().take(48).collect();
                conn.execute("UPDATE ai_chats SET title = ?2 WHERE id = ?1", params![chat_id, short])?;
            }
            conn.execute(
                "UPDATE ai_chats SET profile_id = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![chat_id, profile_id],
            )?;
            Ok(ChatMessageRow {
                id,
                role: "assistant".into(),
                content: resp.text,
                provider: Some(prov_name),
                model: Some(model_name),
                ts: chrono::Utc::now().to_rfc3339(),
            })
        }
        Err(e) => {
            conn.execute(
                "INSERT INTO ai_runs (provider, model, action, status, tokens_in, tokens_out)
                 VALUES (?1, ?2, 'chat', 'error', 0, 0)",
                params![prov_name, model_name],
            )?;
            Err(e)
        }
    }
}

/// Read a file as text so it can be attached to a chat. Capped so a stray
/// binary can't blow up the conversation.
#[tauri::command]
pub fn read_text_file(path: String) -> AppResult<serde_json::Value> {
    const CAP: usize = 64 * 1024;
    let bytes = std::fs::read(&path)?;
    let truncated = bytes.len() > CAP;
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(CAP)]).to_string();
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(path.clone());
    Ok(serde_json::json!({ "name": name, "content": text, "truncated": truncated }))
}

// ---------- Import suggestions ----------

#[derive(Serialize)]
pub struct SuggestedImport {
    pub path: String,
    pub name: String,
    pub size: i64,
    pub mtime: i64,
    pub source: String,
}

const INTERESTING_EXTS: &[&str] = &[
    "pdf", "zip", "step", "stp", "stl", "3mf", "dxf", "scad", "gbr", "gtl", "gbl", "drl",
    "csv", "xlsx", "kicad_pcb", "kicad_sch", "brd", "sch", "bin", "hex", "uf2", "png",
    "jpg", "jpeg", "heic", "svg", "md", "mov", "mp4", "f3d",
];

/// Recent, maker-relevant files sitting in Downloads / Desktop / watched
/// folders that aren't in the library yet — one click away from the Inbox.
#[tauri::command]
pub fn suggest_imports(state: State<AppState>) -> AppResult<Vec<SuggestedImport>> {
    let conn = state.conn.lock().unwrap();
    let mut dirs: Vec<std::path::PathBuf> = vec![];
    if let Some(home) = dirs_home() {
        dirs.push(home.join("Downloads"));
        dirs.push(home.join("Desktop"));
    }
    if let Some(watched) = db::get_setting(&conn, "watched_dirs")? {
        if let Ok(extra) = serde_json::from_str::<Vec<String>>(&watched) {
            dirs.extend(extra.into_iter().map(std::path::PathBuf::from));
        }
    }

    let known: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT name FROM files")?;
        let names: std::collections::HashSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        names
    };
    let inbox_names: std::collections::HashSet<String> = db::get_setting(&conn, "root")?
        .map(|root| {
            std::fs::read_dir(std::path::Path::new(&root).join(crate::scan::INBOX_DIR))
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 24 * 3600);
    let mut out: Vec<SuggestedImport> = vec![];
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let source = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || known.contains(&name) || inbox_names.contains(&name) {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !INTERESTING_EXTS.contains(&ext.as_str()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if modified < cutoff || meta.len() == 0 {
                continue;
            }
            let mtime = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            out.push(SuggestedImport {
                path: path.to_string_lossy().to_string(),
                name,
                size: meta.len() as i64,
                mtime,
                source: source.clone(),
            });
        }
    }
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    out.truncate(25);
    Ok(out)
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

// ---------- Zero-config detection ----------

#[derive(Serialize)]
pub struct DetectedProvider {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub command: String,
    #[serde(default)]
    pub args: String,
    pub note: String,
    /// True when Hangar can connect this in one click. False for things we
    /// found but can't drive yet (e.g. the ChatGPT app with no `codex` CLI) —
    /// those show `note` as a short how-to instead of a Connect button.
    pub connectable: bool,
}

/// Known CLIs Hangar can drive headlessly, with the flags each needs to run a
/// single prompt non-interactively. `command == "claude"` gets special JSON
/// handling; the rest are treated generically (stdout is the reply).
const CLI_RECIPES: &[(&str, &str, &str, &str)] = &[
    // (binary, friendly name, non-interactive args, note)
    (
        "claude",
        "Claude",
        "",
        "Chat with Claude using your existing Claude login on this Mac. No API key.",
    ),
    (
        "codex",
        "ChatGPT · Codex",
        "exec",
        "Chat with ChatGPT using your existing Codex/ChatGPT login. No API key.",
    ),
    (
        "gemini",
        "Gemini",
        "-p",
        "Chat with Gemini using your existing Gemini CLI login. No API key.",
    ),
    (
        "llm",
        "llm (Simon Willison's CLI)",
        "",
        "Runs your default `llm` model. No key stored in Hangar.",
    ),
    (
        "sgpt",
        "ShellGPT",
        "",
        "Runs your configured ShellGPT model. No key stored in Hangar.",
    ),
];

/// Desktop AI apps that don't expose a local API, mapped to the free CLI that
/// bridges them keylessly (same account/login). If the app is installed but the
/// bridge CLI isn't, we surface a one-line how-to instead of a dead button.
const DESKTOP_BRIDGES: &[(&str, &str, &str, &str)] = &[
    // (app path, friendly name, bridge cli, install hint)
    (
        "/Applications/ChatGPT.app",
        "ChatGPT (desktop app)",
        "codex",
        "ChatGPT app found. To chat with it here with no API key, install OpenAI's Codex CLI: run `npm install -g @openai/codex` then `codex login`. Hangar will pick it up automatically. (Or add an OpenAI key below.)",
    ),
    (
        "/Applications/Claude.app",
        "Claude (desktop app)",
        "claude",
        "Claude app found. To chat with it here with no API key, install Claude Code: run `npm install -g @anthropic-ai/claude-code` then `claude` once to log in. Hangar will pick it up automatically.",
    ),
];

/// AIs Hangar can connect to with zero setup: CLIs already logged in on this
/// Mac, and local model servers already running. No key, ever, for any of
/// these — connecting is one click.
#[tauri::command]
pub async fn ai_detect_providers() -> Vec<DetectedProvider> {
    let mut found = vec![];
    let mut have_cli: std::collections::HashSet<&str> = std::collections::HashSet::new();

    // 1. CLIs already logged in on this Mac — one-click, no key.
    for (bin, name, args, note) in CLI_RECIPES {
        if crate::ai::which_bin(bin).is_some() {
            have_cli.insert(bin);
            found.push(DetectedProvider {
                id: format!("cli-{bin}"),
                name: name.to_string(),
                provider: "cli".into(),
                model: "".into(),
                base_url: "".into(),
                command: bin.to_string(),
                args: args.to_string(),
                note: note.to_string(),
                connectable: true,
            });
        }
    }

    let client = reqwest::Client::new();
    let timeout = std::time::Duration::from_millis(800);

    // 2. Local Ollama — connect with the model that's actually pulled.
    let ollama_base = "http://localhost:11434";
    let models = tokio::time::timeout(
        std::time::Duration::from_millis(1200),
        crate::ai::ollama_models(&client, ollama_base),
    )
    .await
    .unwrap_or_default();
    if !models.is_empty() {
        found.push(DetectedProvider {
            id: "local-ollama".into(),
            name: format!("Ollama · {}", models[0]),
            provider: "ollama".into(),
            model: models[0].clone(),
            base_url: ollama_base.into(),
            command: "".into(),
            args: "".into(),
            note: format!(
                "Local models — nothing leaves this Mac, no key. {} model{} installed.",
                models.len(),
                if models.len() == 1 { "" } else { "s" }
            ),
            connectable: true,
        });
    }

    // 3. Local LM Studio server.
    if client
        .get("http://127.0.0.1:1234/v1/models")
        .timeout(timeout)
        .send()
        .await
        .is_ok()
    {
        found.push(DetectedProvider {
            id: "local-lmstudio".into(),
            name: "LM Studio (running locally)".into(),
            provider: "openai".into(),
            model: "".into(),
            base_url: "http://localhost:1234/v1".into(),
            command: "".into(),
            args: "".into(),
            note: "Local models — nothing leaves this Mac, no key needed.".into(),
            connectable: true,
        });
    }

    // 4. Desktop apps we spotted but can't drive yet — show how to enable them
    //    (skip when the bridge CLI is already installed and listed above).
    for (app_path, name, bridge_cli, hint) in DESKTOP_BRIDGES {
        if std::path::Path::new(app_path).exists() && !have_cli.contains(bridge_cli) {
            found.push(DetectedProvider {
                id: format!("app-{bridge_cli}"),
                name: name.to_string(),
                provider: "cli".into(),
                model: "".into(),
                base_url: "".into(),
                command: bridge_cli.to_string(),
                args: "".into(),
                note: hint.to_string(),
                connectable: false,
            });
        }
    }

    found
}

// ---------- MCP switch ----------

#[tauri::command]
pub fn mcp_get_enabled(state: State<AppState>) -> AppResult<bool> {
    let conn = state.conn.lock().unwrap();
    Ok(db::get_setting(&conn, "mcp_enabled")?.map(|v| v != "0").unwrap_or(true))
}

#[tauri::command]
pub fn mcp_set_enabled(state: State<AppState>, enabled: bool) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    db::set_setting(&conn, "mcp_enabled", if enabled { "1" } else { "0" })
}
