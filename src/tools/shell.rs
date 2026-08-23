use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::Command;

pub const NAME: &str = "shell";
/// The command string is not a path, and the sandbox cannot meaningfully
/// constrain a shell anyway — the confirmation prompt is the gate here.
pub const PATH_ARGS: &[&str] = &[];

pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Run a shell command in the working directory and return its output. Every call must be confirmed by the user, so prefer the dedicated file tools for reading, writing and searching, and use this for things they cannot do.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command line to run." },
                    "purpose": {
                        "type": "string",
                        "description": "One short line telling the user why this command is needed. Shown in the confirmation prompt."
                    }
                },
                "required": ["command"]
            }
        }
    })
}

pub fn run(args: &Value) -> Result<String, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or("missing required argument 'command'")?;
    let purpose = args.get("purpose").and_then(Value::as_str);

    if !confirm(command, purpose)? {
        return Err("the user declined to run this command".to_string());
    }

    shell(command)
}

/// Ask on the terminal before running anything. Anything other than an explicit
/// yes — including end of input — counts as a refusal.
fn confirm(command: &str, purpose: Option<&str>) -> Result<bool, String> {
    let mut err = std::io::stderr();
    writeln!(err).ok();
    if let Some(purpose) = purpose {
        writeln!(err, "  {purpose}").ok();
    }
    writeln!(err, "  $ {command}").ok();
    write!(err, "  run this? [y/N] ").ok();
    err.flush().ok();

    let mut answer = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| format!("cannot read confirmation: {e}"))?;

    if read == 0 {
        writeln!(err, "\n  no input, treating as no").ok();
        return Ok(false);
    }

    let yes = matches!(answer.trim().to_lowercase().as_str(), "y" | "yes");
    writeln!(err, "  {}", if yes { "running" } else { "skipped" }).ok();
    Ok(yes)
}

fn shell(command: &str) -> Result<String, String> {
    let output = build(command)
        .output()
        .map_err(|e| format!("cannot start shell: {e}"))?;

    let mut out = String::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.trim().is_empty() {
        out.push_str(stdout.trim_end());
        out.push('\n');
    }
    if !stderr.trim().is_empty() {
        out.push_str("[stderr]\n");
        out.push_str(stderr.trim_end());
        out.push('\n');
    }
    match output.status.code() {
        Some(0) => {}
        Some(code) => out.push_str(&format!("[exit code {code}]\n")),
        None => out.push_str("[terminated by signal]\n"),
    }
    if out.is_empty() {
        out.push_str("[no output]\n");
    }
    Ok(out)
}

#[cfg(windows)]
fn build(command: &str) -> Command {
    // raw_arg passes the line to cmd.exe verbatim; the normal argument escaping
    // follows CRT rules that cmd does not use, which mangles quotes and pipes.
    use std::os::windows::process::CommandExt;
    let mut shell = Command::new("cmd");
    shell.raw_arg(format!("/C {command}"));
    shell
}

#[cfg(not(windows))]
fn build(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    shell
}
