mod chat;
mod config;
mod dotenv;
mod server;
mod tools;

use config::Config;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
rig - a minimal agent harness

Usage:
  rig [options] [prompt]

With no prompt, rig starts an interactive shell that keeps the conversation
history between turns. Leave it with 'exit', 'quit' or Ctrl+D.

Options:
  --config <path>     models.json to load (default: ./models.json)
  --provider <name>   provider from models.json (default: first with models)
  --model <id>        model id (default: first model of the provider)
  --effort <level>    reasoning effort, if the model declares support
  --max-steps <n>     tool-call rounds before giving up (default: 12)
  --prompt-file <path>
                      read the prompt from a file; a prompt given on the
                      command line is appended to it
  --serve             also serve the working directory read-only on
                      http://127.0.0.1:8888
  --list              print providers and models, then exit
  -h, --help          this text
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    config: PathBuf,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    max_steps: usize,
    list: bool,
    serve: bool,
    prompt: String,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        config: PathBuf::from("models.json"),
        provider: None,
        model: None,
        effort: None,
        max_steps: 12,
        list: false,
        serve: false,
        prompt: String::new(),
    };
    let mut rest: Vec<String> = Vec::new();
    let mut prompt_file: Option<PathBuf> = None;
    let mut it = std::env::args().skip(1);

    while let Some(arg) = it.next() {
        let mut value = |flag: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--list" => args.list = true,
            "--serve" => args.serve = true,
            "--config" => args.config = PathBuf::from(value("--config")?),
            "--provider" => args.provider = Some(value("--provider")?),
            "--model" => args.model = Some(value("--model")?),
            "--effort" => args.effort = Some(value("--effort")?),
            "--prompt-file" => prompt_file = Some(PathBuf::from(value("--prompt-file")?)),
            "--max-steps" => {
                args.max_steps = value("--max-steps")?
                    .parse()
                    .map_err(|_| "--max-steps needs a number".to_string())?
            }
            other if other.starts_with("--") => return Err(format!("unknown option '{other}'")),
            other => rest.push(other.to_string()),
        }
    }

    args.prompt = rest.join(" ");
    if let Some(path) = prompt_file {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        args.prompt = if args.prompt.is_empty() {
            text
        } else {
            format!("{}

{}", text.trim_end(), args.prompt)
        };
    }
    Ok(Some(args))
}

fn run() -> Result<(), String> {
    let Some(args) = parse_args()? else {
        print!("{USAGE}");
        return Ok(());
    };

    dotenv::load(std::path::Path::new(".env"));

    let config = Config::load(&args.config)?;

    if args.list {
        print!("{}", config.describe());
        return Ok(());
    }
    let (provider_name, provider, model) =
        config.resolve(args.provider.as_deref(), args.model.as_deref())?;
    eprintln!("[{provider_name}/{}]", model.id);

    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;

    if args.serve {
        server::spawn()?;
        eprintln!("serving {} on http://127.0.0.1:{}", cwd.display(), server::PORT);
    }

    let client = chat::Client::new(provider, model, args.effort)?;
    let tool_schemas = tools::schemas();

    let mut messages = vec![json!({
        "role": "system",
        "content": format!(
            "You are a command-line assistant with access to the local filesystem              through the provided tools. The working directory is {}.              Use the tools to check facts instead of guessing, and read a file before              editing it so your edits match it exactly. Answer concisely.",
            cwd.display()
        ),
    })];

    if args.prompt.is_empty() {
        return interactive(&client, &mut messages, &tool_schemas, args.max_steps);
    }

    messages.push(json!({ "role": "user", "content": args.prompt }));
    println!("{}", turn(&client, &mut messages, &tool_schemas, args.max_steps)?);
    Ok(())
}

fn interactive(
    client: &chat::Client,
    messages: &mut Vec<Value>,
    tool_schemas: &Value,
    max_steps: usize,
) -> Result<(), String> {
    eprintln!("Type 'exit' or Ctrl+D to quit.");
    let stdin = std::io::stdin();
    let mut line = String::new();

    loop {
        eprint!("
> ");
        std::io::stderr().flush().map_err(|e| e.to_string())?;

        line.clear();
        let read = stdin.read_line(&mut line).map_err(|e| e.to_string())?;
        if read == 0 {
            eprintln!();
            return Ok(());
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "exit" | "quit") {
            return Ok(());
        }

        // A failed turn keeps the shell alive; rewinding to the checkpoint drops
        // its half-finished exchange so the transcript stays consistent.
        let checkpoint = messages.len();
        messages.push(json!({ "role": "user", "content": input }));
        match turn(client, messages, tool_schemas, max_steps) {
            Ok(answer) => println!("{answer}"),
            Err(e) => {
                messages.truncate(checkpoint);
                eprintln!("error: {e}");
            }
        }
    }
}

/// Run tool-call rounds until the model answers, then return that answer.
fn turn(
    client: &chat::Client,
    messages: &mut Vec<Value>,
    tool_schemas: &Value,
    max_steps: usize,
) -> Result<String, String> {
    for _ in 0..max_steps {
        let (raw, assistant) = client.complete(messages, tool_schemas)?;
        messages.push(raw);

        if assistant.tool_calls.is_empty() {
            return Ok(assistant.content.unwrap_or_default().trim().to_string());
        }

        for call in &assistant.tool_calls {
            let args = call.args();
            eprintln!("  -> {}({})", call.function.name, compact(&args));
            let result = tools::dispatch(&call.function.name, &args);
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call.id,
                "content": result,
            }));
        }
    }

    Err(format!("gave up after {max_steps} tool rounds"))
}

/// One-line summary of a tool call's arguments. Values are clipped so that a
/// whole file body passed to write_file does not flood the terminal.
fn compact(args: &Value) -> String {
    args.as_object()
        .map(|map| {
            map.iter()
                .map(|(k, v)| format!("{k}={}", clip(v)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn clip(value: &Value) -> String {
    const MAX: usize = 60;

    // Rendering as JSON keeps newlines escaped, so the trace stays on one line.
    let text = value.to_string();
    let length = text.chars().count();
    if length <= MAX {
        return text;
    }
    let head: String = text.chars().take(MAX).collect();
    format!("{head}... +{} chars", length - MAX)
}
