use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const NAME: &str = "grep";
pub const PATH_ARGS: &[&str] = &["path"];

const MAX_MATCHES: usize = 200;
const MAX_LINE: usize = 300;
/// Directories that would drown the results in generated files.
const SKIP_DIRS: &[&str] = &["target", "node_modules"];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Search files for a literal substring and return the matching lines as path:line: text. Plain text matching, not a regular expression. Searches recursively when given a directory.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Literal text to look for." },
                    "path": { "type": "string", "description": "File or directory to search. Defaults to '.'." },
                    "ignore_case": { "type": "boolean", "description": "Match without regard to case. Defaults to false." }
                },
                "required": ["pattern"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let pattern = args
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'pattern'")?;
    let path = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let ignore_case = args
        .get("ignore_case")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    grep(pattern, path, ignore_case)
}

pub fn grep(pattern: &str, path: &str, ignore_case: bool) -> Result<String, String> {
    if pattern.is_empty() {
        return Err("pattern is empty".to_string());
    }

    let root = Path::new(path);
    if !root.exists() {
        return Err(format!("{path} does not exist"));
    }

    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern.to_string()
    };

    let mut hits: Vec<String> = Vec::new();
    let mut files = 0;
    let mut truncated = false;

    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        if current.is_dir() {
            let entries = std::fs::read_dir(&current).map_err(|e| format!("{}: {e}", current.display()))?;
            for entry in entries.flatten() {
                let child = entry.path();
                if child.is_dir() && skipped(&child) {
                    continue;
                }
                stack.push(child);
            }
            continue;
        }

        // Anything that is not valid UTF-8 is treated as binary and passed over.
        let Ok(text) = std::fs::read_to_string(&current) else {
            continue;
        };
        files += 1;

        for (number, line) in text.lines().enumerate() {
            let haystack = if ignore_case {
                line.to_lowercase()
            } else {
                line.to_string()
            };
            if !haystack.contains(&needle) {
                continue;
            }
            if hits.len() == MAX_MATCHES {
                truncated = true;
                break;
            }
            hits.push(format!(
                "{}:{}: {}",
                current.display(),
                number + 1,
                clip(line.trim_end())
            ));
        }

        if truncated {
            break;
        }
    }

    if hits.is_empty() {
        return Ok(format!("No match for '{pattern}' in {path} ({files} files searched)"));
    }

    hits.sort();
    let mut out = format!("{} matches in {path} ({files} files searched):\n", hits.len());
    for hit in hits {
        out.push_str(&hit);
        out.push('\n');
    }
    if truncated {
        out.push_str(&format!("[stopped at {MAX_MATCHES} matches]\n"));
    }
    Ok(out)
}

fn skipped(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.starts_with('.') || SKIP_DIRS.contains(&name))
        .unwrap_or(false)
}

fn clip(line: &str) -> String {
    if line.chars().count() <= MAX_LINE {
        return line.to_string();
    }
    let cut: String = line.chars().take(MAX_LINE).collect();
    format!("{cut} ...")
}
