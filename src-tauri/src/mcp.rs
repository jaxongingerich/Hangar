use crate::error::AppResult;
use crate::{db, ops, scan};
use axum::{
    extract::State as AxState,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;

pub const MCP_PORT: u16 = 41748;

#[derive(Clone)]
pub struct McpState {
    pub db_path: PathBuf,
    pub token: String,
    pub app: tauri::AppHandle,
}

pub fn ensure_token(conn: &Connection) -> AppResult<String> {
    if let Some(t) = db::get_setting(conn, "mcp_token")? {
        return Ok(t);
    }
    let token: String = (0..32)
        .map(|_| {
            let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            chars[rand::random::<u32>() as usize % chars.len()] as char
        })
        .collect();
    db::set_setting(conn, "mcp_token", &token)?;
    Ok(token)
}

pub fn start(app: tauri::AppHandle, db_path: PathBuf, token: String) {
    let state = Arc::new(McpState { db_path, token, app });
    tauri::async_runtime::spawn(async move {
        let router = Router::new()
            .route("/mcp", post(handle))
            .with_state(state);
        let addr = format!("127.0.0.1:{MCP_PORT}");
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                tracing::info!("MCP server on http://{addr}/mcp");
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::warn!("MCP server stopped: {e}");
                }
            }
            Err(e) => tracing::warn!("MCP port {MCP_PORT} unavailable: {e}"),
        }
    });
}

async fn handle(
    AxState(state): AxState<Arc<McpState>>,
    headers: HeaderMap,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    // Bearer auth on every request.
    let authed = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == format!("Bearer {}", state.token))
        .unwrap_or(false);
    if !authed {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"})));
    }

    let id = req["id"].clone();
    let method = req["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "hangar", "version": env!("CARGO_PKG_VERSION") }
        }),
        "notifications/initialized" | "notifications/cancelled" => {
            return (StatusCode::ACCEPTED, Json(json!({})));
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_defs() }),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("");
            let args = req["params"]["arguments"].clone();
            match call_tool(&state, name, &args) {
                Ok(v) => json!({
                    "content": [{ "type": "text", "text": v.to_string() }]
                }),
                Err(e) => json!({
                    "content": [{ "type": "text", "text": format!("error: {e}") }],
                    "isError": true
                }),
            }
        }
        _ => {
            return (
                StatusCode::OK,
                Json(json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": format!("unknown method {method}") }
                })),
            );
        }
    };
    (
        StatusCode::OK,
        Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })),
    )
}

fn tool(name: &str, desc: &str, props: Value, required: Value) -> Value {
    json!({
        "name": name,
        "description": desc,
        "inputSchema": { "type": "object", "properties": props, "required": required }
    })
}

