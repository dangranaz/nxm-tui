# nxm-tui

Terminal UI client for NXM inference servers. Built with ratatui + crossterm.

## Purpose
Connects to any OpenAI-compatible LLM server (including NXM engines) and provides
an interactive chat interface in the terminal.

## Features
- Streaming chat responses (SSE)
- Markdown rendering in terminal
- Session history and persistence
- Server connection management
- Autocomplete for commands
- Configurable via TOML

## Dependencies
- ratatui + crossterm for TUI rendering
- reqwest for HTTP/SSE streaming
- tokio for async runtime

## NOT included
- Inference logic (connects to external server)
- Model loading (that's the engine's job)
