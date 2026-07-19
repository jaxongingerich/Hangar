//! The in-project chat can change the project, not just talk about it. These
//! tests cover the two halves of that:
//!
//!  1. Given tool calls, they really run against the database and the project
//!     folder (no AI needed — runs everywhere).
//!  2. A real model, handed the action protocol, actually emits calls in the
//!     shape we parse. That half is #[ignore]d because it needs the `claude`
//!     CLI logged in on this Mac:
//!     `cargo test --test project_agent_test -- --ignored --nocapture`

use hangar_lib::commands_m7::{action_protocol, execute, parse_actions, ToolCall, EVAL_SYSTEM};
use hangar_lib::{db, mcp, scan};
use serde_json::json;

fn setup(tag: &str) -> (rusqlite::Connection, std::path::PathBuf, i64) {
    let root = std::env::temp_dir().join(format!("hangar-agent-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Widget")).unwrap();
    std::fs::write(root.join("Widget").join("notes.md"), "# Widget\nrev A\n").unwrap();
    let mut conn = db::open(&root.join("test.db")).unwrap();
    scan::scan(&mut conn, &root).unwrap();
    let pid: i64 = conn
        .query_row("SELECT id FROM projects WHERE name='Widget'", [], |r| r.get(0))
        .unwrap();
    (conn, root, pid)
}

fn state(root: &std::path::Path) -> mcp::McpState {
    mcp::McpState {
        db_path: root.join("test.db"),
        token: String::new(),
        app: None,
    }
}

/// The end the user actually feels: "add a task and set progress" has to leave
/// a task and a percentage behind.
#[test]
fn tool_calls_change_the_real_project() {
    let (conn, root, pid) = setup("exec");
    drop(conn); // execute() opens its own connection, same as the MCP server

    let calls = vec![
        ToolCall {
            tool: "add_task".into(),
            args: json!({"title": "panelize gerbers", "priority": "high"}),
        },
        ToolCall {
            tool: "set_progress".into(),
            args: json!({"value": 42}),
        },
        ToolCall {
            tool: "add_log".into(),
            args: json!({"body": "kicked off panelization"}),
        },
    ];
    let results = execute(&state(&root), pid, calls);

    assert_eq!(results.len(), 3);
    assert!(
        results.iter().all(|r| r.ok),
        "every action should succeed: {:?}",
        results.iter().map(|r| (&r.label, &r.detail)).collect::<Vec<_>>()
    );

    let conn = db::open(&root.join("test.db")).unwrap();
    let task: String = conn
        .query_row("SELECT title FROM tasks WHERE project_id = ?1", [pid], |r| r.get(0))
        .unwrap();
    assert_eq!(task, "panelize gerbers");
    let progress: i64 = conn
        .query_row("SELECT progress FROM projects WHERE id = ?1", [pid], |r| r.get(0))
        .unwrap();
    assert_eq!(progress, 42);
    let logs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM logs WHERE project_id = ?1 AND body_md LIKE '%panelization%'",
            [pid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logs, 1);
}

/// A model that names the wrong project must not be able to reach it — the
/// executor pins every call to the project the user has open.
#[test]
fn calls_are_pinned_to_the_open_project() {
    let (mut conn, root, pid) = setup("pinned");
    // A second project the chat has no business touching.
    std::fs::create_dir_all(root.join("Other")).unwrap();
    scan::scan(&mut conn, &root).unwrap();
    let other: i64 = conn
        .query_row("SELECT id FROM projects WHERE name='Other'", [], |r| r.get(0))
        .unwrap();
    drop(conn);

    let calls = vec![ToolCall {
        tool: "add_task".into(),
        // The model asks for the *other* project.
        args: json!({"project_id": other, "title": "should not land here"}),
    }];
    execute(&state(&root), pid, calls);

    let conn = db::open(&root.join("test.db")).unwrap();
    let here: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1", [pid], |r| r.get(0))
        .unwrap();
    let there: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1", [other], |r| r.get(0))
        .unwrap();
    assert_eq!(here, 1, "task belongs to the project the user has open");
    assert_eq!(there, 0, "the other project must be untouched");
}

/// File operations need `confirm: true`; the executor supplies it, so a move
/// requested in chat actually moves the file on disk.
#[test]
fn file_moves_are_pre_confirmed_and_hit_the_disk() {
    let (conn, root, pid) = setup("files");
    let file_id: i64 = conn
        .query_row("SELECT id FROM files WHERE project_id = ?1", [pid], |r| r.get(0))
        .unwrap();
    drop(conn);

    let calls = vec![
        ToolCall {
            tool: "create_bin".into(),
            args: json!({"name": "Docs"}),
        },
        ToolCall {
            tool: "move_files".into(),
            args: json!({"file_ids": [file_id], "dest_bin": "Docs"}),
        },
    ];
    let results = execute(&state(&root), pid, calls);
    assert!(
        results.iter().all(|r| r.ok),
        "confirm should be supplied automatically: {:?}",
        results.iter().map(|r| (&r.label, &r.detail)).collect::<Vec<_>>()
    );
    assert!(
        root.join("Widget").join("Docs").join("notes.md").exists(),
        "the file should have moved on disk, not just in the database"
    );
}

