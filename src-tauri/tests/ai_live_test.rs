//! Live diagnostics for the AI hub — these talk to the real `claude` CLI and
//! the real local Ollama server, so they're #[ignore]d by default and only run
//! with `cargo test --test ai_live_test -- --ignored`. They reproduce exactly
//! what the app does from a GUI (Finder) launch, where PATH is stripped.

use hangar_lib::ai::{ChatMessage, CliFlavor, Provider};

/// Simulate a double-clicked .app: launchd hands the process a minimal PATH,
/// so `claude` in ~/.local/bin is invisible unless the app rebuilds PATH.
fn strip_path_like_gui_launch() {
    std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
}

#[tokio::test]
#[ignore]
async fn claude_cli_answers_under_stripped_path() {
    strip_path_like_gui_launch();
    let provider = Provider::Cli {
        command: "claude".into(),
        model: None,
        extra_args: vec![],
        flavor: CliFlavor::ClaudeCode,
    };
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: "reply with the single word: ready".into(),
    }];
    let resp = provider
        .chat("Be terse.", &msgs)
        .await
        .expect("claude CLI should answer even with a stripped PATH");
    println!("claude reply: {:?}", resp.text);
    assert!(!resp.text.trim().is_empty(), "reply must not be empty");
}

#[tokio::test]
#[ignore]
async fn ollama_answers_with_an_installed_model() {
    // Ask the server what's actually installed, then use the first model —
    // this is what detection must do instead of hardcoding llama3.2.
    let tags: serde_json::Value = reqwest::get("http://127.0.0.1:11434/api/tags")
        .await
        .expect("ollama up")
        .json()
        .await
        .expect("tags json");
    let model = tags["models"][0]["name"]
        .as_str()
        .expect("at least one model pulled")
        .to_string();
    println!("using ollama model: {model}");
    let provider = Provider::Ollama {
        base: "http://localhost:11434".into(),
        model,
    };
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: "reply with the single word: ready".into(),
    }];
    let resp = provider
        .chat("Be terse.", &msgs)
        .await
        .expect("ollama should answer with an installed model");
    println!("ollama reply: {:?}", resp.text);
    assert!(!resp.text.trim().is_empty(), "reply must not be empty");
}

/// Regression for the reported "messages don't go through": an Ollama profile
/// pointing at a model that ISN'T pulled (or with no model set) must now
/// auto-resolve to an installed model and answer, instead of erroring.
#[tokio::test]
#[ignore]
async fn ollama_auto_resolves_a_missing_model() {
    for requested in ["llama3.2", ""] {
        let provider = Provider::Ollama {
            base: "http://localhost:11434".into(),
            model: requested.into(),
        };
        let msgs = vec![ChatMessage {
            role: "user".into(),
            content: "reply with the single word: ready".into(),
        }];
        let resp = provider
            .chat("Be terse.", &msgs)
            .await
            .unwrap_or_else(|e| panic!("model {requested:?} should auto-resolve, got: {e:?}"));
        println!("requested {requested:?} -> reply: {:?}", resp.text);
        assert!(!resp.text.trim().is_empty());
    }
}
