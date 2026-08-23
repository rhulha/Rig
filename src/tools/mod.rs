mod delete;
mod edit_file;
mod git;
mod git_diff;
mod git_log;
mod git_status;
mod grep;
mod list_dir;
mod mkdir;
mod move_file;
mod read_file;
pub mod sandbox;
mod web_fetch;
mod web_search;
mod write_file;

use serde_json::Value;

const MAX_OUTPUT: usize = 40_000;

type Schema = fn() -> Value;
type Run = fn(&Value) -> Result<String, String>;

struct Tool {
    name: &'static str,
    schema: Schema,
    run: Run,
    /// Arguments naming a filesystem path, checked against the sandbox before
    /// the tool runs. Empty for tools that touch no files.
    path_args: &'static [&'static str],
}

macro_rules! tool {
    ($m:ident) => {
        Tool {
            name: $m::NAME,
            schema: $m::schema,
            run: $m::run,
            path_args: $m::PATH_ARGS,
        }
    };
}

/// Every tool the model can call. Adding one means adding a file next to this
/// one and a single row here.
const REGISTRY: &[Tool] = &[
    tool!(list_dir),
    tool!(read_file),
    tool!(write_file),
    tool!(edit_file),
    tool!(mkdir),
    tool!(move_file),
    tool!(delete),
    tool!(grep),
    tool!(git_status),
    tool!(git_diff),
    tool!(git_log),
    tool!(web_fetch),
    tool!(web_search),
];

pub fn schemas() -> Value {
    Value::Array(REGISTRY.iter().map(|tool| (tool.schema)()).collect())
}

/// Run a tool by name. The returned string is fed straight back to the model,
/// so errors are returned as text rather than propagated.
pub fn dispatch(name: &str, args: &Value) -> String {
    let result = match REGISTRY.iter().find(|tool| tool.name == name) {
        Some(tool) => confine_paths(args, tool.path_args).and_then(|()| (tool.run)(args)),
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

/// Every path a tool is about to touch must sit inside the working directory.
fn confine_paths(args: &Value, path_args: &[&str]) -> Result<(), String> {
    for key in path_args {
        if let Some(path) = args.get(*key).and_then(Value::as_str) {
            sandbox::confine(path)?;
        }
    }
    Ok(())
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
