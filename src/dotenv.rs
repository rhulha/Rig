use std::path::Path;

/// Load `KEY=value` lines into the process environment. Missing file is fine,
/// and anything already set in the real environment wins.
pub fn load(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches(['"', '\'']);
        if !key.is_empty() && std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
}
