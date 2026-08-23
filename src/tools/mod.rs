mod delete;
mod edit_file;
mod list_dir;
mod mkdir;
mod move_file;
mod read_file;
mod web_fetch;
mod write_file;

use serde_json::Value;

const MAX_OUTPUT: usize = 40_000;

type Schema = fn() -> Value;
type Run = fn(&Value) -> Result<String, String>;

/// Every tool the model can call. Adding one means adding a file next to this
/// one and a single row here.
const REGISTRY: &[(&str, Schema, Run)] = &[
    (list_dir::NAME, list_dir::schema, list_dir::run),
    (read_file::NAME, read_file::schema, read_file::run),
    (write_file::NAME, write_file::schema, write_file::run),
    (edit_file::NAME, edit_file::schema, edit_file::run),
    (mkdir::NAME, mkdir::schema, mkdir::run),
    (move_file::NAME, move_file::schema, move_file::run),
    (delete::NAME, delete::schema, delete::run),
    (web_fetch::NAME, web_fetch::schema, web_fetch::run),
];

pub fn schemas() -> Value {
    Value::Array(REGISTRY.iter().map(|(_, schema, _)| schema()).collect())
}

/// Run a tool by name. The returned string is fed straight back to the model,
/// so errors are returned as text rather than propagated.
pub fn dispatch(name: &str, args: &Value) -> String {
    let result = match REGISTRY.iter().find(|(tool, _, _)| *tool == name) {
        Some((_, _, run)) => run(args),
        None => Err(format!("unknown tool '{name}'")),
    };

    match result {
        Ok(mut text) => {
            if text.len() > MAX_OUTPUT {
                text.truncate(floor_char_boundary(&text, MAX_OUTPUT));
                text.push_str("\n... [output truncated]");
            }
            text
        }
        Err(e) => format!("Error: {e}"),
    }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
