use serde_json::{json, Value};

pub const NAME: &str = "read_file";

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Read a UTF-8 text file and return it with 1-based line numbers. Pass start_line/end_line to read only part of it.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File to read." },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to return, 1-based and inclusive. Defaults to 1."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to return, 1-based and inclusive. Defaults to the end of the file."
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

    read_file(
        path,
        args.get("start_line").and_then(Value::as_u64),
        args.get("end_line").and_then(Value::as_u64),
    )
}

pub fn read_file(
    path: &str,
    start_line: Option<u64>,
    end_line: Option<u64>,
) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();

    let start = start_line.unwrap_or(1).max(1) as usize;
    let end = (end_line.unwrap_or(total as u64) as usize).min(total);

    if start > total {
        return Err(format!("{path} has {total} lines, start_line {start} is past the end"));
    }
    if end < start {
        return Err(format!("end_line {end} is before start_line {start}"));
    }

    let width = end.to_string().len();
    let mut out = String::new();
    for (offset, line) in lines[start - 1..end].iter().enumerate() {
        out.push_str(&format!("{:>width$}  {line}\n", start + offset, width = width));
    }
    if start > 1 || end < total {
        out.push_str(&format!("[lines {start}-{end} of {total}]\n"));
    }
    Ok(out)
}
