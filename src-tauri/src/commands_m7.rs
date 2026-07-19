//! Project-scoped AI: progress evaluation and a project chat that can act.
//!
//! Two features live here:
//!
//!  - [`ai_evaluate_progress`] asks a model to read the project's milestones,
//!    tasks and recent log and return a percentage with its reasoning. It never
//!    writes — the user decides whether to apply the number.
//!  - [`ai_project_chat`] is the in-project chat. Beyond answering questions it
//!    can *do* things: add tasks, move milestones, rename and move files, set
//!    progress. It reuses the same tool implementations the MCP server exposes,
//!    so there is exactly one code path for "AI changes a project".
//!
//! Providers here are plain text-in/text-out (an Ollama server, a `claude` CLI,
//! an OpenAI-compatible endpoint), and only some of them support native tool
//! calling. So actions ride over a small JSON protocol the model emits, which
//! every provider can produce.

use crate::ai::{self, ChatMessage, Provider};
use crate::commands_m5::{load_provider, project_brief, record_run};
use crate::error::{AppError, AppResult};
use crate::{mcp, ops, AppState};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

/// How many bytes of any one attached file we hand the model. Enough for a
/// BOM, a README or a source file; short enough that ten attachments don't
/// blow the context window.
const ATTACHMENT_BUDGET: usize = 8_000;

/// Tools the project chat may call. A subset of the MCP surface: everything
/// here is scoped to one project and reversible from the UI. Deliberately
/// excluded are `create_project` and `rebuild_index`, which reach outside the
/// project the user is looking at.
const CHAT_TOOLS: &[&str] = &[
    "list_files",
    "move_files",
    "rename_file",
    "create_bin",
    "add_log",
    "set_progress",
    "list_milestones",
    "set_milestone",
    "list_tasks",
    "add_task",
    "complete_task",
    "set_deadline",
    "add_link",
    "add_order",
    "search",
];

// ---------- Progress evaluation ----------

/// Instructions for the progress estimate. A const so the live test exercises
/// the exact wording that ships, not a copy that can drift.
pub const EVAL_SYSTEM: &str = "You estimate how far along a project is, for a solo hardware/software maker.\n\
     Weigh finished milestones heaviest, then closed vs open tasks, then what the \
     recent log actually shows getting done. Late-stage work (testing, packaging, \
     shipping) usually takes longer than it looks, so don't overshoot on a project \
     whose remaining milestones are all end-stage.\n\
     Reply with ONLY JSON: {\"percent\": <0-100 integer>, \"summary\": \"<1-2 sentences>\", \
     \"reasons\": [\"<short evidence>\", ...]}. Give 2-4 reasons, each citing something \
     concrete from the data.";

#[derive(Serialize)]
pub struct ProgressEvaluation {
    /// What the model thinks the project is at, 0-100.
    pub percent: i64,
    /// What the ring currently shows, so the UI can present a delta.
    pub current: i64,
    /// One or two sentences in plain language.
    pub summary: String,
    /// The specific evidence behind the number.
    pub reasons: Vec<String>,
}

#[tauri::command]
pub async fn ai_evaluate_progress(
    state: State<'_, AppState>,
    project_id: i64,
    profile_id: Option<String>,
) -> AppResult<ProgressEvaluation> {
    let (brief, current) = {
        let conn = state.conn.lock().unwrap();
        let current: i64 = conn.query_row(
            "SELECT progress FROM projects WHERE id = ?1",
            [project_id],
            |r| r.get(0),
        )?;
        (project_brief(&conn, project_id)?, current)
    };

    let text = run_ai(
        &state,
        "evaluate_progress",
        profile_id.as_deref(),
        EVAL_SYSTEM.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: format!(
                "{brief}\nThe progress bar currently reads {current}%. Estimate independently — \
                 don't anchor to it."
            ),
        }],
    )
    .await?;

    let v = ai::extract_json(&text)?;
    let percent = v["percent"].as_i64().unwrap_or(current).clamp(0, 100);
    let summary = v["summary"].as_str().unwrap_or("").trim().to_string();
    let reasons: Vec<String> = v["reasons"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_str())
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if summary.is_empty() && reasons.is_empty() {
        return Err(AppError::msg(
            "the model didn't return a usable evaluation — try again, or pick a different AI",
        ));
    }
    Ok(ProgressEvaluation { percent, current, summary, reasons })
}

