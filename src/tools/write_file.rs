use serde_json::{json, Value};
use std::path::Path;

pub const NAME: &str = "write_file";
pub const PATH_ARGS: &[&str] = &["path"];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Write a text file, creating it or replacing its entire contents. To change part of an existing file, prefer edit_file.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to write." },
                    "content": { "type": "string", "description": "The complete new contents of the file." }
                },
                "required": ["path", "content"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'path'")?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'content'")?;

    write_file(path, content)
}

pub fn write_file(path: &str, content: &str) -> Result<String, String> {
    let existed = Path::new(path).exists();
    std::fs::write(path, content).map_err(|e| format!("{path}: {e}"))?;

    let verb = if existed { "Overwrote" } else { "Created" };
    Ok(format!(
        "{verb} {path} ({} bytes, {} lines)",
        content.len(),
        content.lines().count()
    ))
}
