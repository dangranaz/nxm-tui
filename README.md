# nexum-gui-terminal

Terminal UI for Nexum chat interface.

## Overview

This crate provides a terminal-based chat interface using ratatui.

## Usage

```bash
# Start TUI
cargo run --release -p nexum-gui-terminal

# Connect to specific server
cargo run --release -p nexum-gui-terminal -- --server http://127.0.0.1:8080
```

## Features

- Vim-like navigation (i for insert, Esc for normal, q for quit)
- Real-time chat interface
- Message history
- Configurable server connection

## Keybindings

| Key | Mode | Action |
|-----|------|--------|
| i | Normal | Enter insert mode |
| Esc | Editing | Enter normal mode |
| q | Normal | Quit |
| Enter | Editing | Send message |
| Backspace | Editing | Delete character |

## Architecture

```
nexum-gui-terminal
├── app       ← Application state and logic
├── ui        ← Terminal UI rendering
└── input     ← Input handling (TODO)
```

## Dependencies

- `ratatui` - Terminal UI framework
- `crossterm` - Terminal manipulation
- `tokio` - Async runtime
