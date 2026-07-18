use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const KEYRING_SERVICE: &str = "com.hangar.app";
pub const KEYRING_USER: &str = "anthropic_api_key";

/// The real user `PATH`, resolved once and cached.
///
/// A double-clicked macOS `.app` inherits almost no `PATH` (launchd gives it
/// `/usr/bin:/bin:/usr/sbin:/sbin` at most), so CLIs installed in
/// `~/.local/bin`, Homebrew, etc. are invisible unless we reconstruct it. We
/// seed common locations, then ask the login shell for its `PATH` to pick up
/// anything custom (nvm, asdf, pyenv…).
pub fn user_path() -> &'static str {
    static USER_PATH: OnceLock<String> = OnceLock::new();
    USER_PATH.get_or_init(|| {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut dirs: Vec<String> = vec![
            format!("{home}/.local/bin"),
            format!("{home}/bin"),
            format!("{home}/.cargo/bin"),
            "/opt/homebrew/bin".into(),
            "/opt/homebrew/sbin".into(),
            "/usr/local/bin".into(),
            "/usr/bin".into(),
            "/bin".into(),
            "/usr/sbin".into(),
            "/sbin".into(),
        ];
        let push_unique = |p: &str, dirs: &mut Vec<String>| {
            if !p.is_empty() && !dirs.iter().any(|d| d == p) {
                dirs.push(p.to_string());
            }
        };
        // Ask the login shell for its PATH (best-effort, non-interactive so it
        // can never hang waiting on a prompt).
        if let Ok(shell) = std::env::var("SHELL") {
            if let Ok(out) = std::process::Command::new(&shell)
                .args(["-l", "-c", "printf %s \"$PATH\""])
                .stdin(std::process::Stdio::null())
                .output()
            {
                if out.status.success() {
                    let resolved = String::from_utf8_lossy(&out.stdout);
                    let extra: Vec<String> = resolved.trim().split(':').map(String::from).collect();
                    for d in extra {
                        push_unique(&d, &mut dirs);
                    }
                }
            }
        }
        if let Ok(existing) = std::env::var("PATH") {
            let extra: Vec<String> = existing.split(':').map(String::from).collect();
            for d in extra {
                push_unique(&d, &mut dirs);
            }
        }
        dirs.join(":")
    })
}

