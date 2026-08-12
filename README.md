# grok-agent

Local AI coding agent written in Rust, with a **desktop GUI** for chatting with the agent. Reasoning is powered by **xAI Grok 4.5** over the Responses API.

> Grok 4.5 model weights are hosted by xAI — this project is a local agent runtime (GUI + tools + tool loop), not an offline copy of the model.

## Download / run

### Build a downloadable package

```bash
./scripts/package.sh
```

Creates:

```
dist/grok-agent-<version>-<target>.tar.gz
```

Extract and launch:

```bash
tar -xzf dist/grok-agent-*.tar.gz
cd grok-agent-*
# Ubuntu/Debian GUI deps (once):
sudo apt install libxkbcommon-x11-0 libxcb-xkb1
./grok-agent
```

Then open **Settings**, paste your [xAI API key](https://console.x.ai/), and chat.

Cross-platform note: build on each OS you want to ship (`cargo build --release` / `./scripts/package.sh`). The checked-in CI artifact from this repo is a Linux x86_64 tarball.

### From source

```bash
cp .env.example .env   # optional; GUI can store the key in Settings
cargo run --release
```

## Modes

| Mode | Command |
|------|---------|
| GUI (default) | `grok-agent` |
| Terminal REPL | `grok-agent --cli` |
| One-shot | `grok-agent -p "Summarize this repo"` |

## Features

- Native desktop chat UI (egui/eframe)
- Agent loop with function calling against Grok 4.5
- Local tools sandboxed to a workspace:
  - `list_dir`, `read_file`, `write_file`, `edit_file`, `glob_files`, `run_command`
- Live tool activity in the conversation
- Settings panel (API key, model, workspace, web search / code interpreter)
- Persists settings to `~/.grok-agent/settings.json`
- Optional xAI server tools: `--web-search`, `--code-interpreter`

## Configuration

| Flag / env | Default | Description |
|------------|---------|-------------|
| `XAI_API_KEY` | _(required to chat)_ | xAI API key |
| `XAI_BASE_URL` | `https://api.x.ai/v1` | API base |
| `GROK_MODEL` | `grok-4.5` | Model id |
| `GROK_WORKSPACE` | `.` | Tool sandbox root |
| `--cli` | off | Force terminal mode |
| `--max-turns` | `24` | Max tool rounds per message |

## Safety

- File and shell tools resolve paths under the workspace and reject escapes.
- Shell commands still run with your user privileges inside that directory.
- Do not commit `.env` or share `~/.grok-agent/settings.json`.

## Project layout

```
src/
  main.rs       Entry (GUI by default)
  gui.rs        Desktop chat interface
  cli.rs        Terminal REPL
  config.rs     Clap + saved settings
  client.rs     xAI Responses API client
  agent.rs      Tool-calling agent loop
  tools/        Local filesystem + shell tools
scripts/package.sh   Build downloadable tarball
```

## License

MIT