// ---------- Project chat ----------

#[derive(Debug, Deserialize)]
pub struct Attachment {
    pub file_id: i64,
}

/// One tool call the model made, as shown back to the user.
#[derive(Serialize)]
pub struct ActionResult {
    pub tool: String,
    /// Human-readable one-liner, e.g. `add_task · "panelize gerbers"`.
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize)]
pub struct ProjectChatReply {
    pub text: String,
    pub actions: Vec<ActionResult>,
}

#[tauri::command]
pub async fn ai_project_chat(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    project_id: i64,
    messages: Vec<ChatMessage>,
    profile_id: Option<String>,
    attachments: Option<Vec<Attachment>>,
    allow_actions: Option<bool>,
) -> AppResult<ProjectChatReply> {
    let allow_actions = allow_actions.unwrap_or(true);
    let (brief, attached) = {
        let conn = state.conn.lock().unwrap();
        let brief = project_brief(&conn, project_id)?;
        let attached = read_attachments(&conn, project_id, attachments.unwrap_or_default())?;
        (brief, attached)
    };

    let mut system = format!(
        "You are Hangar's project assistant for a solo hardware/software founder. \
         Answer using the project context below. Be direct and concrete.\n\n{brief}"
    );
    if !attached.is_empty() {
        system.push_str("\n\nFiles the user attached to this message:\n");
        system.push_str(&attached);
    }
    if allow_actions {
        system.push_str(&action_protocol(project_id));
    }

    let first = run_ai(&state, "project_chat", profile_id.as_deref(), system.clone(), messages.clone()).await?;

    if !allow_actions {
        return Ok(ProjectChatReply { text: first, actions: vec![] });
    }

    // Did the model ask to act? A plain prose answer has no parseable JSON,
    // and JSON without an `actions` array (say, a code sample the user asked
    // for) is not an action request either.
    let calls = match ai::extract_json(&first) {
        Ok(v) => parse_actions(&v),
        Err(_) => vec![],
    };
    if calls.is_empty() {
        return Ok(ProjectChatReply { text: first, actions: vec![] });
    }

    let mcp_state = mcp::McpState {
        db_path: state.db_path.clone(),
        token: String::new(), // only used for HTTP auth, which we bypass here
        app: Some(app.clone()),
    };
    let actions = execute(&mcp_state, project_id, calls);

    // Hand the results back so the model can report in plain language rather
    // than the user seeing raw JSON.
    let mut followup = messages;
    followup.push(ChatMessage { role: "assistant".into(), content: first });
    followup.push(ChatMessage {
        role: "user".into(),
        content: format!(
            "Those actions ran. Results:\n{}\n\nTell me what you did in one short paragraph, \
             plain prose, no JSON. Mention anything that failed.",
            actions
                .iter()
                .map(|a| format!(
                    "- {} → {}: {}",
                    a.label,
                    if a.ok { "ok" } else { "FAILED" },
                    a.detail
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    });

    // If the summarising call fails the work is already done, so fall back to
    // a plain receipt rather than losing it.
    let text = run_ai(&state, "project_chat", profile_id.as_deref(), system, followup)
        .await
        .unwrap_or_else(|_| {
            actions
                .iter()
                .map(|a| format!("{} {}", if a.ok { "✓" } else { "✕" }, a.label))
                .collect::<Vec<_>>()
                .join("\n")
        });

    Ok(ProjectChatReply { text, actions })
}

/// The instructions that turn a chat model into an agent over [`CHAT_TOOLS`].
pub fn action_protocol(project_id: i64) -> String {
    let defs = mcp::tool_defs();
    let mut list = String::new();
    for d in defs {
        let Some(name) = d["name"].as_str() else { continue };
        if !CHAT_TOOLS.contains(&name) {
            continue;
        }
        let props = d["inputSchema"]["properties"]
            .as_object()
            .map(|o| o.keys().filter(|k| *k != "project_id").cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        list.push_str(&format!(
            "- {name}({props}) — {}\n",
            d["description"].as_str().unwrap_or("")
        ));
    }
    format!(
        "\n\nYou can also change this project, not just talk about it.\n\
         When the user asks for a change, reply with ONLY this JSON object and nothing else:\n\
         {{\"actions\": [{{\"tool\": \"<name>\", \"args\": {{...}}}}]}}\n\
         project_id is always {project_id} and is filled in for you — leave it out.\n\
         Tools:\n{list}\n\
         Rules: only act when the user actually asks for a change; a question gets a prose \
         answer. Batch related changes into one JSON reply. Never mix prose and JSON in the \
         same reply. If you need to look something up before acting, call the list_/search \
         tools first and act on the next turn."
    )
}

#[derive(Debug)]
pub struct ToolCall {
    pub tool: String,
    pub args: Value,
}

/// At most this many tool calls per turn — a runaway model shouldn't be able
/// to rewrite an entire project in one reply.
const MAX_ACTIONS: usize = 12;

pub fn parse_actions(v: &Value) -> Vec<ToolCall> {
    let Some(arr) = v["actions"].as_array() else {
        return vec![];
    };
    arr.iter()
        .filter_map(|a| {
            let tool = a["tool"].as_str()?.to_string();
            if !CHAT_TOOLS.contains(&tool.as_str()) {
                return None;
            }
            let args = if a["args"].is_object() { a["args"].clone() } else { json!({}) };
            Some(ToolCall { tool, args })
        })
        .take(MAX_ACTIONS)
        .collect()
}

pub fn execute(state: &mcp::McpState, project_id: i64, calls: Vec<ToolCall>) -> Vec<ActionResult> {
    calls
        .into_iter()
        .map(|c| {
            let mut args = c.args;
            // Pin every call to the project the user is looking at, whatever
            // the model said, and pre-confirm the file operations that ask for
            // it (the user opted in by chatting from inside the project).
            if let Some(o) = args.as_object_mut() {
                o.insert("project_id".into(), json!(project_id));
                o.insert("confirm".into(), json!(true));
            }
            let label = describe(&c.tool, &args);
            match mcp::call_tool(state, &c.tool, &args) {
                Ok(v) => ActionResult {
                    tool: c.tool,
                    label,
                    ok: true,
                    detail: summarize(&v),
                },
                Err(e) => ActionResult {
                    tool: c.tool,
                    label,
                    ok: false,
                    detail: e.to_string(),
                },
            }
        })
        .collect()
}

/// A short human label for a tool call, for the receipt in the chat panel.
fn describe(tool: &str, args: &Value) -> String {
    let arg = ["title", "name", "new_name", "query", "body", "url", "vendor", "dest_bin", "state", "date", "value"]
        .iter()
        .find_map(|k| args[*k].as_str().map(String::from))
        .or_else(|| args["value"].as_i64().map(|v| format!("{v}%")))
        .unwrap_or_default();
    if arg.is_empty() {
        tool.to_string()
    } else {
        let short: String = arg.chars().take(48).collect();
        format!("{tool} · {short}")
    }
}

/// Tool results are JSON of wildly varying shape; the model gets the full
/// value but the receipt only needs a gist.
fn summarize(v: &Value) -> String {
    let s = v.to_string();
    if s.len() <= 200 {
        s
    } else {
        format!("{}…", s.chars().take(200).collect::<String>())
    }
}

/// Read attached files off disk, as text, within budget. Files that aren't
/// text (a Gerber zip, a STEP model) are listed by name only — the model can
/// still reason about their presence.
fn read_attachments(conn: &Connection, project_id: i64, list: Vec<Attachment>) -> AppResult<String> {
    if list.is_empty() {
        return Ok(String::new());
    }
    let root = ops::project_path(conn, project_id)?;
    let mut out = String::new();
    for a in list.iter().take(10) {
        let rel: String = match conn.query_row(
            "SELECT rel_path FROM files WHERE id = ?1 AND project_id = ?2",
            rusqlite::params![a.file_id, project_id],
            |r| r.get(0),
        ) {
            Ok(r) => r,
            Err(_) => continue, // file vanished or belongs to another project
        };
        let path = root.join(&rel);
        match std::fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let truncated = text.len() > ATTACHMENT_BUDGET;
                    let body: String = text.chars().take(ATTACHMENT_BUDGET).collect();
                    out.push_str(&format!(
                        "\n--- {rel} ---\n{body}{}\n",
                        if truncated { "\n…(truncated)" } else { "" }
                    ));
                }
                Err(_) => out.push_str(&format!("\n--- {rel} (binary, not shown) ---\n")),
            },
            Err(e) => out.push_str(&format!("\n--- {rel} (unreadable: {e}) ---\n")),
        }
    }
    Ok(out)
}

// ---------- Provider plumbing ----------

/// Like `commands_m5::run_ai`, but a specific profile can be named so the user
/// can switch AI per chat without changing the app-wide default.
async fn run_ai(
    state: &State<'_, AppState>,
    action: &str,
    profile_id: Option<&str>,
    system: String,
    messages: Vec<ChatMessage>,
) -> AppResult<String> {
    let provider = {
        let conn = state.conn.lock().unwrap();
        provider_for(&conn, profile_id)?
    };
    let result = provider.chat(&system, &messages).await;
    let conn = state.conn.lock().unwrap();
    match result {
        Ok(resp) => {
            record_run(&conn, &provider, action, "ok", resp.tokens_in, resp.tokens_out);
            Ok(resp.text)
        }
        Err(e) => {
            record_run(&conn, &provider, action, "error", 0, 0);
            Err(e)
        }
    }
}

fn provider_for(conn: &Connection, profile_id: Option<&str>) -> AppResult<Provider> {
    match profile_id.filter(|id| !id.is_empty()) {
        Some(id) => {
            let p = crate::commands_m6::profile_by_id(conn, id)?;
            crate::commands_m6::provider_from_profile(&p)
        }
        None => load_provider(conn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_action_list() {
        let v = json!({"actions": [
            {"tool": "add_task", "args": {"title": "panelize gerbers"}},
            {"tool": "set_progress", "args": {"value": 40}}
        ]});
        let calls = parse_actions(&v);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "add_task");
    }

    #[test]
    fn rejects_tools_outside_the_allowlist() {
        // create_project reaches outside the project the user is in.
        let v = json!({"actions": [{"tool": "create_project", "args": {"name": "x"}}]});
        assert!(parse_actions(&v).is_empty());
    }

    #[test]
    fn plain_json_answer_is_not_an_action_request() {
        // The user asked for a JSON snippet; it must not be executed.
        let v = json!({"pins": [1, 2, 3]});
        assert!(parse_actions(&v).is_empty());
    }

    #[test]
    fn action_count_is_capped() {
        let actions: Vec<Value> = (0..30)
            .map(|i| json!({"tool": "add_task", "args": {"title": format!("t{i}")}}))
            .collect();
        assert_eq!(parse_actions(&json!({ "actions": actions })).len(), MAX_ACTIONS);
    }

    #[test]
    fn labels_name_the_thing_acted_on() {
        assert_eq!(
            describe("add_task", &json!({"title": "panelize gerbers"})),
            "add_task · panelize gerbers"
        );
        assert_eq!(describe("set_progress", &json!({"value": 40})), "set_progress · 40%");
        assert_eq!(describe("list_tasks", &json!({})), "list_tasks");
    }

    #[test]
    fn protocol_lists_only_allowed_tools() {
        let p = action_protocol(7);
        assert!(p.contains("add_task"));
        assert!(p.contains("project_id is always 7"));
        assert!(!p.contains("create_project"));
        assert!(!p.contains("rebuild_index"));
    }
}