fn tool_defs() -> Vec<Value> {
    let pid = json!({"type": "integer", "description": "project id"});
    vec![
        tool("list_projects", "List all projects with status, progress, and paths", json!({}), json!([])),
        tool("get_project", "Project detail: bins, counts, milestones, open tasks", json!({"project_id": pid}), json!(["project_id"])),
        tool("create_project", "Create a project folder from a template (hardware|software|mixed)",
            json!({"name": {"type": "string"}, "template": {"type": "string"}}), json!(["name"])),
        tool("list_files", "List files in a project, optionally one bin (by name)",
            json!({"project_id": pid, "bin": {"type": "string"}}), json!(["project_id"])),
        tool("move_files", "Move files (by id) into a bin (by name). Requires confirm=true to execute",
            json!({"project_id": pid, "file_ids": {"type": "array", "items": {"type": "integer"}}, "dest_bin": {"type": "string"}, "confirm": {"type": "boolean"}}),
            json!(["project_id", "file_ids", "dest_bin"])),
        tool("rename_file", "Rename a file by id. Requires confirm=true to execute",
            json!({"file_id": {"type": "integer"}, "new_name": {"type": "string"}, "confirm": {"type": "boolean"}}),
            json!(["file_id", "new_name"])),
        tool("search", "Full-text search projects, files, and logs", json!({"query": {"type": "string"}}), json!(["query"])),
        tool("add_log", "Append a note to a project's log", json!({"project_id": pid, "body": {"type": "string"}}), json!(["project_id", "body"])),
        tool("set_progress", "Set manual progress 0-100", json!({"project_id": pid, "value": {"type": "integer"}}), json!(["project_id", "value"])),
        tool("list_milestones", "List a project's milestones", json!({"project_id": pid}), json!(["project_id"])),
        tool("set_milestone", "Set milestone state (todo|doing|done)",
            json!({"milestone_id": {"type": "integer"}, "state": {"type": "string"}}), json!(["milestone_id", "state"])),
        tool("list_tasks", "List open tasks for a project", json!({"project_id": pid}), json!(["project_id"])),
        tool("add_task", "Add a task (optional due YYYY-MM-DD, priority low|med|high)",
            json!({"project_id": pid, "title": {"type": "string"}, "due": {"type": "string"}, "priority": {"type": "string"}}),
            json!(["project_id", "title"])),
        tool("complete_task", "Mark a task done", json!({"task_id": {"type": "integer"}}), json!(["task_id"])),
        tool("set_deadline", "Set the project target date (YYYY-MM-DD)",
            json!({"project_id": pid, "date": {"type": "string"}}), json!(["project_id", "date"])),
        tool("add_link", "Pin a URL to a project",
            json!({"project_id": pid, "title": {"type": "string"}, "url": {"type": "string"}, "kind": {"type": "string"}}),
            json!(["project_id", "url"])),
        tool("add_order", "Track an order (cost in cents)",
            json!({"project_id": pid, "vendor": {"type": "string"}, "cost_cents": {"type": "integer"}, "items": {"type": "string"}, "eta": {"type": "string"}}),
            json!(["project_id", "vendor"])),
        tool("status_report", "Draft and save a status report for a project", json!({"project_id": pid}), json!(["project_id"])),
        tool("space_report", "Disk usage per project", json!({}), json!([])),
        tool("create_bin", "Create a bin (folder) in a project. Requires confirm=true",
            json!({"project_id": pid, "name": {"type": "string"}, "confirm": {"type": "boolean"}}), json!(["project_id", "name"])),
        tool("rebuild_index", "Rescan every project folder from disk", json!({}), json!([])),
    ]
}

fn open(state: &McpState) -> AppResult<Connection> {
    db::open(&state.db_path)
}

fn notify(state: &McpState) {
    let _ = state.app.emit("fs-changed", ());
}

