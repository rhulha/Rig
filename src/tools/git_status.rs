use super::git;
use gix::status::index_worktree;
use gix::status::Item;
use serde_json::{json, Value};

pub const NAME: &str = "git_status";
pub const PATH_ARGS: &[&str] = &[];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Show the repository state: staged changes, changes not staged, and untracked files. Respects .gitignore.",
            "parameters": { "type": "object", "properties": {}, "required": [] }
        }
    })
}

pub fn run(_args: &Value) -> Result<String, String> {
    git_status()
}

pub fn git_status() -> Result<String, String> {
    let repo = git::open()?;

    let head_tree = repo
        .head_tree_id_or_empty()
        .map_err(|e| format!("cannot resolve HEAD tree: {e}"))?;

    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|e| format!("cannot read status: {e}"))?
        .head_tree(head_tree)
        .into_iter(None)
        .map_err(|e| format!("cannot read status: {e}"))?;

    let mut staged: Vec<String> = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    let mut untracked: Vec<String> = Vec::new();

    for item in iter {
        let item = item.map_err(|e| format!("cannot read status entry: {e}"))?;
        let path = git::text(item.location());

        match &item {
            // HEAD versus index: what a commit would record.
            Item::TreeIndex(change) => {
                staged.push(format!("  {:<10} {path}", tree_index_label(change)));
            }
            // Index versus working tree, including untracked files.
            Item::IndexWorktree(change) => match summary(change) {
                Some("untracked") => untracked.push(format!("  {path}")),
                Some(label) => changed.push(format!("  {:<10} {path}", label)),
                None => {}
            },
        }
    }

    if staged.is_empty() && changed.is_empty() && untracked.is_empty() {
        return Ok("Working tree clean".to_string());
    }

    let mut out = String::new();
    let mut section = |title: &str, rows: &[String]| {
        if !rows.is_empty() {
            out.push_str(title);
            out.push('\n');
            out.push_str(&rows.join("\n"));
            out.push('\n');
        }
    };
    section("Staged:", &staged);
    section("Not staged:", &changed);
    section("Untracked:", &untracked);
    Ok(out)
}

fn tree_index_label(change: &gix::diff::index::ChangeRef<'_, '_>) -> &'static str {
    use gix::diff::index::ChangeRef::*;
    match change {
        Addition { .. } => "new",
        Deletion { .. } => "deleted",
        Modification { .. } => "modified",
        Rewrite { .. } => "renamed",
    }
}

fn summary(change: &index_worktree::Item) -> Option<&'static str> {
    use gix::status::plumbing::index_as_worktree_with_renames::Summary::*;
    Some(match change.summary()? {
        Removed => "deleted",
        Added => "untracked",
        Modified => "modified",
        TypeChange => "typechange",
        Renamed => "renamed",
        Copied => "copied",
        IntentToAdd => return None,
        Conflict => "conflict",
    })
}
