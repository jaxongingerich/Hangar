use crate::ai::{self, ChatMessage, Provider};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

// ---------- Config ----------

#[derive(Serialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub has_key: bool,
}

fn keyring_entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(ai::KEYRING_SERVICE, ai::KEYRING_USER)
        .map_err(|e| AppError::msg(format!("keychain unavailable: {e}")))
}

fn read_key() -> Option<String> {
    keyring_entry().ok()?.get_password().ok()
}

#[tauri::command]
pub fn ai_get_config(state: State<AppState>) -> AppResult<AiConfig> {
    let conn = state.conn.lock().unwrap();
    Ok(AiConfig {
        provider: db::get_setting(&conn, "ai_provider")?.unwrap_or_else(|| "none".into()),
        model: db::get_setting(&conn, "ai_model")?.unwrap_or_default(),
        base_url: db::get_setting(&conn, "ai_base_url")?.unwrap_or_default(),
        has_key: read_key().is_some(),
    })
}

#[tauri::command]
pub fn ai_set_config(
    state: State<AppState>,
    provider: String,
    model: String,
    base_url: String,
) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    db::set_setting(&conn, "ai_provider", &provider)?;
    db::set_setting(&conn, "ai_model", &model)?;
    db::set_setting(&conn, "ai_base_url", &base_url)?;
    Ok(())
}

#[tauri::command]
pub fn ai_set_key(key: String) -> AppResult<()> {
    let entry = keyring_entry()?;
    if key.trim().is_empty() {
        let _ = entry.delete_credential();
    } else {
        entry
            .set_password(key.trim())
            .map_err(|e| AppError::msg(format!("keychain write failed: {e}")))?;
    }
    Ok(())
}

fn load_provider(conn: &Connection) -> AppResult<Provider> {
    let provider = db::get_setting(conn, "ai_provider")?.unwrap_or_else(|| "none".into());
    let model = db::get_setting(conn, "ai_model")?.unwrap_or_default();
    let base = db::get_setting(conn, "ai_base_url")?.unwrap_or_default();
    match provider.as_str() {
        "anthropic" => {
            let key = read_key().ok_or_else(|| {
                AppError::msg("no Anthropic API key saved — add one in Settings → AI")
            })?;
            Ok(Provider::Anthropic {
                key,
                model: if model.is_empty() { "claude-sonnet-4-6".into() } else { model },
            })
        }
        "ollama" => Ok(Provider::Ollama {
            base: if base.is_empty() { "http://localhost:11434".into() } else { base },
            model: if model.is_empty() { "llama3.2".into() } else { model },
        }),
        "openai" => {
            if base.is_empty() {
                return Err(AppError::msg("set a base URL for the OpenAI-compatible endpoint"));
            }
            Ok(Provider::OpenAiCompat { base, key: read_key(), model })
        }
        _ => Err(AppError::msg("no AI provider configured — pick one in Settings → AI")),
    }
}

fn record_run(
    conn: &Connection,
    provider: &Provider,
    action: &str,
    status: &str,
    tokens_in: i64,
    tokens_out: i64,
) {
    let _ = conn.execute(
        "INSERT INTO ai_runs (provider, model, action, status, tokens_in, tokens_out)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            provider.provider_name(),
            provider.model_name(),
            action,
            status,
            tokens_in,
            tokens_out
        ],
    );
}

