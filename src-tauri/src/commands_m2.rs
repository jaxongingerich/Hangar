use crate::error::{AppError, AppResult};
use crate::ops;
use crate::AppState;
use chrono::{Datelike, Duration, Local, NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------- Milestones ----------

#[derive(Serialize)]
pub struct MilestoneRow {
    pub id: i64,
    pub title: String,
    pub state: String,
    pub weight: i64,
    pub sort_order: i64,
    pub task_count: i64,
    pub done_task_count: i64,
}

#[tauri::command]
pub fn list_milestones(state: State<AppState>, project_id: i64) -> AppResult<Vec<MilestoneRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT m.id, m.title, m.state, m.weight, m.sort_order,
                (SELECT COUNT(*) FROM tasks t WHERE t.milestone_id = m.id),
                (SELECT COUNT(*) FROM tasks t WHERE t.milestone_id = m.id AND t.done = 1)
         FROM milestones m WHERE m.project_id = ?1
         ORDER BY m.sort_order, m.id",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(MilestoneRow {
                id: r.get(0)?,
                title: r.get(1)?,
                state: r.get(2)?,
                weight: r.get(3)?,
                sort_order: r.get(4)?,
                task_count: r.get(5)?,
                done_task_count: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_milestone(
    state: State<AppState>,
    project_id: i64,
    title: String,
    weight: Option<i64>,
) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO milestones (project_id, title, weight, sort_order)
         VALUES (?1, ?2, ?3,
           (SELECT COALESCE(MAX(sort_order),0)+1 FROM milestones WHERE project_id = ?1))",
        params![project_id, title.trim(), weight.unwrap_or(1)],
    )?;
    let id = conn.last_insert_rowid();
    ops::recompute_progress(&conn, project_id)?;
    Ok(id)
}

