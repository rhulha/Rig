use serde_json::{json, Value};
use std::time::Duration;

pub const NAME: &str = "web_search";
pub const PATH_ARGS: &[&str] = &[];

const KEY_VAR: &str = "SERP_API_KEY";
const DEFAULT_RESULTS: u64 = 5;
const MAX_RESULTS: u64 = 20;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Search the web through SerpAPI and return result titles, links and snippets. Follow a link with web_fetch to read the page itself.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to search for." },
                    "num": {
                        "type": "integer",
                        "description": "How many results to return, 1 to 20. Defaults to 5."
                    }
                },
                "required": ["query"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'query'")?;
    let num = args
        .get("num")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_RESULTS)
        .clamp(1, MAX_RESULTS);

    web_search(query, num)
}

pub fn web_search(query: &str, num: u64) -> Result<String, String> {
    if query.trim().is_empty() {
        return Err("query is empty".to_string());
    }

    let api_key = std::env::var(KEY_VAR)
        .map_err(|_| format!("{KEY_VAR} is not set; put it in .env or the environment"))?;

    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("cannot build http client: {e}"))?;

    let resp = http
        .get("https://serpapi.com/search.json")
        .query(&[
            ("engine", "google"),
            ("q", query),
            ("num", &num.to_string()),
            ("api_key", &api_key),
        ])
        .send()
        .map_err(|e| format!("serpapi request failed: {e}"))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .map_err(|e| format!("serpapi returned {status} with unreadable JSON: {e}"))?;

    // SerpAPI reports quota and query problems in the body, not the status code.
    if let Some(error) = body.get("error").and_then(Value::as_str) {
        return Err(format!("serpapi: {error}"));
    }
    if !status.is_success() {
        return Err(format!("serpapi: {status}"));
    }

    Ok(format_results(query, &body, num as usize))
}

fn format_results(query: &str, body: &Value, num: usize) -> String {
    let mut out = String::new();

    if let Some(answer) = body
        .get("answer_box")
        .and_then(|b| b.get("answer").or_else(|| b.get("snippet")))
        .and_then(Value::as_str)
    {
        out.push_str(&format!("Answer box: {answer}\n\n"));
    }

    let results = body
        .get("organic_results")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    if results.is_empty() && out.is_empty() {
        return format!("No results for '{query}'");
    }

    out.push_str(&format!("Results for '{query}':\n"));
    for (i, result) in results.iter().take(num).enumerate() {
        let field = |name: &str| {
            result
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        out.push_str(&format!("\n{}. {}\n", i + 1, field("title")));
        out.push_str(&format!("   {}\n", field("link")));
        let snippet = field("snippet");
        if !snippet.is_empty() {
            out.push_str(&format!("   {snippet}\n"));
        }
    }
    out
}