/// Resolve a command name to an absolute executable path using [`user_path`].
/// Returns `None` if it isn't found or isn't executable.
pub fn which_bin(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    for dir in user_path().split(':') {
        let candidate = std::path::Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct AiResponse {
    pub text: String,
    pub tokens_in: i64,
    pub tokens_out: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CliFlavor {
    /// The `claude` CLI (Claude Code) — already authenticated on this Mac,
    /// no API key involved. Headless, JSON output, tool use locked off.
    ClaudeCode,
    /// Any other single-shot CLI the user points at directly.
    Custom,
}

#[derive(Debug, Clone)]
pub enum Provider {
    Anthropic { key: String, model: String },
    Ollama { base: String, model: String },
    OpenAiCompat { base: String, key: Option<String>, model: String },
    /// Shells out to a local, already-authenticated CLI instead of calling
    /// an HTTP API — no key stored or entered anywhere.
    Cli {
        command: String,
        model: Option<String>,
        extra_args: Vec<String>,
        flavor: CliFlavor,
    },
}

impl Provider {
    pub fn model_name(&self) -> &str {
        match self {
            Provider::Anthropic { model, .. } => model,
            Provider::Ollama { model, .. } => model,
            Provider::OpenAiCompat { model, .. } => model,
            Provider::Cli { model, command, .. } => model.as_deref().unwrap_or(command),
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Provider::Anthropic { .. } => "anthropic",
            Provider::Ollama { .. } => "ollama",
            Provider::OpenAiCompat { .. } => "openai-compat",
            Provider::Cli { .. } => "cli",
        }
    }

    pub async fn chat(&self, system: &str, messages: &[ChatMessage]) -> AppResult<AiResponse> {
        let client = reqwest::Client::new();
        match self {
            Provider::Anthropic { key, model } => {
                let body = json!({
                    "model": model,
                    "max_tokens": 4096,
                    "system": system,
                    "messages": messages,
                });
                let resp = client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AppError::msg(format!("Anthropic request failed: {e}")))?;
                let status = resp.status();
                let v: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::msg(format!("bad Anthropic response: {e}")))?;
                if !status.is_success() {
                    let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
                    return Err(AppError::msg(format!("Anthropic: {msg}")));
                }
                let text = v["content"]
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                Ok(AiResponse {
                    text,
                    tokens_in: v["usage"]["input_tokens"].as_i64().unwrap_or(0),
                    tokens_out: v["usage"]["output_tokens"].as_i64().unwrap_or(0),
                })
            }
            Provider::Ollama { base, model } => {
                // The configured model may be empty (connected via detection) or
                // one that isn't pulled on this machine. Resolve to a model that
                // is actually installed so a send never fails with "not found".
                let model = ollama_resolve_model(&client, base, model).await?;
                let mut msgs = vec![json!({"role": "system", "content": system})];
                msgs.extend(messages.iter().map(|m| json!({"role": m.role, "content": m.content})));
                let body = json!({ "model": model, "messages": msgs, "stream": false });
                let resp = client
                    .post(format!("{}/api/chat", base.trim_end_matches('/')))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AppError::msg(format!("Ollama unreachable: {e}")))?;
                let v: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::msg(format!("bad Ollama response: {e}")))?;
                if let Some(err) = v["error"].as_str() {
                    return Err(AppError::msg(format!("Ollama: {err}")));
                }
                Ok(AiResponse {
                    text: v["message"]["content"].as_str().unwrap_or("").to_string(),
                    tokens_in: v["prompt_eval_count"].as_i64().unwrap_or(0),
                    tokens_out: v["eval_count"].as_i64().unwrap_or(0),
                })
            }
            Provider::OpenAiCompat { base, key, model } => {
                let mut msgs = vec![json!({"role": "system", "content": system})];
                msgs.extend(messages.iter().map(|m| json!({"role": m.role, "content": m.content})));
                let body = json!({ "model": model, "messages": msgs });
                let mut req = client
                    .post(format!("{}/chat/completions", base.trim_end_matches('/')))
                    .json(&body);
                if let Some(k) = key {
                    req = req.header("Authorization", format!("Bearer {k}"));
                }
                let resp = req
                    .send()
                    .await
                    .map_err(|e| AppError::msg(format!("endpoint unreachable: {e}")))?;
                let v: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::msg(format!("bad response: {e}")))?;
                if let Some(err) = v["error"]["message"].as_str() {
                    return Err(AppError::msg(format!("API error: {err}")));
                }
                Ok(AiResponse {
                    text: v["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    tokens_in: v["usage"]["prompt_tokens"].as_i64().unwrap_or(0),
                    tokens_out: v["usage"]["completion_tokens"].as_i64().unwrap_or(0),
                })
            }
            Provider::Cli { command, model, extra_args, flavor } => {
                let transcript = render_transcript(messages);
                let cwd = cli_sandbox_dir();
                // Resolve to an absolute path so it works from a GUI launch
                // where PATH is stripped, and hand the child a real PATH too.
                let program = which_bin(command).ok_or_else(|| {
                    AppError::msg(format!(
                        "couldn't find \"{command}\" on this Mac. Open a terminal and check it's installed and on your PATH."
                    ))
                })?;
                match flavor {
                    CliFlavor::ClaudeCode => {
                        let mut cmd = tokio::process::Command::new(&program);
                        cmd.env("PATH", user_path())
                            .arg("-p")
                            .arg(&transcript)
                            .arg("--system-prompt")
                            .arg(system)
                            .arg("--output-format")
                            .arg("json")
                            // No tool access and no interactive prompts — this is a
                            // plain chat call, not an agent session.
                            .arg("--allowedTools")
                            .arg("")
                            .arg("--permission-mode")
                            .arg("dontAsk")
                            .stdin(std::process::Stdio::null())
                            .kill_on_drop(true);
                        if let Some(m) = model {
                            if !m.is_empty() {
                                cmd.arg("--model").arg(m);
                            }
                        }
                        if let Some(dir) = &cwd {
                            cmd.current_dir(dir);
                        }
                        let out = cmd.output().await.map_err(|e| {
                            AppError::msg(format!("couldn't run \"{command}\": {e}"))
                        })?;
                        if !out.status.success() {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            return Err(AppError::msg(format!(
                                "{command} exited with an error: {}",
                                stderr.trim()
                            )));
                        }
                        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
                            .map_err(|e| {
                                AppError::msg(format!("bad response from {command}: {e}"))
                            })?;
                        if v["is_error"].as_bool().unwrap_or(false) {
                            return Err(AppError::msg(
                                v["result"]
                                    .as_str()
                                    .unwrap_or("the CLI reported an error")
                                    .to_string(),
                            ));
                        }
                        Ok(AiResponse {
                            text: v["result"].as_str().unwrap_or_default().to_string(),
                            tokens_in: v["usage"]["input_tokens"].as_i64().unwrap_or(0),
                            tokens_out: v["usage"]["output_tokens"].as_i64().unwrap_or(0),
                        })
                    }
                    CliFlavor::Custom => {
                        // Generic CLIs have no system-prompt flag, so fold the
                        // system context into the prompt itself.
                        let prompt = if system.trim().is_empty() {
                            transcript.clone()
                        } else {
                            format!("{system}\n\n{transcript}")
                        };
                        let mut cmd = tokio::process::Command::new(&program);
                        cmd.env("PATH", user_path())
                            .args(extra_args)
                            .arg(&prompt)
                            .stdin(std::process::Stdio::null())
                            .kill_on_drop(true);
                        if let Some(dir) = &cwd {
                            cmd.current_dir(dir);
                        }
                        let out = cmd.output().await.map_err(|e| {
                            AppError::msg(format!("couldn't run \"{command}\": {e}"))
                        })?;
                        if !out.status.success() {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            return Err(AppError::msg(format!(
                                "{command} exited with an error: {}",
                                stderr.trim()
                            )));
                        }
                        Ok(AiResponse {
                            text: String::from_utf8_lossy(&out.stdout).trim().to_string(),
                            tokens_in: 0,
                            tokens_out: 0,
                        })
                    }
                }
            }
        }
    }
}