#[tauri::command]
pub fn set_milestone_state(
    state: State<AppState>,
    milestone_id: i64,
    new_state: String,
) -> AppResult<()> {
    if !["todo", "doing", "done"].contains(&new_state.as_str()) {
        return Err(AppError::msg("invalid milestone state"));
    }
    let conn = state.conn.lock().unwrap();
    let (project_id, title): (i64, String) = conn.query_row(
        "SELECT project_id, title FROM milestones WHERE id = ?1",
        [milestone_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute(
        "UPDATE milestones SET state = ?1 WHERE id = ?2",
        params![new_state, milestone_id],
    )?;
    if new_state == "done" {
        ops::auto_log(&conn, project_id, &format!("milestone done: {title}"))?;
    }
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

#[tauri::command]
pub fn update_milestone(
    state: State<AppState>,
    milestone_id: i64,
    title: Option<String>,
    weight: Option<i64>,
) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let project_id: i64 = conn.query_row(
        "SELECT project_id FROM milestones WHERE id = ?1",
        [milestone_id],
        |r| r.get(0),
    )?;
    if let Some(t) = title {
        conn.execute(
            "UPDATE milestones SET title = ?1 WHERE id = ?2",
            params![t.trim(), milestone_id],
        )?;
    }
    if let Some(w) = weight {
        conn.execute(
            "UPDATE milestones SET weight = ?1 WHERE id = ?2",
            params![w.max(1), milestone_id],
        )?;
    }
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

#[tauri::command]
pub fn delete_milestone(state: State<AppState>, milestone_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let project_id: i64 = conn.query_row(
        "SELECT project_id FROM milestones WHERE id = ?1",
        [milestone_id],
        |r| r.get(0),
    )?;
    conn.execute("DELETE FROM milestones WHERE id = ?1", [milestone_id])?;
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

pub const HARDWARE_MILESTONES: &[&str] = &[
    "Idea", "Schematic", "PCB Layout", "Gerbers Out", "Boards In",
    "Firmware Bring-up", "Enclosure", "App", "Beta", "Ship",
];
pub const SOFTWARE_MILESTONES: &[&str] = &[
    "Idea", "Design", "Prototype", "Core Features", "Polish", "Beta", "Ship",
];

#[tauri::command]
pub fn apply_milestone_template(
    state: State<AppState>,
    project_id: i64,
    template: String,
) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let titles: &[&str] = if template == "software" {
        SOFTWARE_MILESTONES
    } else {
        HARDWARE_MILESTONES
    };
    for (i, t) in titles.iter().enumerate() {
        conn.execute(
            "INSERT INTO milestones (project_id, title, weight, sort_order) VALUES (?1, ?2, 1, ?3)",
            params![project_id, t, i as i64],
        )?;
    }
    // Milestones now drive the ring.
    conn.execute(
        "UPDATE projects SET progress_mode = 'milestones' WHERE id = ?1",
        [project_id],
    )?;
    ops::recompute_progress(&conn, project_id)?;
    ops::auto_log(&conn, project_id, &format!("applied {template} milestone template"))?;
    Ok(())
}

#[tauri::command]
pub fn set_progress_mode(state: State<AppState>, project_id: i64, mode: String) -> AppResult<()> {
    if !["manual", "milestones"].contains(&mode.as_str()) {
        return Err(AppError::msg("invalid progress mode"));
    }
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE projects SET progress_mode = ?1 WHERE id = ?2",
        params![mode, project_id],
    )?;
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

// ---------- Tasks ----------

#[derive(Serialize)]
pub struct TaskRow {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub project_emoji: String,
    pub milestone_id: Option<i64>,
    pub title: String,
    pub done: bool,
    pub due: Option<String>,
    pub priority: String,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
    pub recurrence: Option<String>,
}

fn map_task(r: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: r.get(0)?,
        project_id: r.get(1)?,
        project_name: r.get(2)?,
        project_emoji: r.get(3)?,
        milestone_id: r.get(4)?,
        title: r.get(5)?,
        done: r.get::<_, i64>(6)? != 0,
        due: r.get(7)?,
        priority: r.get(8)?,
        blocked: r.get::<_, i64>(9)? != 0,
        blocked_reason: r.get(10)?,
        recurrence: r.get(11)?,
    })
}

const TASK_COLS: &str = "t.id, t.project_id, p.name, p.emoji, t.milestone_id, t.title, t.done,
    t.due, t.priority, t.blocked, t.blocked_reason, t.recurrence";

#[tauri::command]
pub fn list_tasks(
    state: State<AppState>,
    project_id: i64,
    include_done: Option<bool>,
) -> AppResult<Vec<TaskRow>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.project_id = ?1 {}
         ORDER BY t.done, t.due IS NULL, t.due,
           CASE t.priority WHEN 'high' THEN 0 WHEN 'med' THEN 1 ELSE 2 END, t.id",
        if include_done.unwrap_or(false) { "" } else { "AND t.done = 0" }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([project_id], map_task)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[derive(Deserialize)]
pub struct NewTask {
    pub project_id: i64,
    pub title: String,
    pub due: Option<String>,
    pub priority: Option<String>,
    pub milestone_id: Option<i64>,
    pub recurrence: Option<String>,
}

#[tauri::command]
pub fn add_task(state: State<AppState>, task: NewTask) -> AppResult<i64> {
    let title = task.title.trim();
    if title.is_empty() {
        return Err(AppError::msg("task needs a title"));
    }
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks (project_id, milestone_id, title, due, priority, recurrence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            task.project_id,
            task.milestone_id,
            title,
            task.due,
            task.priority.unwrap_or_else(|| "med".into()),
            task.recurrence
        ],
    )?;
    let id = conn.last_insert_rowid();
    ops::recompute_progress(&conn, task.project_id)?;
    Ok(id)
}