fn call_tool(state: &McpState, name: &str, args: &Value) -> AppResult<Value> {
    let conn = open(state)?;
    let root = PathBuf::from(
        db::get_setting(&conn, "root")?.unwrap_or_default(),
    );
    let pid = args["project_id"].as_i64().unwrap_or(-1);
    let confirmed = args["confirm"].as_bool().unwrap_or(false);

    match name {
        "list_projects" => {
            let mut stmt = conn.prepare(
                "SELECT id, name, status, progress, path FROM projects ORDER BY updated_at DESC",
            )?;
            let rows: Vec<Value> = stmt
                .query_map([], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                        "status": r.get::<_, String>(2)?, "progress": r.get::<_, i64>(3)?,
                        "path": r.get::<_, String>(4)?
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!(rows))
        }
        "get_project" => {
            let base = conn.query_row(
                "SELECT name, status, progress, target_date, path FROM projects WHERE id = ?1",
                [pid],
                |r| {
                    Ok(json!({
                        "id": pid, "name": r.get::<_, String>(0)?, "status": r.get::<_, String>(1)?,
                        "progress": r.get::<_, i64>(2)?, "target_date": r.get::<_, Option<String>>(3)?,
                        "path": r.get::<_, String>(4)?
                    }))
                },
            )?;
            let mut out = base;
            let mut stmt = conn.prepare(
                "SELECT b.name, COUNT(f.id) FROM bins b LEFT JOIN files f ON f.bin_id = b.id
                 WHERE b.project_id = ?1 GROUP BY b.id",
            )?;
            let bins: Vec<Value> = stmt
                .query_map([pid], |r| {
                    Ok(json!({"name": r.get::<_, String>(0)?, "files": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            out["bins"] = json!(bins);
            let mut stmt = conn.prepare(
                "SELECT id, title, state FROM milestones WHERE project_id = ?1 ORDER BY sort_order",
            )?;
            let ms: Vec<Value> = stmt
                .query_map([pid], |r| {
                    Ok(json!({"id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)?, "state": r.get::<_, String>(2)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            out["milestones"] = json!(ms);
            Ok(out)
        }
        "create_project" => {
            let name = args["name"].as_str().unwrap_or("").trim().to_string();
            if name.is_empty() {
                return Err(crate::error::AppError::msg("name required"));
            }
            let template = args["template"].as_str().unwrap_or("hardware");
            let dir = root.join(&name);
            if dir.exists() {
                return Err(crate::error::AppError::msg("already exists"));
            }
            std::fs::create_dir_all(&dir)?;
            let bins: &[&str] = match template {
                "software" => &["Design", "Assets", "Research", "Exports", "Docs"],
                _ => &["Gerbers", "JLCPCB", "Firmware", "CAD", "Datasheets", "BOM", "Photos", "Docs"],
            };
            for b in bins {
                std::fs::create_dir_all(dir.join(b))?;
            }
            crate::sidecar::Sidecar::new(&name).save(&dir)?;
            let mut conn2 = open(state)?;
            scan::scan(&mut conn2, &root)?;
            notify(state);
            let new_id: i64 = conn2.query_row(
                "SELECT id FROM projects WHERE name = ?1",
                [&name],
                |r| r.get(0),
            )?;
            Ok(json!({"created": name, "project_id": new_id, "template": template}))
        }
        "list_files" => {
            let bin = args["bin"].as_str();
            let rows: Vec<Value> = match bin {
                Some(bname) => {
                    let mut stmt = conn.prepare(
                        "SELECT f.id, f.name, f.rel_path, f.size FROM files f
                         JOIN bins b ON b.id = f.bin_id
                         WHERE f.project_id = ?1 AND b.name = ?2 ORDER BY f.name LIMIT 500",
                    )?;
                    let v = stmt
                        .query_map(params![pid, bname], |r| {
                            Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                                "rel_path": r.get::<_, String>(2)?, "size": r.get::<_, i64>(3)?}))
                        })?
                        .filter_map(|r| r.ok())
                        .collect();
                    v
                }
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT id, name, rel_path, size FROM files
                         WHERE project_id = ?1 ORDER BY rel_path LIMIT 500",
                    )?;
                    let v = stmt
                        .query_map([pid], |r| {
                            Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?,
                                "rel_path": r.get::<_, String>(2)?, "size": r.get::<_, i64>(3)?}))
                        })?
                        .filter_map(|r| r.ok())
                        .collect();
                    v
                }
            };
            Ok(json!(rows))
        }
        "move_files" => {
            let file_ids: Vec<i64> = args["file_ids"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
                .unwrap_or_default();
            let dest_bin = args["dest_bin"].as_str().unwrap_or("");
            let bin_id: i64 = conn.query_row(
                "SELECT id FROM bins WHERE project_id = ?1 AND name = ?2",
                params![pid, dest_bin],
                |r| r.get(0),
            )?;
            if !confirmed {
                return Ok(json!({
                    "plan": format!("move {} file(s) to bin \"{dest_bin}\"", file_ids.len()),
                    "note": "call again with confirm=true to execute"
                }));
            }
            let moved = ops::move_files(&conn, &root, &file_ids, Some(bin_id))?;
            notify(state);
            Ok(json!({"moved": moved}))
        }
        "rename_file" => {
            let file_id = args["file_id"].as_i64().unwrap_or(-1);
            let new_name = args["new_name"].as_str().unwrap_or("");
            if !confirmed {
                return Ok(json!({
                    "plan": format!("rename file {file_id} to \"{new_name}\""),
                    "note": "call again with confirm=true to execute"
                }));
            }
            ops::rename_file(&conn, &root, file_id, new_name)?;
            notify(state);
            Ok(json!({"renamed": true}))
        }
        "search" => {
            let q = args["query"].as_str().unwrap_or("").replace('"', "");
            let fts = format!("\"{q}\"*");
            let mut stmt = conn.prepare(
                "SELECT f.id, f.name, p.name FROM files_fts ft
                 JOIN files f ON f.id = ft.rowid JOIN projects p ON p.id = f.project_id
                 WHERE files_fts MATCH ?1 LIMIT 25",
            )?;
            let rows: Vec<Value> = stmt
                .query_map([&fts], |r| {
                    Ok(json!({"file_id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "project": r.get::<_, String>(2)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!(rows))
        }
        "add_log" => {
            let body = args["body"].as_str().unwrap_or("").trim().to_string();
            conn.execute(
                "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'note', ?2)",
                params![pid, body],
            )?;
            let dir = ops::project_path(&conn, pid)?;
            let _ = crate::sidecar::append_log_md(
                &dir,
                &format!("- {} · {body}", chrono::Local::now().format("%Y-%m-%d %H:%M")),
            );
            notify(state);
            Ok(json!({"logged": true}))
        }
        "set_progress" => {
            let value = args["value"].as_i64().unwrap_or(0);
            ops::set_progress(&conn, pid, value, "ai")?;
            notify(state);
            Ok(json!({"progress": value.clamp(0, 100)}))
        }
        "list_milestones" => {
            let mut stmt = conn.prepare(
                "SELECT id, title, state, weight FROM milestones WHERE project_id = ?1 ORDER BY sort_order",
            )?;
            let rows: Vec<Value> = stmt
                .query_map([pid], |r| {
                    Ok(json!({"id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)?,
                        "state": r.get::<_, String>(2)?, "weight": r.get::<_, i64>(3)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!(rows))
        }
        "set_milestone" => {
            let mid = args["milestone_id"].as_i64().unwrap_or(-1);
            let new_state = args["state"].as_str().unwrap_or("todo");
            let project_id: i64 = conn.query_row(
                "SELECT project_id FROM milestones WHERE id = ?1",
                [mid],
                |r| r.get(0),
            )?;
            conn.execute(
                "UPDATE milestones SET state = ?1 WHERE id = ?2",
                params![new_state, mid],
            )?;
            ops::recompute_progress(&conn, project_id)?;
            notify(state);
            Ok(json!({"milestone": mid, "state": new_state}))
        }
        "list_tasks" => {
            let mut stmt = conn.prepare(
                "SELECT id, title, due, priority, blocked FROM tasks
                 WHERE project_id = ?1 AND done = 0 ORDER BY due IS NULL, due",
            )?;
            let rows: Vec<Value> = stmt
                .query_map([pid], |r| {
                    Ok(json!({"id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)?,
                        "due": r.get::<_, Option<String>>(2)?, "priority": r.get::<_, String>(3)?,
                        "blocked": r.get::<_, i64>(4)? != 0}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!(rows))
        }
        "add_task" => {
            let title = args["title"].as_str().unwrap_or("").trim().to_string();
            conn.execute(
                "INSERT INTO tasks (project_id, title, due, priority) VALUES (?1, ?2, ?3, ?4)",
                params![
                    pid,
                    title,
                    args["due"].as_str(),
                    args["priority"].as_str().unwrap_or("med")
                ],
            )?;
            notify(state);
            Ok(json!({"task_id": conn.last_insert_rowid()}))
        }
        "complete_task" => {
            let tid = args["task_id"].as_i64().unwrap_or(-1);
            let project_id: i64 =
                conn.query_row("SELECT project_id FROM tasks WHERE id = ?1", [tid], |r| r.get(0))?;
            conn.execute(
                "UPDATE tasks SET done = 1, done_at = datetime('now') WHERE id = ?1",
                [tid],
            )?;
            ops::recompute_progress(&conn, project_id)?;
            notify(state);
            Ok(json!({"completed": tid}))
        }
        "set_deadline" => {
            let date = args["date"].as_str().unwrap_or("");
            conn.execute(
                "UPDATE projects SET target_date = ?1 WHERE id = ?2",
                params![date, pid],
            )?;
            notify(state);
            Ok(json!({"target_date": date}))
        }
        "add_link" => {
            conn.execute(
                "INSERT INTO links (project_id, title, url, kind) VALUES (?1, ?2, ?3, ?4)",
                params![
                    pid,
                    args["title"].as_str().unwrap_or("link"),
                    args["url"].as_str().unwrap_or(""),
                    args["kind"].as_str().unwrap_or("other")
                ],
            )?;
            notify(state);
            Ok(json!({"link_id": conn.last_insert_rowid()}))
        }
        "add_order" => {
            conn.execute(
                "INSERT INTO orders (project_id, vendor, items, cost_cents, eta)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    pid,
                    args["vendor"].as_str().unwrap_or(""),
                    args["items"].as_str(),
                    args["cost_cents"].as_i64().unwrap_or(0),
                    args["eta"].as_str()
                ],
            )?;
            notify(state);
            Ok(json!({"order_id": conn.last_insert_rowid()}))
        }
        "status_report" => {
            // Reuse the template report (AI report needs async; MCP callers
            // usually are the AI).
            let mut stmt = conn.prepare(
                "SELECT body_md FROM logs WHERE project_id = ?1 AND ts > datetime('now','-7 days')
                 ORDER BY ts DESC LIMIT 12",
            )?;
            let recent: Vec<String> = stmt
                .query_map([pid], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            let (name, progress): (String, i64) = conn.query_row(
                "SELECT name, progress FROM projects WHERE id = ?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let report = format!(
                "Status — {}\n\n{name} · {progress}%\n\nThis week:\n{}",
                chrono::Local::now().format("%Y-%m-%d"),
                recent.iter().map(|r| format!("• {r}\n")).collect::<String>()
            );
            conn.execute(
                "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'status_report', ?2)",
                params![pid, report],
            )?;
            notify(state);
            Ok(json!({"report": report}))
        }
        "space_report" => {
            let mut stmt = conn.prepare(
                "SELECT p.name, COALESCE(SUM(f.size),0), COUNT(f.id)
                 FROM projects p LEFT JOIN files f ON f.project_id = p.id
                 GROUP BY p.id ORDER BY 2 DESC",
            )?;
            let rows: Vec<Value> = stmt
                .query_map([], |r| {
                    Ok(json!({"project": r.get::<_, String>(0)?, "bytes": r.get::<_, i64>(1)?, "files": r.get::<_, i64>(2)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!(rows))
        }
        "create_bin" => {
            let bname = args["name"].as_str().unwrap_or("").trim().to_string();
            if !confirmed {
                return Ok(json!({
                    "plan": format!("create bin \"{bname}\" in project {pid}"),
                    "note": "call again with confirm=true to execute"
                }));
            }
            let dir = ops::project_path(&conn, pid)?.join(&bname);
            std::fs::create_dir_all(&dir)?;
            conn.execute(
                "INSERT OR IGNORE INTO bins (project_id, name, rel_path) VALUES (?1, ?2, ?2)",
                params![pid, bname],
            )?;
            notify(state);
            Ok(json!({"created_bin": bname}))
        }
        "rebuild_index" => {
            let mut conn2 = open(state)?;
            let stats = scan::scan(&mut conn2, &root)?;
            notify(state);
            Ok(json!({"projects": stats.projects, "files": stats.files, "ms": stats.elapsed_ms}))
        }
        _ => Err(crate::error::AppError::msg(format!("unknown tool {name}"))),
    }
}
