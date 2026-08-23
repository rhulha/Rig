use super::git;
use serde_json::{json, Value};

pub const NAME: &str = "git_log";
pub const PATH_ARGS: &[&str] = &[];

const DEFAULT_COUNT: u64 = 10;
const MAX_COUNT: u64 = 100;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Show recent commits on the current branch: short hash, author, date and subject line.",
            "parameters": {
                "type": "object",
                "properties": {
                    "count": {
                        "type": "integer",
                        "description": "How many commits to show, 1 to 100. Defaults to 10."
                    }
                },
                "required": []
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let count = args
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_COUNT)
        .clamp(1, MAX_COUNT);

    git_log(count as usize)
}

pub fn git_log(count: usize) -> Result<String, String> {
    let repo = git::open()?;
    let head = repo
        .head_id()
        .map_err(|e| format!("cannot resolve HEAD (no commits yet?): {e}"))?;

    let walk = repo
        .rev_walk([head])
        .all()
        .map_err(|e| format!("cannot walk history: {e}"))?;

    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| git::text(name.shorten()))
        .unwrap_or_else(|| "detached HEAD".to_string());

    let mut out = format!("On {branch}:\n");
    let mut shown = 0;

    for info in walk.take(count) {
        let info = info.map_err(|e| format!("cannot read commit: {e}"))?;
        let commit = info
            .object()
            .map_err(|e| format!("cannot load commit: {e}"))?;

        let short = commit
            .short_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| info.id.to_string());
        let author = commit
            .author()
            .map(|a| git::text(a.name))
            .unwrap_or_else(|_| "unknown".to_string());
        let date = commit
            .time()
            .ok()
            .and_then(|t| t.format(gix::date::time::format::SHORT).ok())
            .unwrap_or_else(|| "unknown date".to_string());
        let subject = commit
            .message()
            .map(|m| git::text(m.summary().as_ref()))
            .unwrap_or_else(|_| "(unreadable message)".to_string());

        out.push_str(&format!("{short}  {date}  {author}  {subject}\n"));
        shown += 1;
    }

    if shown == 0 {
        return Ok("No commits yet".to_string());
    }
    Ok(out)
}
