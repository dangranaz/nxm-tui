# nxm-tui — Design (self-contained summary)

> **Product:** `nxm-tui` (repo `nxm-tui/`, public mirror of `dangranaz/nxm-tui`).
> **Status:** work-in-progress on `main` + feature branches (see § Status & branches).
> **Origin:** wayfinder tickets `docs/wayfinder/tickets/` + handoffs `docs/handoffs/` + loop engineering `loopeng-01` (bounded loop, circuit breaker, continue-after-halt).
> **Companion ITA:** `2026-11-05_nxm-tui-DESIGN_ITA.md` (keep in sync).
> **Target:** nexum.framer.ai publication as public WIP.

## What it is and why

`nxm-tui` is a **terminal chat client** for any OpenAI-compatible LLM server (Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM). It is a **pure client**: no server management, no hardware detection, no model loading — just connect and chat.

It exists because the NXM inference engines (`engine-mlx`, `engine-metal`, `nxm-memory`) needed a lightweight, keyboard-driven frontend that works entirely in the terminal, supports streaming SSE responses, agentic tool calls, and per-provider API key management without ever writing secrets to disk.

Unlike a web UI, `nxm-tui` runs locally, draws only on change (no 60 Hz busy redraw), and treats the conversation as a structured transcript that an agentic loop can extend with tool calls.

## Public interface (TUI commands + slash commands)

Transport: **native TUI** (ratatui + crossterm, alternate screen, raw mode). One JSON-RPC-free interactive session.

### Slash commands

| Command | Action |
|---|---|
| `/provider` | List available providers |
| `/provider add <name> <url>` | Add a custom OpenAI-compatible provider |
| `/provider use <name>` | Switch to a provider |
| `/provider remove <name>` | Remove a custom provider |
| `/provider set-key <name> <key>` | Store API key for a provider (via KeyStore) |
| `/provider remove-key <name>` | Delete a stored API key |
| `/continue` (alias `/cont`) | Resume after a hard-halt (budget cap or circuit breaker) with a fresh Agent and the current transcript |
| `/tools` (Ctrl+E) | Open the tools overlay: enable/disable tools per checkbox; `a` = enable all, `n` = disable all |
| `/todo` (Ctrl+D) | Toggle the read-only todo checklist overlay |
| `Ctrl+R` | Toggle reasoning overlay (live `<thinking>` stream) |
| `Ctrl+O` | Toggle tool detail overlay (last invocation args + output) |
| `Ctrl+M` | Toggle per-token metrics overlay |
| `Esc` | Close any open overlay / cancel pending passphrase prompt |
| `Ctrl-C` / `Ctrl-Q` | Quit (aborts in-flight agent task first) |

### Built-in providers

| Provider | Endpoint | Auth |
|---|---|---|
| Nexum Inferentia | `http://127.0.0.1:11434` | none |
| Ollama | `http://127.0.0.1:11434/v1` | none |
| LM Studio | `http://127.0.0.1:1234/v1` | none |
| NVIDIA NIM | `https://integrate.api.nvidia.com/v1` | `NVIDIA_API_KEY` |

API keys are read from the environment (`NEXUM_API_KEY`, then `OPENAI_API_KEY`) and are **never** written to disk unless the user explicitly stores them via the KeyStore.

## Key decisions (as-built, verified)

1. **Pure client, no server side.** `nxm-tui` only consumes `GET /v1/models` and `POST /v1/chat/completions` (SSE). It never starts, builds, or loads a model. That is the engine's job (`engine-mlx`, `engine-metal`, `nxm-memory`).
2. **Draw-on-change + 30 Hz idle polling.** The frame is rebuilt only when something visible changed (key, resize, token drain, status/health change, spinner tick). Idle ticks perform no draw work — no busy redraw at 60 Hz.
3. **Per-message history render cache.** Streaming re-renders one message, not O(N) over the whole history.
4. **Agentic loop with hard cap + stagnation breaker.** `Agent::run` streams a response, resolves `tool_calls` through `nexum-tools`, appends `role:"tool"` results, and re-queries until `finish_reason: "stop"` — or until `bounded::BudgetTracker` (`DEFAULT_MAX_STEPS = 15`) halts the loop, or `circuit_breaker::StallDetector` (`DEFAULT_STALL_THRESHOLD = 3`) trips on repeated identical tool calls. Both emit `ToolPart::LoopHalted { reason, steps }`.
5. **No `unwrap`/`expect` in production code.** Fallible parsing maps to `String` errors. `anyhow::Result` for app-level errors, `thiserror` for library-level typed errors (per `RULES.md`).
6. **KeyStore injectable seam (I1/R5).** `KeyStore` trait with three backends: `EnvStore` (env only), `OsKeychainStore` (OS-native via `keyring`), `EncryptedFileStore` (AES-256-GCM + Argon2id, `keys.enc` 0600). Secret values are wrapped in `Zeroizing` and never logged.
7. **MCP host subsystem (stdio).** `nxm-tui` spawns an MCP server child (`--transport stdio`), completes the `initialize` handshake declaring the `sampling` capability, and serves `sampling/createMessage` by bridging to the LLM backend (auto-approved). Clean no-op when disabled or binary absent — never blocks chat.
8. **Markdown V3 pipeline (adopted from grok-build `xai-grok-markdown`).** Pure-Rust port of pulldown-cmark → styled ratatui output, with syntect syntax highlighting (`default-fancy`, no onig/C), cell-width-aware wrapping (CJK/emoji), OSC 8 hyperlinks, source maps for copy/selection. Legacy V1 renderer kept as fallback behind `NXM_USE_NEW_MD=1` (default OFF).
9. **Tool selection orthogonality (T1–MT3).** `Registry` tracks `enabled: HashSet<String>` orthogonal to gating (`needs_approval`). `manifest_for(task)` does per-task automatic selection (Core always included as fail-safe) unioned with manually enabled tools. Default manual set is empty; the user adds via overlay.

