# grok-agent

Local AI coding agent written in Rust. The agent process runs on your PC; reasoning is powered by **xAI Grok 4.5** over the Responses API.

> Grok 4.5 model weights are hosted by xAI — this project is a local agent runtime (REPL + tools + tool loop), not an offline copy of the model.

## Features

- Interactive REPL or one-shot `-p` prompts
- Agent loop with function calling against Grok 4.5
- Local tools sandboxed to a workspace:
  - `list_dir`, `read_file`, `write_file`, `edit_file`, `glob_files`, `run_command`
- Optional xAI server tools: `--web-search`, `--code-interpreter`
- Conversation continuity via `previous_response_id`

## Requirements

- Rust 1.75+ (edition 2021)
- An [xAI API key](https://console.x.ai/)

## Setup

```bash
cp .env.example .env
# edit .env and set XAI_API_KEY=...

cargo build --release
```

## Usage

```bash
# Interactive agent in the current directory
cargo run --release

# One-shot task
cargo run --release -- -p "List Rust source files and summarize what this crate does"

# Custom workspace + web search
cargo run --release -- --workspace ~/projects/my-app --web-search
```

Installed binary:

```bash
cargo install --path .
grok-agent -p "Fix the failing unit tests"
```

### REPL commands

| Command       | Action                         |
|---------------|--------------------------------|
| `/help`       | Show help                      |
| `/workspace`  | Print sandbox root            |
| `/reset`      | Clear conversation state       |
| `/quit`       | Exit                           |

## Configuration

| Flag / env            | Default                 | Description                          |
|-----------------------|-------------------------|--------------------------------------|
| `XAI_API_KEY`         | _(required)_            | xAI API key                          |
| `XAI_BASE_URL`        | `https://api.x.ai/v1`   | API base                             |
| `GROK_MODEL`          | `grok-4.5`              | Model id                             |
| `GROK_WORKSPACE`      | `.`                     | Tool sandbox root                    |
| `--max-turns`         | `24`                    | Max tool rounds per user message     |
| `--shell-timeout-secs`| `60`                    | Shell tool timeout                   |

## Safety

- File and shell tools resolve paths under `--workspace` and reject escapes.
- Shell commands still run with your user privileges inside that directory — review prompts before letting the agent execute destructive work.
- Do not commit `.env` (ignored by default).

## Project layout

```
src/
  main.rs       CLI / REPL
  config.rs     Clap + env config
  client.rs     xAI Responses API client
  agent.rs      Tool-calling agent loop
  tools/        Local filesystem + shell tools
```

## License

MIT
