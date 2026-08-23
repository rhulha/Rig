use serde_json::{json, Value};
use std::path::Path;

pub const NAME: &str = "delete";

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Delete a file, or an empty directory. Deleting a directory that still has contents requires recursive: true.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or directory to delete." },
                    "recursive": {
                        "type": "boolean",
                        "description": "Allow deleting a non-empty directory and everything inside it. Defaults to false."
                    }
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
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    delete(path, recursive)
}

pub fn delete(path: &str, recursive: bool) -> Result<String, String> {
    let target = Path::new(path);
    if !target.exists() {
        return Err(format!("{path} does not exist"));
    }

    if target.is_file() {
        std::fs::remove_file(target).map_err(|e| format!("{path}: {e}"))?;
        return Ok(format!("Deleted file {path}"));
    }

    let count = std::fs::read_dir(target)
        .map_err(|e| format!("{path}: {e}"))?
        .count();

    if count > 0 && !recursive {
        return Err(format!(
            "{path} is not empty ({count} entries); set recursive to delete it and its contents"
        ));
    }

    if recursive {
        std::fs::remove_dir_all(target).map_err(|e| format!("{path}: {e}"))?;
        Ok(format!("Deleted directory {path} and its {count} entries"))
    } else {
        std::fs::remove_dir(target).map_err(|e| format!("{path}: {e}"))?;
        Ok(format!("Deleted empty directory {path}"))
    }
}
