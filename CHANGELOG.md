# Changelog

# 2026-09-27

## Sotto-tappa 2 — Migrazione a repo autonomo + fix remotes (Tappa 2 MIGRATION-PLAN)
- Remotes updated per ADR-002/003; public mirror managed by `nxm-sync`.

# 2026-09-20

## perf: memoized token estimate + cached cwd (T4)
- `app.rs`: `estimated_tokens` memoized in a `Cell` fingerprinted by
  (messages.len(), last message) — O(N) walk ran 3-4× per frame in the
  render path; recomputes only when messages change.
- `app.rs`: `current_dir_name` cached once (`OnceLock`) — was a syscall
  per frame; the process never chdirs (upgrade path: TTL if that changes).
- New `tests/render_caches.rs` (4 tests): streaming growth (a len-only
  cache key would serve stale 8), new-message recompute, differential
  oracle vs a fresh App, cwd consistency. 68/68 green, clippy 0 error.

## perf: draw-on-change + per-message history render cache (T3)
- `event.rs`: idle poll 16 ms → 33 ms (~30 Hz wake-up; halves idle CPU).
- `main.rs`: dirty flag — `terminal.draw` only on key/resize/token drain/
  status/health change; the Thinking spinner keeps animating while working.
- `history.rs`: per-message render cache (`HistoryCache`, key = content_len
  + tool_parts + width): streaming re-renders only the last message instead
  of reparsing the whole O(N) history each frame; resize invalidates all.
- New `tests/history_cache.rs` (3 tests): repeated frames identical, streaming
  token invalidation, resize re-wrap (output differential, not internals).