/// Run one chat turn against the configured provider, logging to ai_runs.
async fn run_ai(
    state: &State<'_, AppState>,
    action: &str,
    system: String,
    messages: Vec<ChatMessage>,
) -> AppResult<String> {
    let provider = {
        let conn = state.conn.lock().unwrap();
        load_provider(&conn)?
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

#[tauri::command]
pub async fn ai_test(state: State<'_, AppState>) -> AppResult<String> {
    let text = run_ai(
        &state,
        "test",
        "Reply with exactly: ok".into(),
        vec![ChatMessage { role: "user".into(), content: "ping".into() }],
    )
    .await?;
    Ok(text.chars().take(80).collect())
}

#[tauri::command]
pub async fn ai_ollama_models(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let base = {
        let conn = state.conn.lock().unwrap();
        db::get_setting(&conn, "ai_base_url")?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://localhost:11434".into())
    };
    let v: serde_json::Value = reqwest::get(format!("{}/api/tags", base.trim_end_matches('/')))
        .await
        .map_err(|e| AppError::msg(format!("Ollama unreachable: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::msg(format!("bad Ollama response: {e}")))?;
    Ok(v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

#[derive(Serialize)]
pub struct AiUsage {
    pub month_runs: i64,
    pub month_tokens_in: i64,
    pub month_tokens_out: i64,
}

#[tauri::command]
pub fn ai_usage(state: State<AppState>) -> AppResult<AiUsage> {
    let conn = state.conn.lock().unwrap();
    let (month_runs, month_tokens_in, month_tokens_out) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(tokens_in),0), COALESCE(SUM(tokens_out),0)
         FROM ai_runs WHERE ts >= date('now', 'start of month')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(AiUsage { month_runs, month_tokens_in, month_tokens_out })
}

// ---------- Context builders ----------

fn project_brief(conn: &Connection, project_id: i64) -> AppResult<String> {
    let (name, status, progress): (String, String, i64) = conn.query_row(
        "SELECT name, status, progress FROM projects WHERE id = ?1",
        [project_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut brief = format!("Project: {name} · status {status} · {progress}%\n");

    brief.push_str("\nBins:\n");
    let mut stmt = conn.prepare(
        "SELECT b.name, COUNT(f.id) FROM bins b LEFT JOIN files f ON f.bin_id = b.id
         WHERE b.project_id = ?1 GROUP BY b.id ORDER BY b.sort_order",
    )?;
    for row in stmt.query_map([project_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })? {
        let (bin, count) = row?;
        brief.push_str(&format!("- {bin} ({count} files)\n"));
    }

    brief.push_str("\nMilestones:\n");
    let mut stmt = conn.prepare(
        "SELECT title, state FROM milestones WHERE project_id = ?1 ORDER BY sort_order",
    )?;
    for row in stmt.query_map([project_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (title, mstate) = row?;
        brief.push_str(&format!("- [{mstate}] {title}\n"));
    }

    brief.push_str("\nOpen tasks:\n");
    let mut stmt = conn.prepare(
        "SELECT title, due, blocked, blocked_reason FROM tasks
         WHERE project_id = ?1 AND done = 0 ORDER BY due IS NULL, due LIMIT 20",
    )?;
    for row in stmt.query_map([project_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })? {
        let (title, due, blocked, reason) = row?;
        brief.push_str(&format!(
            "- {title}{}{}\n",
            due.map(|d| format!(" (due {d})")).unwrap_or_default(),
            if blocked != 0 {
                format!(" [BLOCKED: {}]", reason.unwrap_or_default())
            } else {
                String::new()
            }
        ));
    }

    brief.push_str("\nRecent log:\n");
    let mut stmt = conn.prepare(
        "SELECT ts, body_md FROM logs WHERE project_id = ?1 ORDER BY ts DESC LIMIT 20",
    )?;
    for row in stmt.query_map([project_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (ts, body) = row?;
        brief.push_str(&format!("- {} · {body}\n", &ts[..16.min(ts.len())]));
    }
    Ok(brief)
}

// ---------- Assistant actions ----------

#[derive(Serialize)]
pub struct InboxPlanItem {
    pub path: String,
    pub name: String,
    pub project_id: i64,
    pub project_name: String,
    pub bin_id: Option<i64>,
    pub bin_name: String,
    pub reason: String,
}

#[tauri::command]
pub async fn ai_organize_inbox(state: State<'_, AppState>) -> AppResult<Vec<InboxPlanItem>> {
    // Gather context synchronously.
    #[allow(clippy::type_complexity)]
    let (inbox_files, catalog): (Vec<(String, String)>, Vec<(i64, String, Vec<(i64, String)>)>) = {
        let conn = state.conn.lock().unwrap();
        let root = db::get_setting(&conn, "root")?
            .ok_or_else(|| AppError::msg("no root configured"))?;
        let inbox = std::path::Path::new(&root).join(crate::scan::INBOX_DIR);
        let files: Vec<(String, String)> = std::fs::read_dir(&inbox)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .map(|e| {
                        (
                            e.path().to_string_lossy().to_string(),
                            e.file_name().to_string_lossy().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if files.is_empty() {
            return Ok(vec![]);
        }
        let mut catalog = vec![];
        let mut stmt =
            conn.prepare("SELECT id, name FROM projects WHERE status != 'archived'")?;
        let projects: Vec<(i64, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let mut bin_stmt =
            conn.prepare("SELECT id, name FROM bins WHERE project_id = ?1")?;
        for (pid, pname) in projects {
            let bins: Vec<(i64, String)> = bin_stmt
                .query_map([pid], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            catalog.push((pid, pname, bins));
        }
        (files, catalog)
    };

    let catalog_text: String = catalog
        .iter()
        .map(|(pid, name, bins)| {
            format!(
                "- project {pid}: \"{name}\" · bins: {}",
                bins.iter()
                    .map(|(bid, bname)| format!("{bid}:\"{bname}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let files_text: String = inbox_files
        .iter()
        .map(|(_, name)| format!("- {name}"))
        .collect::<Vec<_>>()
        .join("\n");

    let system = "You file inbox items into project bins for a hardware/software maker. \
        Reply with ONLY a JSON array, one object per file: \
        {\"file\": \"<file name>\", \"project_id\": <id>, \"bin_id\": <id or null>, \"reason\": \"<short>\"} \
        Pick the most plausible project by name similarity and file type; gerbers→Gerbers, \
        3D models→CAD, datasheet PDFs→Datasheets, BOMs→BOM, images→Photos, code/binaries→Firmware."
        .to_string();
    let user = format!("Files in inbox:\n{files_text}\n\nProjects and bins:\n{catalog_text}");
    let text = run_ai(
        &state,
        "organize_inbox",
        system,
        vec![ChatMessage { role: "user".into(), content: user }],
    )
    .await?;
    let plan = ai::extract_json(&text)?;

    // Validate every suggestion against the real catalog — the model only
    // proposes; Hangar decides what is even allowed on the review sheet.
    let mut items = vec![];
    if let Some(arr) = plan.as_array() {
        for entry in arr {
            let Some(fname) = entry["file"].as_str() else { continue };
            let Some((path, name)) = inbox_files.iter().find(|(_, n)| n == fname) else {
                continue;
            };
            let pid = entry["project_id"].as_i64().unwrap_or(-1);
            let Some((_, pname, bins)) = catalog.iter().find(|(id, _, _)| *id == pid) else {
                continue;
            };
            let bid = entry["bin_id"].as_i64();
            let (bin_id, bin_name) = match bid.and_then(|b| bins.iter().find(|(id, _)| *id == b)) {
                Some((id, name)) => (Some(*id), name.clone()),
                None => (None, "project root".to_string()),
            };
            items.push(InboxPlanItem {
                path: path.clone(),
                name: name.clone(),
                project_id: pid,
                project_name: pname.clone(),
                bin_id,
                bin_name,
                reason: entry["reason"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(items)
}

#[tauri::command]
pub async fn ai_summarize(state: State<'_, AppState>, project_id: i64) -> AppResult<String> {
    let brief = {
        let conn = state.conn.lock().unwrap();
        project_brief(&conn, project_id)?
    };
    run_ai(
        &state,
        "summarize",
        "You write tight status summaries for a solo hardware founder. \
         3-6 bullet lines: where the project stands, what moved recently, what's next, any blockers. \
         No fluff, no headers."
            .into(),
        vec![ChatMessage { role: "user".into(), content: format!("Catch me up:\n\n{brief}") }],
    )
    .await
}

#[tauri::command]
pub async fn ai_auto_milestones(
    state: State<'_, AppState>,
    project_id: i64,
    description: String,
) -> AppResult<Vec<String>> {
    let brief = {
        let conn = state.conn.lock().unwrap();
        project_brief(&conn, project_id)?
    };
    let text = run_ai(
        &state,
        "auto_milestones",
        "You plan build milestones for maker projects (PCBs, firmware, apps). \
         Reply with ONLY a JSON array of 5-10 short milestone title strings, ordered."
            .into(),
        vec![ChatMessage {
            role: "user".into(),
            content: format!("Project description: {description}\n\nCurrent state:\n{brief}"),
        }],
    )
    .await?;
    let v = ai::extract_json(&text)?;
    Ok(v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

#[tauri::command]
pub async fn ai_status_report(state: State<'_, AppState>, project_id: i64) -> AppResult<String> {
    let brief = {
        let conn = state.conn.lock().unwrap();
        project_brief(&conn, project_id)?
    };
    let date = chrono::Local::now().format("%Y-%m-%d");
    let report = run_ai(
        &state,
        "status_report",
        format!(
            "You draft succinct status reports. Start with the line \"Status — {date}\" then \
             short sections: This week / Next up / Blocked (omit empty sections). Plain text."
        ),
        vec![ChatMessage { role: "user".into(), content: brief }],
    )
    .await?;
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'status_report', ?2)",
        params![project_id, report],
    )?;
    Ok(report)
}

#[tauri::command]
pub async fn ai_weekly_digest(state: State<'_, AppState>) -> AppResult<String> {
    let (context, busiest): (String, Option<i64>) = {
        let conn = state.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name FROM projects WHERE status = 'active'",
        )?;
        let projects: Vec<(i64, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let mut context = String::new();
        let mut busiest = None;
        let mut max_events = -1i64;
        for (pid, _) in &projects {
            let events: i64 = conn.query_row(
                "SELECT COUNT(*) FROM logs WHERE project_id = ?1 AND ts > datetime('now','-7 days')",
                [pid],
                |r| r.get(0),
            )?;
            if events > max_events {
                max_events = events;
                busiest = Some(*pid);
            }
            context.push_str(&project_brief(&conn, *pid)?);
            context.push_str("\n---\n");
        }
        (context, busiest)
    };
    if context.is_empty() {
        return Err(AppError::msg("no active projects to digest"));
    }
    let digest = run_ai(
        &state,
        "weekly_digest",
        "You write a weekly digest across a founder's active projects. \
         Per project: one line on movement (or 'stale'). Then: top 3 next actions overall. Plain text, tight."
            .into(),
        vec![ChatMessage { role: "user".into(), content: context }],
    )
    .await?;
    if let Some(pid) = busiest {
        let conn = state.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'digest', ?2)",
            params![pid, digest],
        )?;
    }
    Ok(digest)
}

#[derive(Serialize)]
pub struct RenamePlanItem {
    pub file_id: i64,
    pub old_name: String,
    pub new_name: String,
}

#[tauri::command]
pub async fn ai_smart_rename(state: State<'_, AppState>, bin_id: i64) -> AppResult<Vec<RenamePlanItem>> {
    let (project_name, bin_name, files): (String, String, Vec<(i64, String)>) = {
        let conn = state.conn.lock().unwrap();
        let (pid, bname): (i64, String) = conn.query_row(
            "SELECT project_id, name FROM bins WHERE id = ?1",
            [bin_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let pname: String =
            conn.query_row("SELECT name FROM projects WHERE id = ?1", [pid], |r| r.get(0))?;
        let mut stmt =
            conn.prepare("SELECT id, name FROM files WHERE bin_id = ?1 ORDER BY name LIMIT 100")?;
        let files = stmt
            .query_map([bin_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        (pname, bname, files)
    };
    if files.is_empty() {
        return Err(AppError::msg("bin is empty"));
    }
    let listing: String = files
        .iter()
        .map(|(id, name)| format!("{id}: {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    let text = run_ai(
        &state,
        "smart_rename",
        "You design consistent file naming schemes (e.g. Project_Kind_RevX_Detail.ext). \
         Infer one scheme for the folder and rename every file to fit, KEEPING each extension. \
         Reply with ONLY a JSON array: {\"id\": <id>, \"new\": \"<new file name>\"}. \
         Skip files that already fit by omitting them."
            .into(),
        vec![ChatMessage {
            role: "user".into(),
            content: format!("Project: {project_name}\nFolder: {bin_name}\nFiles:\n{listing}"),
        }],
    )
    .await?;
    let v = ai::extract_json(&text)?;
    let mut plan = vec![];
    if let Some(arr) = v.as_array() {
        for entry in arr {
            let (Some(id), Some(new)) = (entry["id"].as_i64(), entry["new"].as_str()) else {
                continue;
            };
            let Some((_, old)) = files.iter().find(|(fid, _)| *fid == id) else { continue };
            // Guardrails: keep the extension, no path separators.
            if new.contains('/') || new.trim().is_empty() || new == old {
                continue;
            }
            let old_ext = old.rsplit('.').next().unwrap_or("");
            let new_ext = new.rsplit('.').next().unwrap_or("");
            if !old.contains('.') || old_ext.eq_ignore_ascii_case(new_ext) {
                plan.push(RenamePlanItem {
                    file_id: id,
                    old_name: old.clone(),
                    new_name: new.trim().to_string(),
                });
            }
        }
    }
    Ok(plan)
}

#[tauri::command]
pub async fn ai_project_chat(
    state: State<'_, AppState>,
    project_id: i64,
    messages: Vec<ChatMessage>,
) -> AppResult<String> {
    let brief = {
        let conn = state.conn.lock().unwrap();
        project_brief(&conn, project_id)?
    };
    run_ai(
        &state,
        "project_chat",
        format!(
            "You are Hangar's project assistant for a solo hardware/software founder. \
             Answer using the project context below. Be direct and concrete.\n\n{brief}"
        ),
        messages,
    )
    .await
}
