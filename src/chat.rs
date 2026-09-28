use crate::config::{Model, Provider};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const OPENAI_MAX_TOKENS: u32 = 4096;
const ANTHROPIC_MAX_TOKENS: u32 = 16000;
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone, Copy)]
pub enum Api {
    OpenAi,
    Anthropic,
}

impl Api {
    fn parse(name: Option<&str>) -> Result<Self, String> {
        match name {
            None | Some("openai-completions") => Ok(Api::OpenAi),
            Some("anthropic-messages") => Ok(Api::Anthropic),
            Some(other) => Err(format!("unsupported provider api '{other}'")),
        }
    }
}

pub struct Client {
    http: reqwest::blocking::Client,
    base_url: String,
    api: Api,
    api_key: Option<String>,
    model: String,
    max_tokens_field: String,
    reasoning_effort: Option<String>,
    temperature: f64,
    top_p: Option<f64>,
    top_k: Option<u32>,
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
        let api = Api::parse(provider.api.as_deref())?;
        let api_key = provider.api_key()?;

        if matches!(api, Api::Anthropic) && api_key.is_none() {
            return Err("the anthropic provider needs an apiKey".to_string());
        }

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| format!("cannot build http client: {e}"))?;

        Ok(Self {
            http,
            base_url: provider.base_url.trim_end_matches('/').to_string(),
            api,
            api_key,
            model: model.id.clone(),
            max_tokens_field: model.compat.max_tokens_field().to_string(),
            reasoning_effort: effort.filter(|_| model.compat.supports_reasoning_effort),
            temperature: model.compat.temperature.unwrap_or(0.0),
            top_p: model.compat.top_p,
            top_k: model.compat.top_k,
        })
    }

    /// One round trip. Returns the assistant message both raw (to append to the
    /// history verbatim) and parsed (to drive the loop). The raw form is always
    /// OpenAI-shaped, whichever API answered.
    pub fn complete(&self, messages: &[Value], tools: &Value) -> Result<(Value, Assistant), String> {
        let raw = match self.api {
            Api::OpenAi => self.call_openai(messages, tools)?,
            Api::Anthropic => self.call_anthropic(messages, tools)?,
        };
        let assistant: Assistant = serde_json::from_value(raw.clone())
            .map_err(|e| format!("unexpected message shape: {e}"))?;
        Ok((raw, assistant))
    }

    fn send(&self, url: String, body: Value, anthropic: bool) -> Result<Value, String> {
        let mut req = self.http.post(&url).json(&body);
        req = if anthropic {
            req.header("x-api-key", self.api_key.clone().unwrap_or_default())
                .header("anthropic-version", ANTHROPIC_VERSION)
        } else if let Some(key) = &self.api_key {
            req.bearer_auth(key)
        } else {
            req
        };

        let resp = req.send().map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| format!("cannot read response: {e}"))?;

        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("bad JSON from provider ({status}): {e}"))?;

        // Anthropic reports failures in the body as well as the status line.
        if let Some(message) = parsed["error"]["message"].as_str() {
            return Err(format!("{status}: {message}"));
        }
        if !status.is_success() {
            return Err(format!("{status}: {text}"));
        }
        Ok(parsed)
    }

    fn call_openai(&self, messages: &[Value], tools: &Value) -> Result<Value, String> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools,
            "temperature": self.temperature,
        });
        body[&self.max_tokens_field] = json!(OPENAI_MAX_TOKENS);
        if let Some(top_p) = self.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(top_k) = self.top_k {
            body["top_k"] = json!(top_k);
        }
        if let Some(effort) = &self.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }

        let parsed = self.send(format!("{}/chat/completions", self.base_url), body, false)?;
        if parsed["choices"][0]["finish_reason"] == "length" {
            return Err(format!(
                "response cut off at the token limit ({})",
                parsed["usage"]
            ));
        }
        let mut message = parsed["choices"][0]["message"].clone();
        let Some(fields) = message.as_object_mut() else {
            return Err(format!("no message in response: {parsed}"));
        };
        // Replaying reasoning makes local models (Gemma via LM Studio) write
        // their thinking and tool calls as plain text on later turns.
        fields.remove("reasoning_content");
        fields.remove("reasoning");
        Ok(message)
    }

    fn call_anthropic(&self, messages: &[Value], tools: &Value) -> Result<Value, String> {
        let (system, converted) = to_anthropic_messages(messages);

        // No temperature: sampling parameters are rejected on current models,
        // and thinking is left at its adaptive default by omitting the field.
        let mut body = json!({
            "model": self.model,
            "max_tokens": ANTHROPIC_MAX_TOKENS,
            "messages": converted,
            "tools": to_anthropic_tools(tools),
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }
        if let Some(effort) = &self.reasoning_effort {
            body["output_config"] = json!({ "effort": effort });
        }

        let parsed = self.send(format!("{}/messages", self.base_url), body, true)?;
        if parsed["stop_reason"] == "max_tokens" {
            return Err(format!(
                "response cut off at the token limit ({})",
                parsed["usage"]
            ));
        }
        let blocks = parsed["content"]
            .as_array()
            .ok_or_else(|| format!("no content in response: {parsed}"))?;

        let mut text = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        for block in blocks {
            match block["type"].as_str().unwrap_or("") {
                "text" => text.push_str(block["text"].as_str().unwrap_or("")),
                "tool_use" => tool_calls.push(json!({
                    "id": block["id"],
                    "type": "function",
                    "function": {
                        "name": block["name"],
                        "arguments": block["input"].to_string(),
                    }
                })),
                _ => {}
            }
        }

        Ok(json!({
            "role": "assistant",
            "content": text,
            "tool_calls": tool_calls,
            // Kept so thinking blocks and their signatures can be replayed
            // verbatim on the next turn, which the API requires.
            "_anthropic_content": blocks,
        }))
    }
}

