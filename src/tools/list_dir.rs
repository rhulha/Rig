use serde_json::{json, Value};

pub const NAME: &str = "list_directory";
pub const PATH_ARGS: &[&str] = &["path"];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "List the entries of a directory. Directories are marked [dir]; files show their size in bytes.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to list. Relative paths resolve against the working directory. Defaults to '.'."
                    }
                },
                "required": []
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    list_directory(args.get("path").and_then(Value::as_str).unwrap_or("."))
}

pub fn list_directory(path: &str) -> Result<String, String> {
    let entries = std::fs::read_dir(path).map_err(|e| format!("{path}: {e}"))?;

    let mut rows: Vec<(bool, String, u64)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{path}: {e}"))?;
        let meta = entry.metadata().map_err(|e| format!("{path}: {e}"))?;
        rows.push((
            meta.is_dir(),
            entry.file_name().to_string_lossy().into_owned(),
            meta.len(),
        ));
    }

    if rows.is_empty() {
        return Ok(format!("{path} is empty"));
    }

    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

    let mut out = format!("{path}:\n");
    for (is_dir, name, len) in rows {
        if is_dir {
            out.push_str(&format!("  [dir] {name}\n"));
        } else {
            out.push_str(&format!("        {name}  ({len} bytes)\n"));
        }
    }
    Ok(out)
}
