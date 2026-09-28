# nxm-tui

Terminal client for LLM chat. Connects to any OpenAI-compatible server
(Nexum Inferentia, Ollama, LM Studio, NVIDIA). Built with ratatui + crossterm.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh
```

This downloads the right prebuilt binary for your platform (Apple Silicon macOS
or Linux x86_64) from the latest release and installs it to `~/.local/bin`.

Prefer to inspect before running? Download the script, read it, then execute:

```bash
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh -o install.sh
less install.sh
sh install.sh
```

Install options (environment variables):

- `NXM_INSTALL_DIR` — install directory (default `~/.local/bin`)
- `NXM_VERSION` — release tag to install (default `latest`)

### Build from source

```bash
git clone https://github.com/dangranaz/nxm-tui.git
cd nxm-tui
cargo build --release
./target/release/nxm-tui
```

## Providers

On first run, if no local server is detected, `nxm-tui` shows a menu of
providers. You can also manage them with slash commands:

| Command | Action |
|---------|--------|
| `/provider` | List available providers |
| `/provider add <name> <url>` | Add a custom provider |
| `/provider use <name>` | Switch to a provider |
| `/provider remove <name>` | Remove a custom provider |

Built-in providers:

| Provider | Endpoint | Auth |
|----------|----------|------|
| Nexum Inferentia | `http://127.0.0.1:11434` | none |
| Ollama | `http://127.0.0.1:11434/v1` | none |
| LM Studio | `http://127.0.0.1:1234/v1` | none |
| NVIDIA | `https://integrate.api.nvidia.com/v1` | `NVIDIA_API_KEY` |

API keys are read from the environment (`NEXUM_API_KEY`, then `OPENAI_API_KEY`)
and are **never** written to disk.

## Features

- Streaming chat responses (SSE)
- Agentic tool calls (via `nxm-tools`: `read_file`, `list_resources`, `web_search`)
- Markdown rendering in the terminal
- Session save/load, metrics, context overlay
- Configurable providers via `/provider` commands
- Agent modes: chat / architect / developer / researcher

## Configuration

Config is stored in `~/.nexum/tui.toml`. Sessions live in `~/.nexum/sessions/`.

## Dependencies

- `ratatui` + `crossterm` — TUI rendering
- `reqwest` — HTTP/SSE streaming
- `tokio` — async runtime
- [`nxm-tools`](https://github.com/dangranaz/nxm-tools) — pure-Rust tool actuators

## License

MIT
