use serde_json::{json, Value};

pub const NAME: &str = "edit_file";
pub const PATH_ARGS: &[&str] = &["path"];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Replace an exact string in a file. old_string must match the file byte for byte, including indentation, and must be unique unless replace_all is true. Read the file first so the match is exact.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to edit." },
                    "old_string": { "type": "string", "description": "Exact text to replace, with enough surrounding context to be unique." },
                    "new_string": { "type": "string", "description": "Text to put in its place. Use an empty string to delete." },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace every occurrence instead of requiring a unique match. Defaults to false."
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'path'")?;
    let old_string = args
        .get("old_string")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'old_string'")?;
    let new_string = args
        .get("new_string")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'new_string'")?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    edit_file(path, old_string, new_string, replace_all)
}

pub fn edit_file(
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String, String> {
    if old_string.is_empty() {
        return Err("old_string is empty; use write_file to create a file".to_string());
    }
    if old_string == new_string {
        return Err("old_string and new_string are identical".to_string());
    }

    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;

    // read_file shows lines without their `\r` and with number prefixes; models
    // copy both habits into their edits, so undo them before matching.
    let crlf = text.contains("\r\n");
    let mut old = fit_line_endings(old_string, crlf);
    let mut new = fit_line_endings(new_string, crlf);
    if !text.contains(&old) {
        if let Some(stripped) = strip_line_numbers(&old) {
            old = stripped;
            new = strip_line_numbers(&new).unwrap_or(new);
        }
    }
    let (old_string, new_string) = (old.as_str(), new.as_str());

    let hits = text.matches(old_string).count();

    match hits {
        0 => return Err(format!("old_string not found in {path}")),
        n if n > 1 && !replace_all => {
            return Err(format!(
                "old_string matches {n} times in {path}; add surrounding context to make it unique, or set replace_all"
            ))
        }
        _ => {}
    }

    let line = text
        .find(old_string)
        .map(|i| text[..i].matches('\n').count() + 1)
        .unwrap_or(1);

    let updated = if replace_all {
        text.replace(old_string, new_string)
    } else {
        text.replacen(old_string, new_string, 1)
    };
    std::fs::write(path, &updated).map_err(|e| format!("{path}: {e}"))?;

    let replaced = if replace_all { hits } else { 1 };
    let plural = if replaced == 1 { "" } else { "s" };
    Ok(format!(
        "Edited {path}: {replaced} replacement{plural}, first at line {line}"
    ))
}

fn fit_line_endings(s: &str, crlf: bool) -> String {
    let lf = s.replace("\r\n", "\n");
    if crlf {
        lf.replace('\n', "\r\n")
    } else {
        lf
    }
}

// line numbers so the model can reference locations and request specific start_line/end_line ranges.
// The downside is that models sometimes copy those prefixes into edits.
// the numbers are useful for navigation but unnecessary for applying edits.

/// Remove the `  42  ` prefixes read_file puts in front of each line, but only
/// when every line carries one, so ordinary text starting with digits survives.
fn strip_line_numbers(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    for piece in s.split_inclusive('\n') {
        let rest = piece.trim_start_matches(' ');
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            return None;
        }
        let body = &rest[digits..];
        // An empty line's trailing separator is often trimmed away.
        if body.trim_end_matches(['\r', '\n']).is_empty() {
            out.push_str(body);
        } else {
            out.push_str(body.strip_prefix("  ")?);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_line_numbers() {
        let dir = std::env::temp_dir().join("rig_edit_test.txt");
        let path = dir.to_str().unwrap();
        std::fs::write(path, "a\r\n\tlast line\r\n\r\n").unwrap();

        edit_file(path, "198  \tlast line\n199\n", "198  \tlast line\n199  \n200  \tnew\n", false).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "a\r\n\tlast line\r\n\r\n\tnew\r\n");

        edit_file(path, "a\n\tlast", "b\n\tlast", false).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "b\r\n\tlast line\r\n\r\n\tnew\r\n");
        std::fs::remove_file(path).unwrap();
    }
}
