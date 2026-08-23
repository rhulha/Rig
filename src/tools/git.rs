/// Shared helpers for the git tools. gix reads the repository directly, so
/// none of these shell out to the git binary.
pub fn open() -> Result<gix::Repository, String> {
    gix::open(".").map_err(|e| format!("not a git repository: {e}"))
}

/// gix hands back raw bytes for paths and messages, which need not be UTF-8.
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