- Infra forced by the perf change: lib target (`src/lib.rs`) — one module
  graph; tests consume `nxm_tui::`, per-test `#[path]` mirrors deleted (they
  had gone stale: test crates couldn't resolve `crate::autocomplete`). 9 test
  files now compile: 64/64 green, clippy 0 error.
- Includes the uncommitted command-menu work from the previous session
  (autocomplete/handler/sidebar + tests/command_menu.rs) — entangled with the
  lib conversion, so it rides in the same commit.

# 2026-09-08

## feat: overlay render regression net + content-sized help
- New `tests/overlay_render.rs` (7 tests) on ratatui `TestBackend` (80x24,
  no TTY — pi-tui VirtualTerminal pattern): every overlay renders without
  panicking and shows its content (help, thinking full/empty, tool detail
  full/empty, approval with risk badge, approval empty no-op).
- The first run caught a real bug: help content had outgrown its fixed h=24
  box, cutting the newest hints. Help height is now content-sized
  (`items.len()+2`, clamped) and section blanks condensed.
- Note: `cargo fmt --check` is dirty repo-wide (pre-existing style, incl.
  untouched files) — intentionally not reformatted to avoid churn.

## feat: risk badge in approval overlay
- Approval overlay now shows a red advisory badge on top of the Y/N decision:
  `⚠ sensitive path` for secret-bearing paths (`.env`, `.ssh`/`.aws`/`.gnupg`
  segments, keys/certs/credentials/tokens, `..` traversal), `⇄ network egress`
  for `web_search`, `⚠ unrecognized tool` for unknown tools.
- Matcher is deliberately conservative (case-insensitive, segment-aware):
  false positives only add a warning line, a miss could leak a secret into
  the transcript. Advisory only — the user still decides.
- Tests: 4 new in `tests/tool_approval.rs` (network, sensitive matrix incl.
  Windows paths, quiet negatives, unknown tools).

## feat: history viewport (follow-tail + scrollback model)
- Fixed inverted/missing follow: new tokens now stay visible (pinned tail);
  previously offset 0 showed the oldest lines and fresh output scrolled away
  unseen. `App::scrollback` counts lines hidden below the viewport;
  `App::viewport_offset()` clamps short buffers to top, deep scrollbacks to
  the oldest line.
- Conventional scroll keys: Up = older, Down = newer, plus PgUp/PgDn (±10).
  Send/clear/load re-pin to the tail. Scrolling up never loses the live tail
  position — Down returns to it.
- Fix markdown code-fence truncation panic (`&line[..w]` byte slice) with a
  char-safe `…` truncation, same family as the earlier `ToolResult` fix.
- Tests: `tests/history_viewport.rs` (8 tests) — offset math, key direction,
  follow invariant, emoji fence no-panic, plain-text regression.

## feat: tool approval gating (Y/N overlay)
- Agent now asks before running gated tools: `read_file` (arbitrary file
  contents), `web_search` (network egress) and unknown tools. Only
  `list_resources` is auto-approved (`needs_approval()` in `tool_types.rs`).
- Modal approval overlay (`y` approve / `n` or `Esc` deny) in
  `tool_overlay.rs::render_approval`, rendered above all others; other keys
  are swallowed while a decision is pending so a tool can never run or skip
  by an unrelated key. The agent task parks on a oneshot reply; quit/drop =
  fail-safe deny.
- Denials are recorded as the tool result (`denied by user …`) so the model
  reacts on the next turn instead of stalling.
- Plumbing: `ApprovalRequest` channel (`tool_types.rs`), `Agent::approve()`,
  `App::{pending_approval, resolve_approval}`, drain in `main.rs` Tick loop.
  Headless (`None` channel, tests) auto-approves so non-interactive runs
  never hang.
- Tests: `tests/tool_approval.rs` (4 new: policy, resolve, deny e2e with
  no-leak assert, approve e2e) + updated `roundtrip.rs` for the new
  `Agent::new` signature.

## feat: tool detail overlay + char-safe truncation
- New tool detail overlay (`Ctrl+O`, `Esc` closes) in `src/tool_overlay.rs`:
  `last_tool_exchange()` finds the most recent invocation + its `role:"tool"`
  result; overlay shows name, call id, full args and output (6-line cap).
  Read-only kimi `approval-preview` pattern — gating the agent is a later step.
- Fix `ToolResult` truncation panic: byte slice `&s[..100]` panicked on
  multibyte boundaries; new `ToolPart::truncate_chars()` is char-safe + `…`.
- Tool history rows now render an 80-char args preview (`▶ read_file {…}`)
  instead of the bare tool name.
- Tests: `tests/tool_overlay.rs` (5 tests) — multibyte truncation, args
  preview, pending/resolved/most-recent exchange lookup.

## feat: reasoning overlay + per-token metrics + thinking-tag parser fix
- New reasoning overlay (`Ctrl+R`, `Esc` closes): renders live `thinking_content`
  in `src/overlays.rs::render_thinking`, wired in `src/ui.rs`, `src/handler.rs`.
  Mirrors codex transcript overlay / kimi `streaming-ui` thinking separation.
- Per-token metrics wired: `main.rs` Tick loop now calls
  `metrics.record_chars()` on every `ToolPart::Text` (was dead code, tokens/sec
  always 0). Metrics overlay (`Ctrl+M`) now shows real throughput.
- Fix `apply_token` thinking parser: standalone `>` closing bracket previously
  trapped all later tokens in thinking mode; now exits thinking and preserves
  text after the bracket. Handles split (`<thinking>`/`>`), combined
  (`>visible`), and single-token (`a<thinking>b</thinking>c`) shapes.
- Tests: `tests/thinking_metrics.rs` (6 tests) — streaming, metrics, overlay
  flag, three thinking-tag shapes.

# 2026-09-07

## feat: configurable LLM providers via /provider commands
- New `src/provider.rs`: `Provider` struct + presets (Nexum Inferentia, Ollama,
  LM Studio, NVIDIA) and custom-provider validation.
- New slash commands: `/provider list|add <name> <url>|use <name>|remove <name>`
  (aliases `/prov`, `/p`).
- Custom providers persisted in `~/.nexum/tui.toml`; API keys stay env-only
  (never on disk).
- NoServer menu now generated dynamically from the provider registry.
- Fix: Ollama preset uses its official OpenAI-compatible port `11434` (was `11435`).
- NVIDIA cloud endpoint (`https://integrate.api.nvidia.com/v1`) with Bearer auth.
- Tests: `tests/provider.rs` (8 tests) — presets, validation, merge/lookup, parsing.

# 2026-08-22

## feat: migrate nxm-tui to GitHub
- Migrated from monorepo (Projects_Tmp/nxm/tui/)
- Converted workspace deps to standalone versions
- 20 source files (app, connection, ui, markdown, history, etc.)
- Added RULES.md, CONTEXT.md, CHANGELOG.md
