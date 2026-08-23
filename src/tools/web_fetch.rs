use serde_json::{json, Value};
use std::time::Duration;

pub const NAME: &str = "web_fetch";

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Fetch a URL over HTTP(S) and return its contents. HTML pages are reduced to readable text; other text formats are returned as-is.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Absolute http:// or https:// URL." }
                },
                "required": ["url"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'url'")?;

    web_fetch(url)
}

pub fn web_fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("{url} is not an http:// or https:// URL"));
    }

    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("rig/0.1")
        .build()
        .map_err(|e| format!("cannot build http client: {e}"))?;

    let resp = http.get(url).send().map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let body = resp.text().map_err(|e| format!("{url}: {e}"))?;
    if !status.is_success() {
        return Err(format!("{url}: {status}"));
    }

    let text = if content_type.contains("html") {
        html_to_text(&body)
    } else {
        body
    };

    Ok(format!("{url} ({status}):\n\n{}", text.trim()))
}

/// Crude HTML to text: enough for a model to read a page, not a parser.
fn html_to_text(html: &str) -> String {
    let stripped = drop_block(&drop_block(html, "script"), "style");

    let mut out = String::with_capacity(stripped.len() / 2);
    let mut tag = String::new();
    let mut in_tag = false;

    for c in stripped.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                out.push(if breaks_line(&tag) { '\n' } else { ' ' });
            }
            _ if in_tag => {
                if tag.len() < 12 {
                    tag.push(c.to_ascii_lowercase());
                }
            }
            _ => out.push(c),
        }
    }

    collapse(&decode_entities(&out))
}

/// Remove `<name ...> ... </name>` blocks, case-insensitively.
fn drop_block(html: &str, name: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{name}");
    let close = format!("</{name}>");

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find(&open).map(|i| cursor + i) {
        out.push_str(&html[cursor..start]);
        match lower[start..].find(&close) {
            Some(len) => cursor = start + len + close.len(),
            None => return out,
        }
    }
    out.push_str(&html[cursor..]);
    out
}

fn breaks_line(tag: &str) -> bool {
    let name = tag.trim_start_matches('/');
    matches!(
        name.split([' ', '/']).next().unwrap_or(""),
        "br" | "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "section" | "article" | "header" | "footer" | "nav" | "table" | "ul" | "ol"
    )
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Squeeze runs of spaces and blank lines left behind by tag removal.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;

    for line in s.lines() {
        let trimmed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(&trimmed);
        out.push('\n');
    }
    out
}
