# Rig

Rig is a minimal Rust command-line harness for running tool-capable language models.

## Key features

- Interactive chat with conversation history, plus one-shot prompts.
- OpenAI-compatible Chat Completions and Anthropic Messages API support.
- Configurable providers, models, base URLs, and reasoning effort.
- API keys loaded from environment variables or a local `.env` file.
- Built-in tools for reading, searching, editing, moving, and deleting files.
- Web search and readable webpage fetching.
- Filesystem operations confined to the current working directory.
- Configurable limit on agent tool-call rounds.
- built-in HTTP web server running on port 8888 (start it with --serve)
- built-in git support


