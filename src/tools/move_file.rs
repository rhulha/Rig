use serde_json::{json, Value};
use std::path::Path;

// The wire name is "move"; the module cannot be, because `move` is a keyword.
pub const NAME: &str = "move";

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Move or rename a file or directory. Fails if the destination already exists.",
            "parameters": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Existing file or directory." },
                    "to": { "type": "string", "description": "New path. Its parent directory must already exist." }
                },
                "required": ["from", "to"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let from = args
        .get("from")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'from'")?;
    let to = args
        .get("to")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'to'")?;

    move_path(from, to)
}

pub fn move_path(from: &str, to: &str) -> Result<String, String> {
    if !Path::new(from).exists() {
        return Err(format!("{from} does not exist"));
    }
    // Unix rename silently overwrites, Windows errors. Refuse either way.
    if Path::new(to).exists() {
        return Err(format!("{to} already exists"));
    }
    std::fs::rename(from, to).map_err(|e| format!("{from} -> {to}: {e}"))?;
    Ok(format!("Moved {from} to {to}"))
}