## Code layout

```
src/
  main.rs            — TUI entry: ratatui terminal, main event loop, agent spawn, MCP connect
  lib.rs             — public module graph (bin + lib share one crate tree)
  app.rs             — App state: RunState, AgentMode, messages, metrics, providers, keystore passphrase
  agent.rs           — Agentic streaming loop (bounded budget + circuit breaker)
  bounded.rs         — BudgetTracker: hard iteration cap (DEFAULT_MAX_STEPS = 15)
  circuit_breaker.rs — StallDetector: consecutive identical tool-call tripping (DEFAULT_STALL_THRESHOLD = 3)
  commands.rs        — slash command parsing (parse_command, Command enum)
  config.rs          — TuiConfig: TOML persistence (~/.nexum/tui.toml), max_steps, stall_threshold
  connection.rs      — SSE chat stream (single-turn path, pre-agentic)
  event.rs           — AppEvent: Key / Resize / Tick polling
  handler.rs         — key dispatch to commands / overlays / agent actions
  keys.rs            — key resolution modes (EnvFirst / KeychainFirst / EncryptedFile)
  keystore/          — KeyStore trait + EnvStore / OsKeychainStore / EncryptedFileStore
  mcp/               — MCP host: protocol, transport (stdio), sampling bridge, client session
  md/                — Markdown V3 renderer (core, parse, render, buffers, wrap, links, syntax, theme)
  markdown.rs        — dispatcher: legacy V1 vs V3 pipeline (NXM_USE_NEW_MD env flag)
  message.rs         — Message / Role domain types
  metrics.rs         — per-token metrics (chars, tokens/sec)
  mode_bar.rs        — status bar: mode, provider, model, halted indicator
  models.rs          — /v1/models fetch + ModelEntry
  provider.rs        — provider presets + custom provider management
  session.rs         — session save/load (~/.nexum/sessions/)
  sidebar.rs         — left sidebar render (sessions, providers, models)
  tool_types.rs      — ToolPart / ToolInvocation / ToolResult / ApprovalRequest
  tools_overlay.rs   — tools enable/disable overlay (T1)
  todo.rs / todo_tool.rs / todo_overlay.rs — todo list state + tool + read-only overlay
  ui.rs              — top-level render: layout, overlays, mode bar
  utils/             — small pure helpers
tests/               — all integration tests (never inline #[cfg(test)] in src/, per RULES)
```

## How to verify

```bash
cd nxm-tui

# Formatting + lint + tests
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Install from source
cargo build --release
./target/release/nxm-tui

# Install via script (downloads prebuilt binary)
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh

# Acceptance (interactive):
#   1. nxm-tui → provider menu appears if no server detected
#   2. /provider add ollama http://127.0.0.1:11434/v1
#   3. /provider use ollama → state = Running
#   4. Send a message → streaming SSE tokens appear
#   5. Ctrl+E → tools overlay toggles read_file/list_resources/web_search
#   6. Ctrl+D → todo overlay appears (read-only checklist)
#   7. After a LoopHalted → /continue resumes with fresh Agent + same transcript
```

Prerequisite for building: Rust stable (edition 2021). Prebuilt binaries for Apple Silicon macOS and Linux x86_64 are published on each release.

## Out of scope (known follow-ups)

- **HTTP stateless transport** for the TUI itself (it is a stdio-native TUI; MCP is stdio child).
- **Persistence of tool selection** to TOML (overlay state is in-session only today; phase 2).
- **MCP `tools/call`** — only `sampling/createMessage` is served today; full MCP client is refinement roadmap (`docs/mcp/ARCHITECTURE.md`).
- **Rich gate configurability** — bounded loop uses `DEFAULT_MAX_STEPS` / `DEFAULT_STALL_THRESHOLD` with config overrides; no per-tool step budgets yet.
- **Resume/ask-to-continue** beyond `/continue` (no Ralph loop, no token budget gating).
- **Install.sh + MCP registration** in agent configs (documented but not automated).

## Status & branches

| Branch | State |
|---|---|
| `main` | loopeng-01-01 merged (bounded execution, hard cap on Agent::run) |
| `work/loopeng-01-02-circuit-breaker` | pending external review (stagnation breaker) |
| `work/loopeng-01-03-continue-after-halt` | in progress (`/continue` command) |
| `feat/tools-select-mechanism` | MT-1..MT-3 (per-task tool selection) pending external review |
| `feat/tools-todo` | MT-1..MT-3 (todo tool + overlay) pending external review |

## Review history (process)

Each micro-task is implemented by external agents and **reviewed by a fixed external reviewer** (two axes: Standards + Spec) before merge. Review has caught defects an auto-approve would have let through (empty diffs, gates declared but not run, non-working backend invocation). Countermeasure: a delivery clause requiring gates run on the final commit plus independent verification with real calls. All pending branches above await this review before `--ff-only` into their target branches.
