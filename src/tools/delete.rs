use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

pub const NAME: &str = "delete";
pub const PATH_ARGS: &[&str] = &["path"];

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

    if count > 0 && !confirm(path, count)? {
        return Err(format!("the user declined deleting {path}"));
    }

    if recursive {
        std::fs::remove_dir_all(target).map_err(|e| format!("{path}: {e}"))?;
        Ok(format!("Deleted directory {path} and its {count} entries"))
    } else {
        std::fs::remove_dir(target).map_err(|e| format!("{path}: {e}"))?;
        Ok(format!("Deleted empty directory {path}"))
    }
}

/// Ask on the terminal before a non-empty directory goes. Anything but "y" or
/// "yes", including a closed stdin, counts as no.
fn confirm(path: &str, count: usize) -> Result<bool, String> {
    eprint!("  delete {path} and its {count} entries? [y/N] ");
    std::io::stderr().flush().map_err(|e| e.to_string())?;

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}
