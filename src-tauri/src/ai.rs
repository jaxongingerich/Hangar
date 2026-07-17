use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const KEYRING_SERVICE: &str = "com.hangar.app";
pub const KEYRING_USER: &str = "anthropic_api_key";

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

#[derive(Debug, Clone)]
pub enum Provider {
    Anthropic { key: String, model: String },
    Ollama { base: String, model: String },
    OpenAiCompat { base: String, key: Option<String>, model: String },
}

impl Provider {
    pub fn model_name(&self) -> &str {
        match self {
            Provider::Anthropic { model, .. } => model,
            Provider::Ollama { model, .. } => model,
            Provider::OpenAiCompat { model, .. } => model,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Provider::Anthropic { .. } => "anthropic",
            Provider::Ollama { .. } => "ollama",
            Provider::OpenAiCompat { .. } => "openai-compat",
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
        }
    }
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
}
