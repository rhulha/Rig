use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Reject any path that resolves outside the working directory.
///
/// The path need not exist yet: the deepest existing ancestor is canonicalized
/// (which is what defeats symlinks) and the missing tail is appended to it.
pub fn confine(path: &str) -> Result<PathBuf, String> {
    let root = std::env::current_dir()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("cannot resolve the working directory: {e}"))?;

    let raw = Path::new(path);
    let absolute = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        root.join(raw)
    };

    let resolved = resolve_existing(&lexical_normalize(&absolute));
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(format!(
            "{path} resolves outside the working directory ({}) and is off limits",
            display(&root)
        ))
    }
}

/// Fold away `.` and `..` textually, so a non-existent tail cannot escape.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Canonicalize as much of the path as exists, keeping the rest verbatim.
fn resolve_existing(path: &Path) -> PathBuf {
    let mut tail: Vec<OsString> = Vec::new();
    let mut cursor = path.to_path_buf();

    loop {
        if let Ok(real) = cursor.canonicalize() {
            let mut out = real;
            out.extend(tail.iter().rev());
            return out;
        }
        match cursor.file_name() {
            Some(name) => {
                tail.push(name.to_os_string());
                if !cursor.pop() {
                    return path.to_path_buf();
                }
            }
            None => return path.to_path_buf(),
        }
    }
}

/// Canonical paths on Windows carry a `\?\` prefix that only confuses a model.
fn display(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}