#[tauri::command]
pub fn toggle_task(state: State<AppState>, task_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let (project_id, done, due, recurrence, title): (i64, i64, Option<String>, Option<String>, String) =
        conn.query_row(
            "SELECT project_id, done, due, recurrence, title FROM tasks WHERE id = ?1",
            [task_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let now_done = done == 0;
    conn.execute(
        "UPDATE tasks SET done = ?1, done_at = CASE WHEN ?1 = 1 THEN datetime('now') ELSE NULL END
         WHERE id = ?2",
        params![now_done as i64, task_id],
    )?;
    // Recurring tasks respawn with the next due date.
    if now_done {
        if let (Some(rec), Some(due_str)) = (&recurrence, &due) {
            if let Ok(d) = NaiveDate::parse_from_str(&due_str[..10.min(due_str.len())], "%Y-%m-%d") {
                let next = match rec.as_str() {
                    "daily" => d + Duration::days(1),
                    "weekly" => d + Duration::weeks(1),
                    "monthly" => d + Duration::days(30),
                    _ => d,
                };
                if next != d {
                    conn.execute(
                        "INSERT INTO tasks (project_id, milestone_id, title, due, priority, recurrence)
                         SELECT project_id, milestone_id, title, ?1, priority, recurrence
                         FROM tasks WHERE id = ?2",
                        params![next.format("%Y-%m-%d").to_string(), task_id],
                    )?;
                }
            }
        }
        ops::auto_log(&conn, project_id, &format!("task done: {title}"))?;
    }
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

#[derive(Deserialize)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub due: Option<Option<String>>,
    pub priority: Option<String>,
    pub blocked: Option<bool>,
    pub blocked_reason: Option<Option<String>>,
    pub milestone_id: Option<Option<i64>>,
    pub recurrence: Option<Option<String>>,
}

#[tauri::command]
pub fn update_task(state: State<AppState>, task_id: i64, patch: TaskPatch) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let project_id: i64 =
        conn.query_row("SELECT project_id FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))?;
    if let Some(t) = patch.title {
        conn.execute("UPDATE tasks SET title=?1 WHERE id=?2", params![t.trim(), task_id])?;
    }
    if let Some(due) = patch.due {
        conn.execute("UPDATE tasks SET due=?1 WHERE id=?2", params![due, task_id])?;
    }
    if let Some(p) = patch.priority {
        conn.execute("UPDATE tasks SET priority=?1 WHERE id=?2", params![p, task_id])?;
    }
    if let Some(b) = patch.blocked {
        conn.execute("UPDATE tasks SET blocked=?1 WHERE id=?2", params![b as i64, task_id])?;
    }
    if let Some(reason) = patch.blocked_reason {
        conn.execute("UPDATE tasks SET blocked_reason=?1 WHERE id=?2", params![reason, task_id])?;
    }
    if let Some(mid) = patch.milestone_id {
        conn.execute("UPDATE tasks SET milestone_id=?1 WHERE id=?2", params![mid, task_id])?;
    }
    if let Some(rec) = patch.recurrence {
        conn.execute("UPDATE tasks SET recurrence=?1 WHERE id=?2", params![rec, task_id])?;
    }
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

#[tauri::command]
pub fn delete_task(state: State<AppState>, task_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    let project_id: i64 =
        conn.query_row("SELECT project_id FROM tasks WHERE id = ?1", [task_id], |r| r.get(0))?;
    conn.execute("DELETE FROM tasks WHERE id = ?1", [task_id])?;
    ops::recompute_progress(&conn, project_id)?;
    Ok(())
}

// ---------- Progress stats ----------

#[derive(Serialize)]
pub struct HistoryPoint {
    pub ts: String,
    pub value: i64,
}

#[derive(Serialize)]
pub struct ProgressStats {
    pub history: Vec<HistoryPoint>,
    pub velocity_per_week: f64,
    pub projected_finish: Option<String>,
    pub health: String, // on_track | at_risk | late
    pub days_since_touch: Option<i64>,
    pub hours_this_week: f64,
    pub heatmap: Vec<i64>, // 182 days, oldest first
    pub blocked_count: i64,
}

pub fn compute_stats(conn: &Connection, project_id: i64) -> AppResult<ProgressStats> {
    let mut stmt = conn.prepare(
        "SELECT ts, value FROM progress_history WHERE project_id = ?1 ORDER BY ts",
    )?;
    let history: Vec<HistoryPoint> = stmt
        .query_map([project_id], |r| {
            Ok(HistoryPoint { ts: r.get(0)?, value: r.get(1)? })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let (progress, target_date, status): (i64, Option<String>, String) = conn.query_row(
        "SELECT progress, target_date, status FROM projects WHERE id = ?1",
        [project_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    // Linear fit over the last 30 days of history.
    let now = Utc::now();
    let cutoff = now - Duration::days(30);
    let pts: Vec<(f64, f64)> = history
        .iter()
        .filter_map(|p| {
            let ts = chrono::NaiveDateTime::parse_from_str(&p.ts, "%Y-%m-%d %H:%M:%S").ok()?;
            let ts = ts.and_utc();
            if ts > cutoff {
                Some(((ts - cutoff).num_hours() as f64 / 24.0, p.value as f64))
            } else {
                None
            }
        })
        .collect();
    let slope_per_day = if pts.len() >= 2 {
        let n = pts.len() as f64;
        let sx: f64 = pts.iter().map(|(x, _)| x).sum();
        let sy: f64 = pts.iter().map(|(_, y)| y).sum();
        let sxy: f64 = pts.iter().map(|(x, y)| x * y).sum();
        let sxx: f64 = pts.iter().map(|(x, _)| x * x).sum();
        let denom = n * sxx - sx * sx;
        if denom.abs() > f64::EPSILON {
            (n * sxy - sx * sy) / denom
        } else {
            0.0
        }
    } else {
        0.0
    };

    let projected_finish = if slope_per_day > 0.01 && progress < 100 {
        let days_left = (100 - progress) as f64 / slope_per_day;
        if days_left < 365.0 * 3.0 {
            Some((Local::now() + Duration::days(days_left.ceil() as i64))
                .format("%Y-%m-%d")
                .to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Last touch: newest file mtime or log entry.
    let last_file_ms: Option<i64> = conn
        .query_row(
            "SELECT MAX(mtime) FROM files WHERE project_id = ?1",
            [project_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let days_since_touch = last_file_ms.map(|ms| {
        ((Utc::now().timestamp_millis() - ms) / 86_400_000).max(0)
    });

    // Health.
    let today = Local::now().date_naive();
    let health = if status != "active" {
        "on_track"
    } else if let Some(td) = target_date
        .as_deref()
        .and_then(|t| NaiveDate::parse_from_str(&t[..10.min(t.len())], "%Y-%m-%d").ok())
    {
        if progress < 100 && today > td {
            "late"
        } else if let Some(pf) = projected_finish
            .as_deref()
            .and_then(|p| NaiveDate::parse_from_str(p, "%Y-%m-%d").ok())
        {
            if pf > td {
                "at_risk"
            } else {
                "on_track"
            }
        } else if days_since_touch.unwrap_or(0) > 14 {
            "at_risk"
        } else {
            "on_track"
        }
    } else if days_since_touch.unwrap_or(0) > 21 {
        "at_risk"
    } else {
        "on_track"
    };

    // Hours this week from time entries.
    let week_start = (today - Duration::days(today.weekday().num_days_from_monday() as i64))
        .format("%Y-%m-%d")
        .to_string();
    let hours_this_week: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(
               (julianday(COALESCE(ended_at, datetime('now'))) - julianday(started_at)) * 24.0
             ), 0.0)
             FROM time_entries WHERE project_id = ?1 AND started_at >= ?2",
            params![project_id, week_start],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    // 26-week activity heatmap from log events.
    let mut heatmap = vec![0i64; 182];
    let mut stmt = conn.prepare(
        "SELECT date(ts), COUNT(*) FROM logs WHERE project_id = ?1
         AND ts > datetime('now', '-182 days') GROUP BY date(ts)",
    )?;
    let day_counts: Vec<(String, i64)> = stmt
        .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (date_str, count) in day_counts {
        if let Ok(d) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            let age = (today - d).num_days();
            if (0..182).contains(&age) {
                heatmap[(181 - age) as usize] = count;
            }
        }
    }

    let blocked_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND blocked = 1 AND done = 0",
        [project_id],
        |r| r.get(0),
    )?;

    Ok(ProgressStats {
        history,
        velocity_per_week: (slope_per_day * 7.0 * 10.0).round() / 10.0,
        projected_finish,
        health: health.to_string(),
        days_since_touch,
        hours_this_week: (hours_this_week * 10.0).round() / 10.0,
        heatmap,
        blocked_count,
    })
}

#[tauri::command]
pub fn get_progress_stats(state: State<AppState>, project_id: i64) -> AppResult<ProgressStats> {
    let conn = state.conn.lock().unwrap();
    compute_stats(&conn, project_id)
}

// ---------- Status report ----------

#[tauri::command]
pub fn draft_status_report(state: State<AppState>, project_id: i64) -> AppResult<String> {
    let conn = state.conn.lock().unwrap();
    let (name, progress): (String, i64) = conn.query_row(
        "SELECT name, progress FROM projects WHERE id = ?1",
        [project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let recent: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT body_md FROM logs WHERE project_id = ?1
             AND ts > datetime('now', '-7 days') AND kind IN ('auto','note')
             ORDER BY ts DESC LIMIT 12",
        )?;
        let v = stmt
            .query_map([project_id], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    let open_tasks: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT title FROM tasks WHERE project_id = ?1 AND done = 0
             ORDER BY due IS NULL, due LIMIT 6",
        )?;
        let v = stmt
            .query_map([project_id], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    let blocked: Vec<(String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT title, blocked_reason FROM tasks
             WHERE project_id = ?1 AND blocked = 1 AND done = 0",
        )?;
        let v = stmt
            .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };

    let date = Local::now().format("%Y-%m-%d");
    let mut report = format!("Status — {date}\n\n{name} · {progress}%\n");
    if !recent.is_empty() {
        report.push_str("\nThis week:\n");
        for r in &recent {
            report.push_str(&format!("• {r}\n"));
        }
    }
    if !open_tasks.is_empty() {
        report.push_str("\nNext up:\n");
        for t in &open_tasks {
            report.push_str(&format!("• {t}\n"));
        }
    }
    if !blocked.is_empty() {
        report.push_str("\nBlocked:\n");
        for (t, reason) in &blocked {
            report.push_str(&format!(
                "• {t}{}\n",
                reason.as_deref().map(|r| format!(" — {r}")).unwrap_or_default()
            ));
        }
    }
    conn.execute(
        "INSERT INTO logs (project_id, kind, body_md) VALUES (?1, 'status_report', ?2)",
        params![project_id, report],
    )?;
    Ok(report)
}

// ---------- Orders ----------

#[derive(Serialize)]
pub struct OrderRow {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub vendor: String,
    pub r#ref: Option<String>,
    pub items: Option<String>,
    pub cost_cents: i64,
    pub currency: String,
    pub ordered_at: String,
    pub eta: Option<String>,
    pub status: String,
    pub tracking_url: Option<String>,
    pub notes: Option<String>,
}

fn map_order(r: &rusqlite::Row) -> rusqlite::Result<OrderRow> {
    Ok(OrderRow {
        id: r.get(0)?,
        project_id: r.get(1)?,
        project_name: r.get(2)?,
        vendor: r.get(3)?,
        r#ref: r.get(4)?,
        items: r.get(5)?,
        cost_cents: r.get(6)?,
        currency: r.get(7)?,
        ordered_at: r.get(8)?,
        eta: r.get(9)?,
        status: r.get(10)?,
        tracking_url: r.get(11)?,
        notes: r.get(12)?,
    })
}

const ORDER_COLS: &str = "o.id, o.project_id, p.name, o.vendor, o.ref, o.items, o.cost_cents,
    o.currency, o.ordered_at, o.eta, o.status, o.tracking_url, o.notes";

#[tauri::command]
pub fn list_orders(state: State<AppState>, project_id: Option<i64>) -> AppResult<Vec<OrderRow>> {
    let conn = state.conn.lock().unwrap();
    let rows = match project_id {
        Some(pid) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {ORDER_COLS} FROM orders o JOIN projects p ON p.id = o.project_id
                 WHERE o.project_id = ?1 ORDER BY o.ordered_at DESC"
            ))?;
            let v = stmt.query_map([pid], map_order)?.filter_map(|r| r.ok()).collect();
            v
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {ORDER_COLS} FROM orders o JOIN projects p ON p.id = o.project_id
                 ORDER BY o.ordered_at DESC"
            ))?;
            let v = stmt.query_map([], map_order)?.filter_map(|r| r.ok()).collect();
            v
        }
    };
    Ok(rows)
}

#[derive(Deserialize)]
pub struct NewOrder {
    pub project_id: i64,
    pub vendor: String,
    pub r#ref: Option<String>,
    pub items: Option<String>,
    pub cost_cents: i64,
    pub eta: Option<String>,
    pub tracking_url: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn add_order(state: State<AppState>, order: NewOrder) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO orders (project_id, vendor, ref, items, cost_cents, eta, tracking_url, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            order.project_id, order.vendor.trim(), order.r#ref, order.items,
            order.cost_cents, order.eta, order.tracking_url, order.notes
        ],
    )?;
    ops::auto_log(
        &conn,
        order.project_id,
        &format!("ordered from {} (${:.2})", order.vendor, order.cost_cents as f64 / 100.0),
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_order_status(state: State<AppState>, order_id: i64, status: String) -> AppResult<()> {
    if !["ordered", "shipped", "arrived", "issue"].contains(&status.as_str()) {
        return Err(AppError::msg("invalid order status"));
    }
    let conn = state.conn.lock().unwrap();
    let (project_id, vendor): (i64, String) = conn.query_row(
        "SELECT project_id, vendor FROM orders WHERE id = ?1",
        [order_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute(
        "UPDATE orders SET status = ?1 WHERE id = ?2",
        params![status, order_id],
    )?;
    ops::auto_log(&conn, project_id, &format!("{vendor} order → {status}"))?;
    Ok(())
}

#[tauri::command]
pub fn delete_order(state: State<AppState>, order_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM orders WHERE id = ?1", [order_id])?;
    Ok(())
}

#[derive(Serialize)]
pub struct SpendSummary {
    pub total_cents: i64,
    pub in_flight_cents: i64,
    pub by_month: Vec<(String, i64)>,
}

#[tauri::command]
pub fn spend_summary(state: State<AppState>, project_id: Option<i64>) -> AppResult<SpendSummary> {
    let conn = state.conn.lock().unwrap();
    let filter = project_id.map(|p| format!("WHERE project_id = {p}")).unwrap_or_default();
    let total: i64 = conn.query_row(
        &format!("SELECT COALESCE(SUM(cost_cents),0) FROM orders {filter}"),
        [],
        |r| r.get(0),
    )?;
    let in_flight: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(cost_cents),0) FROM orders {}",
            if filter.is_empty() {
                "WHERE status IN ('ordered','shipped')".to_string()
            } else {
                format!("{filter} AND status IN ('ordered','shipped')")
            }
        ),
        [],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT strftime('%Y-%m', ordered_at), SUM(cost_cents) FROM orders {filter}
         GROUP BY 1 ORDER BY 1 DESC LIMIT 12"
    ))?;
    let by_month: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(SpendSummary {
        total_cents: total,
        in_flight_cents: in_flight,
        by_month,
    })
}

// ---------- Links ----------

#[derive(Serialize)]
pub struct LinkRow {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub kind: String,
}

#[tauri::command]
pub fn list_links(state: State<AppState>, project_id: i64) -> AppResult<Vec<LinkRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt =
        conn.prepare("SELECT id, title, url, kind FROM links WHERE project_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(LinkRow {
                id: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                kind: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_link(
    state: State<AppState>,
    project_id: i64,
    title: String,
    url: String,
    kind: String,
) -> AppResult<i64> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO links (project_id, title, url, kind) VALUES (?1,?2,?3,?4)",
        params![project_id, title.trim(), url.trim(), kind],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_link(state: State<AppState>, link_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM links WHERE id = ?1", [link_id])?;
    Ok(())
}

/// Read-only git badge for the project folder (if it's a repo).
#[derive(Serialize)]
pub struct GitBadge {
    pub branch: String,
    pub dirty: bool,
}

#[tauri::command]
pub fn git_badge(state: State<AppState>, project_id: i64) -> AppResult<Option<GitBadge>> {
    let dir = {
        let conn = state.conn.lock().unwrap();
        ops::project_path(&conn, project_id)?
    };
    if !dir.join(".git").exists() {
        return Ok(None);
    }
    let run = |args: &[&str]| -> Option<String> {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let Some(branch) = run(&["rev-parse", "--abbrev-ref", "HEAD"]) else {
        return Ok(None);
    };
    let dirty = run(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    Ok(Some(GitBadge { branch, dirty }))
}

// ---------- Time tracking ----------

#[derive(Serialize)]
pub struct ActiveTimer {
    pub project_id: i64,
    pub project_name: String,
    pub started_at: String,
}

#[tauri::command]
pub fn start_timer(state: State<AppState>, project_id: i64) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    // One running timer at a time: stop any others first.
    conn.execute(
        "UPDATE time_entries SET ended_at = datetime('now') WHERE ended_at IS NULL",
        [],
    )?;
    conn.execute(
        "INSERT INTO time_entries (project_id, started_at) VALUES (?1, datetime('now'))",
        [project_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn stop_timer(state: State<AppState>) -> AppResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE time_entries SET ended_at = datetime('now') WHERE ended_at IS NULL",
        [],
    )?;
    Ok(())
}

#[tauri::command]
pub fn active_timer(state: State<AppState>) -> AppResult<Option<ActiveTimer>> {
    let conn = state.conn.lock().unwrap();
    let row = conn
        .query_row(
            "SELECT t.project_id, p.name, t.started_at
             FROM time_entries t JOIN projects p ON p.id = t.project_id
             WHERE t.ended_at IS NULL ORDER BY t.id DESC LIMIT 1",
            [],
            |r| {
                Ok(ActiveTimer {
                    project_id: r.get(0)?,
                    project_name: r.get(1)?,
                    started_at: r.get(2)?,
                })
            },
        )
        .ok();
    Ok(row)
}

// ---------- Today & portfolio ----------

#[derive(Serialize)]
pub struct TodayData {
    pub overdue: Vec<TaskRow>,
    pub due_today: Vec<TaskRow>,
    pub high_priority: Vec<TaskRow>,
    pub arriving: Vec<OrderRow>,
    pub suggestions: Vec<(i64, String, String, String)>, // project_id, emoji, name, suggestion
}

#[tauri::command]
pub fn today_data(state: State<AppState>) -> AppResult<TodayData> {
    let conn = state.conn.lock().unwrap();
    let today = Local::now().format("%Y-%m-%d").to_string();

    let query_tasks = |cond: &str| -> AppResult<Vec<TaskRow>> {
        let sql = format!(
            "SELECT {TASK_COLS} FROM tasks t JOIN projects p ON p.id = t.project_id
             WHERE t.done = 0 AND {cond}
             ORDER BY t.due, CASE t.priority WHEN 'high' THEN 0 WHEN 'med' THEN 1 ELSE 2 END
             LIMIT 30"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([&today], map_task)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    };

    let overdue = query_tasks("t.due IS NOT NULL AND date(t.due) < date(?1)")?;
    let due_today = query_tasks("t.due IS NOT NULL AND date(t.due) = date(?1)")?;
    let high_priority = query_tasks(
        "t.priority = 'high' AND (t.due IS NULL OR date(t.due) > date(?1))",
    )?;

    let arriving = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {ORDER_COLS} FROM orders o JOIN projects p ON p.id = o.project_id
             WHERE o.status IN ('ordered','shipped') AND o.eta IS NOT NULL
             AND date(o.eta) <= date(?1, '+3 days') ORDER BY o.eta"
        ))?;
        let v: Vec<OrderRow> = stmt
            .query_map([&today], map_order)?
            .filter_map(|r| r.ok())
            .collect();
        v
    };

    // One suggested next action per active project (rule-based; AI refines later).
    let mut suggestions = vec![];
    let mut stmt = conn.prepare(
        "SELECT id, emoji, name FROM projects WHERE status = 'active' ORDER BY pinned DESC, name",
    )?;
    let projects: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (pid, emoji, name) in projects {
        let blocked: Option<String> = conn
            .query_row(
                "SELECT title FROM tasks WHERE project_id=?1 AND blocked=1 AND done=0 LIMIT 1",
                [pid],
                |r| r.get(0),
            )
            .ok();
        let next_task: Option<String> = conn
            .query_row(
                "SELECT title FROM tasks WHERE project_id=?1 AND done=0 AND blocked=0
                 ORDER BY due IS NULL, due,
                 CASE priority WHEN 'high' THEN 0 WHEN 'med' THEN 1 ELSE 2 END LIMIT 1",
                [pid],
                |r| r.get(0),
            )
            .ok();
        let doing_milestone: Option<String> = conn
            .query_row(
                "SELECT title FROM milestones WHERE project_id=?1 AND state='doing'
                 ORDER BY sort_order LIMIT 1",
                [pid],
                |r| r.get(0),
            )
            .ok();
        let suggestion = if let Some(b) = blocked {
            format!("Unblock: {b}")
        } else if let Some(t) = next_task {
            format!("Next: {t}")
        } else if let Some(m) = doing_milestone {
            format!("Push milestone: {m}")
        } else {
            "Add the next task or milestone".to_string()
        };
        suggestions.push((pid, emoji, name, suggestion));
    }

    Ok(TodayData {
        overdue,
        due_today,
        high_priority,
        arriving,
        suggestions,
    })
}

#[derive(Serialize)]
pub struct PortfolioRow {
    pub id: i64,
    pub emoji: String,
    pub name: String,
    pub color: String,
    pub progress: i64,
    pub health: String,
    pub velocity_per_week: f64,
    pub target_date: Option<String>,
    pub projected_finish: Option<String>,
    pub days_since_touch: Option<i64>,
    pub blocked_count: i64,
    pub history: Vec<HistoryPoint>,
}

#[derive(Serialize)]
pub struct HealthRollup {
    pub active: i64,
    pub at_risk: i64,
    pub late: i64,
    pub open_orders: i64,
    pub in_flight_cents: i64,
    pub disk_free_bytes: i64,
    pub hours_this_week: f64,
}

#[tauri::command]
pub fn portfolio(state: State<AppState>) -> AppResult<Vec<PortfolioRow>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, emoji, name, color, progress, target_date FROM projects
         WHERE status = 'active' ORDER BY pinned DESC, updated_at DESC",
    )?;
    let base: Vec<(i64, String, String, String, i64, Option<String>)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    let mut rows = vec![];
    for (id, emoji, name, color, progress, target_date) in base {
        let stats = compute_stats(&conn, id)?;
        rows.push(PortfolioRow {
            id,
            emoji,
            name,
            color,
            progress,
            health: stats.health,
            velocity_per_week: stats.velocity_per_week,
            target_date,
            projected_finish: stats.projected_finish,
            days_since_touch: stats.days_since_touch,
            blocked_count: stats.blocked_count,
            history: stats.history,
        });
    }
    Ok(rows)
}

#[tauri::command]
pub fn health_rollup(state: State<AppState>) -> AppResult<HealthRollup> {
    let conn = state.conn.lock().unwrap();
    let active: i64 =
        conn.query_row("SELECT COUNT(*) FROM projects WHERE status='active'", [], |r| r.get(0))?;
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM projects WHERE status='active'")?;
        let v = stmt
            .query_map([], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    let (mut at_risk, mut late) = (0i64, 0i64);
    let mut hours = 0.0f64;
    for id in ids {
        let s = compute_stats(&conn, id)?;
        match s.health.as_str() {
            "at_risk" => at_risk += 1,
            "late" => late += 1,
            _ => {}
        }
        hours += s.hours_this_week;
    }
    let open_orders: i64 = conn.query_row(
        "SELECT COUNT(*) FROM orders WHERE status IN ('ordered','shipped')",
        [],
        |r| r.get(0),
    )?;
    let in_flight_cents: i64 = conn.query_row(
        "SELECT COALESCE(SUM(cost_cents),0) FROM orders WHERE status IN ('ordered','shipped')",
        [],
        |r| r.get(0),
    )?;
    // Free space on the volume holding the root.
    let root = crate::db::get_setting(&conn, "root")?.unwrap_or_else(|| "/".into());
    let disk_free_bytes = free_space(&root).unwrap_or(0);

    Ok(HealthRollup {
        active,
        at_risk,
        late,
        open_orders,
        in_flight_cents,
        disk_free_bytes,
        hours_this_week: (hours * 10.0).round() / 10.0,
    })
}

fn free_space(path: &str) -> Option<i64> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    let c_path = CString::new(path).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some(stat.f_bavail as i64 * stat.f_frsize as i64)
}
