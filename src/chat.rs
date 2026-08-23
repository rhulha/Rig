use crate::config::{Model, Provider};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub struct Client {
    http: reqwest::blocking::Client,
    url: String,
    api_key: Option<String>,
    model: String,
    max_tokens_field: String,
    reasoning_effort: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Assistant {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

impl ToolCall {
    /// Arguments arrive as a JSON string; an empty or malformed one becomes `{}`.
    pub fn args(&self) -> Value {
        serde_json::from_str(&self.function.arguments).unwrap_or_else(|_| json!({}))
    }
}

impl Client {
    pub fn new(provider: &Provider, model: &Model, effort: Option<String>) -> Result<Self, String> {
        if let Some(api) = &provider.api {
            if api != "openai-completions" {
                return Err(format!("unsupported provider api '{api}'"));
            }
        }

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| format!("cannot build http client: {e}"))?;

        Ok(Self {
            http,
            url: format!("{}/chat/completions", provider.base_url.trim_end_matches('/')),
            api_key: provider.api_key.clone(),
            model: model.id.clone(),
            max_tokens_field: model.compat.max_tokens_field().to_string(),
            reasoning_effort: effort.filter(|_| model.compat.supports_reasoning_effort),
        })
    }

    /// One round trip. Returns the assistant message both raw (to append to the
    /// history verbatim) and parsed (to drive the loop).
    pub fn complete(&self, messages: &[Value], tools: &Value) -> Result<(Value, Assistant), String> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools,
            "temperature": 0,
        });
        body[&self.max_tokens_field] = json!(4096);
        if let Some(effort) = &self.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }

        let mut req = self.http.post(&self.url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| format!("cannot read response: {e}"))?;
        if !status.is_success() {
            return Err(format!("{status}: {text}"));
        }

        let parsed: Value =
            serde_json::from_str(&text).map_err(|e| format!("bad JSON from provider: {e}\n{text}"))?;
        let raw = parsed["choices"][0]["message"].clone();
        if raw.is_null() {
            return Err(format!("no message in response: {text}"));
        }
        let assistant: Assistant = serde_json::from_value(raw.clone())
            .map_err(|e| format!("unexpected message shape: {e}\n{raw}"))?;

        Ok((raw, assistant))
    }
}