/// List the models actually pulled on a local Ollama server, distinguishing
/// "server unreachable" from "server up but empty" so callers can say something
/// accurate. Newest first, as Ollama returns them.
pub async fn ollama_models_checked(client: &reqwest::Client, base: &str) -> AppResult<Vec<String>> {
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let resp = client.get(&url).send().await.map_err(|_| {
        AppError::msg(
            "Ollama isn't running — open the Ollama app (or run `ollama serve`), then try again.",
        )
    })?;
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::msg(format!("unexpected response from Ollama: {e}")))?;
    Ok(v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// Infallible variant for detection: empty when the server is down or has no
/// models (detection just shouldn't list Ollama in that case).
pub async fn ollama_models(client: &reqwest::Client, base: &str) -> Vec<String> {
    ollama_models_checked(client, base).await.unwrap_or_default()
}

/// Pick a usable Ollama model: the requested one if it's installed (matched
/// with or without an explicit `:tag`), otherwise the first installed model.
/// Errors with an accurate message when the server is down or has no models.
async fn ollama_resolve_model(
    client: &reqwest::Client,
    base: &str,
    requested: &str,
) -> AppResult<String> {
    let installed = ollama_models_checked(client, base).await?;
    if installed.is_empty() {
        return Err(AppError::msg(
            "Ollama is running but has no models yet — pull one first, e.g. `ollama pull llama3.2`.",
        ));
    }
    let want = requested.trim();
    if !want.is_empty() {
        let hit = installed.iter().any(|n| {
            n == want || n.split(':').next() == Some(want) || Some(n.as_str()) == want.split(':').next()
        });
        if hit {
            // Use the exact installed name (so a bare "llama3.2" maps to
            // "llama3.2:latest" if that's what's pulled).
            let exact = installed
                .iter()
                .find(|n| n.as_str() == want)
                .cloned()
                .or_else(|| {
                    installed
                        .iter()
                        .find(|n| n.split(':').next() == Some(want))
                        .cloned()
                });
            return Ok(exact.unwrap_or_else(|| want.to_string()));
        }
    }
    Ok(installed[0].clone())
}

/// Flatten a conversation into one prompt string for single-shot CLIs, which
/// have no notion of a `messages` array — Hangar already resends the full
/// history each turn (same as every other provider), so context isn't lost.
fn render_transcript(messages: &[ChatMessage]) -> String {
    if messages.len() == 1 {
        return messages[0].content.clone();
    }
    let mut out = String::new();
    for m in messages {
        let who = if m.role == "user" { "User" } else { "Assistant" };
        out.push_str(&format!("{who}: {}\n\n", m.content));
    }
    out.push_str("Assistant:");
    out
}

/// A throwaway working directory for CLI subprocesses — keeps a headless
/// coding-agent CLI from picking up project context it wasn't invited into.
fn cli_sandbox_dir() -> Option<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("hangar-ai-cli");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Pull the first JSON array or object out of a model reply that may be
/// wrapped in prose or code fences.
pub fn extract_json(text: &str) -> AppResult<serde_json::Value> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str(cleaned) {
        return Ok(v);
    }
    for open in ['[', '{'] {
        let close = if open == '[' { ']' } else { '}' };
        if let (Some(start), Some(end)) = (cleaned.find(open), cleaned.rfind(close)) {
            if end > start {
                if let Ok(v) = serde_json::from_str(&cleaned[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    Err(AppError::msg("model reply contained no parseable JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fenced_json() {
        let v = extract_json("```json\n[{\"a\": 1}]\n```").unwrap();
        assert_eq!(v[0]["a"], 1);
    }

    #[test]
    fn extracts_embedded_json() {
        let v = extract_json("Sure! Here's the plan:\n[{\"file\": \"x\"}]\nLet me know.").unwrap();
        assert_eq!(v[0]["file"], "x");
    }

    #[test]
    fn rejects_no_json() {
        assert!(extract_json("no structured data here").is_err());
    }

    #[test]
    fn user_path_has_common_locations() {
        // A GUI launch strips PATH; user_path() must still include the usual
        // spots CLIs get installed, so detection and subprocesses work.
        let p = user_path();
        assert!(p.contains("/usr/bin"));
        assert!(p.contains(".local/bin") || p.contains("/opt/homebrew/bin"));
    }

    #[test]
    fn which_bin_resolves_absolute_paths() {
        // Absolute path that exists resolves; a bogus name doesn't.
        assert_eq!(which_bin("/bin/sh"), Some(std::path::PathBuf::from("/bin/sh")));
        assert!(which_bin("definitely-not-a-real-binary-xyz").is_none());
    }

    #[test]
    fn which_bin_finds_sh_on_path() {
        // `sh` lives in /bin, which user_path() always includes.
        assert!(which_bin("sh").is_some());
    }

    #[test]
    fn single_message_transcript_is_verbatim() {
        let msgs = vec![ChatMessage { role: "user".into(), content: "hello".into() }];
        assert_eq!(render_transcript(&msgs), "hello");
    }

    #[test]
    fn multi_message_transcript_is_labelled() {
        let msgs = vec![
            ChatMessage { role: "user".into(), content: "hi".into() },
            ChatMessage { role: "assistant".into(), content: "hey".into() },
            ChatMessage { role: "user".into(), content: "bye".into() },
        ];
        let t = render_transcript(&msgs);
        assert!(t.contains("User: hi"));
        assert!(t.contains("Assistant: hey"));
        assert!(t.trim_end().ends_with("Assistant:"));
    }
}