/// OpenAI-shaped history to Anthropic shape: system lifted out, tool results
/// folded into user turns, assistant turns replayed verbatim when possible.
fn to_anthropic_messages(messages: &[Value]) -> (String, Vec<Value>) {
    let mut system = String::new();
    let mut out: Vec<Value> = Vec::new();

    for message in messages {
        let content = message["content"].as_str().unwrap_or("");
        match message["role"].as_str().unwrap_or("user") {
            "system" => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(content);
            }
            "tool" => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": message["tool_call_id"],
                    "content": content,
                });
                // Results for one assistant turn must share a single user
                // message, or the model stops calling tools in parallel.
                match out.last_mut().filter(|last| is_tool_result_turn(last)) {
                    Some(last) => last["content"].as_array_mut().unwrap().push(block),
                    None => out.push(json!({ "role": "user", "content": [block] })),
                }
            }
            "assistant" => {
                if let Some(original) = message.get("_anthropic_content") {
                    out.push(json!({ "role": "assistant", "content": original }));
                    continue;
                }
                let mut blocks: Vec<Value> = Vec::new();
                if !content.is_empty() {
                    blocks.push(json!({ "type": "text", "text": content }));
                }
                if let Some(calls) = message["tool_calls"].as_array() {
                    for call in calls {
                        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call["id"],
                            "name": call["function"]["name"],
                            "input": serde_json::from_str::<Value>(arguments).unwrap_or(json!({})),
                        }));
                    }
                }
                if !blocks.is_empty() {
                    out.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            _ => out.push(json!({ "role": "user", "content": content })),
        }
    }

    (system, out)
}

fn is_tool_result_turn(message: &Value) -> bool {
    message["role"] == "user" && message["content"][0]["type"] == "tool_result"
}

/// `{type: function, function: {...}}` becomes a flat Anthropic tool.
fn to_anthropic_tools(tools: &Value) -> Value {
    let converted: Vec<Value> = tools
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|tool| {
            json!({
                "name": tool["function"]["name"],
                "description": tool["function"]["description"],
                "input_schema": tool["function"]["parameters"],
            })
        })
        .collect();
    Value::Array(converted)
}
