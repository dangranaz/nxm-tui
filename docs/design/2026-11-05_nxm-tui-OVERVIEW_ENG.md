# nxm-tui — Overview (work-in-progress)

> Terminal chat client for any OpenAI-compatible LLM server. Public WIP mirror of `dangranaz/nxm-tui`.

## In one line

A keyboard-driven terminal TUI that connects to any OpenAI-compatible LLM server (Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM) and streams chat responses with agentic tool calls, per-provider key management, and a bounded agentic loop — no server management, no secrets on disk.

## What it does

- **Connects to any provider** — built-in presets for Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM; add custom providers with `/provider add`.
- **Streams chat** — SSE token streaming rendered cell-width-aware (CJK/emoji safe) with Markdown V3 (grok-build port) and syntax highlighting.
- **Agentic tools** — `read_file`, `list_resources`, `web_search`, `grep`, `search_replace` via `nexum-tools`, with per-tool enable/disable overlay and approval gating (Y/N).
- **Bounded loop** — hard iteration cap (`DEFAULT_MAX_STEPS = 15`) plus stagnation breaker (`DEFAULT_STALL_THRESHOLD = 3`) stop runaway tool-calling models; `/continue` resumes with a fresh Agent and the same transcript.
- **Secret-safe** — injectable `KeyStore` seam (`EnvStore`, `OsKeychainStore`, `EncryptedFileStore` AES-256-GCM + Argon2id); keys wrapped in `Zeroizing`, never logged, never written to disk unless explicitly stored.
- **MCP host** — spawns stdio MCP servers, serves `sampling/createMessage` by bridging to the LLM backend; clean no-op when absent.
- **Overlays** — reasoning (`Ctrl+R`), tool detail (`Ctrl+O`), metrics (`Ctrl+M`), tools (`Ctrl+E`), todo (`Ctrl+D`).

## Status

Work-in-progress. `main` has bounded execution (loopeng-01-01). Circuit breaker, continue-after-halt, per-task tool selection, and todo tool are on feature branches pending external review (Standards + Spec axes) before `--ff-only` merge.

## Try it

```bash
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh
nxm-tui
```

Or build from source:

```bash
git clone https://github.com/dangranaz/nxm-tui.git
cd nxm-tui
cargo build --release
./target/release/nxm-tui
```

## Design reference

Full design doc: `docs/design/2026-11-05_nxm-tui-DESIGN_ENG.md` (ENG) / `..._DESIGN_ITA.md` (ITA).
