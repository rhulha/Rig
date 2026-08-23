use super::git;
use gix::status::index_worktree;
use gix::status::Item;
use serde_json::{json, Value};

pub const NAME: &str = "git_diff";
pub const PATH_ARGS: &[&str] = &["path"];

const MAX_DIFF: usize = 30_000;

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Show a unified diff of unstaged changes in the working tree, compared to the index.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Limit the diff to this file. Defaults to every changed file."
                    }
                },
                "required": []
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    git_diff(args.get("path").and_then(Value::as_str))
}

pub fn git_diff(only: Option<&str>) -> Result<String, String> {
    let repo = git::open()?;

    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|e| format!("cannot read status: {e}"))?
        .into_iter(None)
        .map_err(|e| format!("cannot read status: {e}"))?;

    let wanted = only.map(|p| p.replace('\\', "/"));
    let mut out = String::new();
    let mut files = 0;

    for item in iter {
        let item = item.map_err(|e| format!("cannot read status entry: {e}"))?;
        let Item::IndexWorktree(index_worktree::Item::Modification { entry, rela_path, .. }) = &item
        else {
            continue;
        };

        let path = git::text(rela_path);
        if let Some(wanted) = &wanted {
            if &path != wanted {
                continue;
            }
        }

        let old = repo
            .find_object(entry.id)
            .map(|object| git::text(&object.data))
            .unwrap_or_default();
        let new = std::fs::read(&path).map(|b| git::text(&b)).unwrap_or_default();
        if old == new {
            continue;
        }

        out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
        out.push_str(&unified(&old, &new));
        files += 1;

        if out.len() > MAX_DIFF {
            out.push_str("... [diff truncated]\n");
            break;
        }
    }

    if files == 0 {
        return Ok(match only {
            Some(path) => format!("No unstaged changes in {path}"),
            None => "No unstaged changes".to_string(),
        });
    }
    Ok(out)
}

fn unified(old: &str, new: &str) -> String {
    use gix::diff::blob::unified_diff::{ConsumeBinaryHunk, ContextSize};
    use gix::diff::blob::{Algorithm, Diff, InternedInput, UnifiedDiff};

    let input = InternedInput::new(old.as_bytes(), new.as_bytes());
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    UnifiedDiff::new(
        &diff,
        &input,
        ConsumeBinaryHunk::new(String::new(), "\n"),
        ContextSize::default(),
    )
    .consume()
    .unwrap_or_else(|e| format!("[diff failed: {e}]"))
}