/// A real model, given the protocol, emits calls we can parse and run. This is
/// the part unit tests can't prove: prompt wording versus an actual model.
#[test]
#[ignore = "needs the claude CLI logged in on this Mac"]
fn a_real_model_emits_runnable_tool_calls() {
    use hangar_lib::ai::{ChatMessage, CliFlavor, Provider};

    let (conn, root, pid) = setup("live");
    drop(conn);

    let provider = Provider::Cli {
        command: "claude".into(),
        model: None,
        extra_args: vec![],
        flavor: CliFlavor::ClaudeCode,
    };
    let system = format!(
        "You are Hangar's project assistant.\n\nProject: Widget · status active · 0%\n{}",
        action_protocol(pid)
    );
    let reply = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.chat(
            &system,
            &[ChatMessage {
                role: "user".into(),
                content: "add a task called panelize gerbers, and set progress to 35%".into(),
            }],
        ))
        .expect("provider call failed");

    println!("model replied: {}", reply.text);
    let v = hangar_lib::ai::extract_json(&reply.text)
        .expect("model should reply with the JSON action object");
    let calls = parse_actions(&v);
    assert!(!calls.is_empty(), "expected tool calls, got: {}", reply.text);
    assert!(
        calls.iter().any(|c| c.tool == "add_task"),
        "expected an add_task call, got {:?}",
        calls.iter().map(|c| &c.tool).collect::<Vec<_>>()
    );

    // And they run.
    let results = execute(&state(&root), pid, calls);
    assert!(results.iter().any(|r| r.ok && r.tool == "add_task"));
    let conn = db::open(&root.join("test.db")).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1", [pid], |r| r.get(0))
        .unwrap();
    assert!(n >= 1, "the task the model asked for should exist");
}

/// The progress estimate has to come back as parseable JSON with a number and
/// reasoning — the UI has nothing to show otherwise.
#[test]
#[ignore = "needs the claude CLI logged in on this Mac"]
fn a_real_model_returns_a_usable_progress_estimate() {
    use hangar_lib::ai::{ChatMessage, CliFlavor, Provider};

    let provider = Provider::Cli {
        command: "claude".into(),
        model: None,
        extra_args: vec![],
        flavor: CliFlavor::ClaudeCode,
    };
    // A project that is clearly mid-flight: half the milestones done, the
    // remaining ones early-stage, so a sane estimate lands well under 100.
    let brief = "Project: Widget · status active · 10%\n\n\
        Milestones:\n- [done] Schematic\n- [done] Layout\n- [doing] Prototype build\n\
        - [todo] Firmware\n- [todo] Enclosure\n- [todo] Production run\n\n\
        Open tasks:\n- order stencils\n- write firmware bootloader\n- design enclosure\n\n\
        Recent log:\n- boards arrived from fab\n- first article assembled, powers up\n";
    let reply = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.chat(
            EVAL_SYSTEM,
            &[ChatMessage {
                role: "user".into(),
                content: format!("{brief}\nThe progress bar currently reads 10%. Estimate independently."),
            }],
        ))
        .expect("provider call failed");

    println!("model replied: {}", reply.text);
    let v = hangar_lib::ai::extract_json(&reply.text).expect("expected JSON");
    let percent = v["percent"].as_i64().expect("percent must be a number");
    assert!((0..=100).contains(&percent), "percent out of range: {percent}");
    assert!(
        (20..=75).contains(&percent),
        "a half-done project should land mid-range, got {percent}%"
    );
    assert!(
        !v["summary"].as_str().unwrap_or("").trim().is_empty(),
        "summary must not be empty"
    );
    assert!(
        v["reasons"].as_array().map(|a| a.len()).unwrap_or(0) >= 2,
        "expected at least 2 reasons"
    );
}

/// The opposite failure: a question must get a prose answer, not a stray tool
/// call. A model that acts on "what should I do next?" would be worse than one
/// that can't act at all.
#[test]
#[ignore = "needs the claude CLI logged in on this Mac"]
fn a_question_gets_prose_not_actions() {
    use hangar_lib::ai::{ChatMessage, CliFlavor, Provider};

    let provider = Provider::Cli {
        command: "claude".into(),
        model: None,
        extra_args: vec![],
        flavor: CliFlavor::ClaudeCode,
    };
    let system = format!(
        "You are Hangar's project assistant.\n\nProject: Widget · status active · 30%\n\
         Milestones:\n- [done] Schematic\n- [doing] Layout\n- [todo] Firmware\n\
         Open tasks:\n- order stencils\n{}",
        action_protocol(1)
    );
    let reply = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(provider.chat(
            &system,
            &[ChatMessage {
                role: "user".into(),
                content: "what should I work on next, and why?".into(),
            }],
        ))
        .expect("provider call failed");

    println!("model replied: {}", reply.text);
    let calls = hangar_lib::ai::extract_json(&reply.text)
        .map(|v| parse_actions(&v))
        .unwrap_or_default();
    assert!(
        calls.is_empty(),
        "a question must not trigger changes, got {:?}",
        calls.iter().map(|c| &c.tool).collect::<Vec<_>>()
    );
    assert!(
        reply.text.trim().len() > 40,
        "expected a real prose answer, got: {}",
        reply.text
    );
}
