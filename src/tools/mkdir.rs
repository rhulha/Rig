use serde_json::{json, Value};
use std::path::Path;

pub const NAME: &str = "mkdir";

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Create a directory, including any missing parent directories. Succeeds quietly if it already exists.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to create." }
                },
                "required": ["path"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'path'")?;

    mkdir(path)
}

pub fn mkdir(path: &str) -> Result<String, String> {
    if Path::new(path).is_dir() {
        return Ok(format!("{path} already exists"));
    }
    std::fs::create_dir_all(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(format!("Created {path}"))
}
