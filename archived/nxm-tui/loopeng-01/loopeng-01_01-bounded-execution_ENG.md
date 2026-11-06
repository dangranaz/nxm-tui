# Delegation — 01 Bounded Execution (iteration cap in Agent::run)

> **Status:** READY · **Created:** 2026-10-26
> Companion: `loopeng-01_01-bounded-execution_ITA.md` (keep in sync).
> Design origin: nxm-session session `loopeng-01` (loop engineering) +
> `docs/wayfinder/research/loop-engineering.md` §5 (P0 Bounded Execution) and §7 item 1.

## Objective (one micro-task, then STOP)
Give `Agent::run` (`src/agent.rs`) a **hard cap on the number of iterations**
of the ReAct loop. Today `run()` is an uncapped `loop {}`: it only exits when
the model stops calling tools. We need a configurable cap (default **15**):
when reached, the loop stops (**hard stop**), emits a new
`ToolPart::LoopHalted { reason, steps }` toward the UI, and returns `Ok(())`
without panicking. No git, no agent work-branch at runtime, no `run_gate`:
ONLY the cap. (The "measurable branch" and the external validator are step 5
of loop engineering, a different micro-task.)

## Skill
**`tdd`** — the cap logic is pure and verifiable; write it test-first (counter
→ stop condition → ToolPart emission). An integration test that trips the cap
is the acceptance gate (rule 7).

## Context (already decided — do not reopen)
- What we count: **loop iterations** (= model turns), not tool-calls, not
  tokens. Token budget is a later phase, OUT of scope here.
- Default cap = **15** (the user chose 15, not 25).
- On expiry: **hard stop** (the loop ends and returns `Ok(())`), NOT
  ask-to-continue. The user-confirmed resume tool is a separate micro-task
  (loopeng-01_02), do NOT do it here.
- Config in an **external file**: `TuiConfig` already exists, persisted in
  `~/.nexum/tui.toml` (`src/config.rs`). Add `max_steps` there.
- Invariant architectural boundary: `Agent` lives in a `tokio::spawn`, **never
  touches `App`**, communicates only via `mpsc::UnboundedSender<ToolPart>`. The
  cap and its signaling must live INSIDE the Agent and emit via `ToolPart`.
  Do NOT let the Agent touch `App`/filesystem/git.
- `src/agent.rs` is **460 lines**, already over the 300-line limit in
  `RULES.md`. Therefore the pure cap logic must be extracted into a **new
  module** `src/bounded.rs` (see Requirements), so agent.rs does not get worse
  and the logic is testable in isolation.
- `src/agent.rs` has a **pre-existing** inline `#[cfg(test)] mod tests`: do NOT
  add more inline tests there. New tests go under `tests/` (RULES.md).

## Rules to follow
- Current repo: `RULES.md` (max 300 lines/file; NO `unwrap`/`expect` in prod
  code; structured `tracing` with target `nexum::<module>::<feature>` on every
  public function; tests in `tests/<module>.rs`, NEVER new inline `#[cfg(test)]`;
  `///` docs with an example on public APIs; pin exact versions; update
  `CHANGELOG.md` and `CONTEXT.md`; Conventional Commits).
- Ecosystem: `nxm-docs/RULES.md` and relevant docs (search with `nxm-memory`).
- Scope↔rule conflict already resolved in this brief: creating the new module
  `src/bounded.rs`, adding the variant in `src/tool_types.rs`, the field in
  `src/config.rs`, the wiring in `src/main.rs`, and updating
  `CHANGELOG.md`/`CONTEXT.md` **are authorized** and are NOT "leaving scope".

## Requirements (each with its test — see invariant rule 7)
- **R1 — Pure bounded module.** New file `src/bounded.rs` (<300 lines) with the
  pure cap logic: a constant `DEFAULT_MAX_STEPS: u32 = 15`, and a type (e.g.
  `BudgetTracker`) with `new(max_steps: u32)` and a method that, called each
  iteration, signals continue-or-halt (e.g. returns an enum `Budget::Continue`
  / `Budget::Halted { steps }`). Zero I/O, zero network deps. Public functions
  with `tracing` + `///` doc with an example.
  + **test** `tests/bounded.rs`: a `BudgetTracker::new(3)` returns `Continue`
    for the first 3 ticks and `Halted { steps: 3 }` on the 4th; `new(0)` halts
    immediately.
- **R2 — UI variant.** In `src/tool_types.rs`, add
  `ToolPart::LoopHalted { reason: String, steps: u32 }` and its arm in
  `render_lines()` (one readable line, e.g. warning color, showing `reason` and
  `steps`). Keep `#[derive(Debug, Clone, PartialEq, Eq)]`.
  + **test** under `tests/` (new or adjacent to `tests/agent_request.rs`):
    construct a `ToolPart::LoopHalted { reason: "...".into(), steps: 15 }`, call
    `render_lines()` and assert the output is non-empty and contains "15" (or
    the reason text).
- **R3 — Cap wired into Agent::run.** `Agent::new` takes a `max_steps: u32`
  (add the parameter); `run()` uses `BudgetTracker`: each iteration, before or
  after `stream_turn`, advance it; if the budget is `Halted`, emit
  `tx.send(ToolPart::LoopHalted { reason, steps })` and return `Ok(())` WITHOUT
  panicking. Normal behavior (exit on `calls.is_empty()`) stays unchanged when
  the cap does not trip. Update the `request_body()` doc-test that calls
  `Agent::new(...)` with the new signature, and any module/`run` doctest if
  present.
  + **test** `tests/bounded.rs` (integration): an `Agent` with a low
    `max_steps` (e.g. 2), `approval_tx: None` (headless), pointed at a fake HTTP
    server (e.g. `wiremock` or a local listener) that ALWAYS replies with
    `tool_calls` (never `finish_reason:"stop"`), must terminate after exactly
    `max_steps` iterations and emit a `ToolPart::LoopHalted` on the channel.
    The test fails (hang/timeout) if the cap is missing. If the repo already
    has a fake-server helper in its tests (search with `nxm-memory`
    `search_code "bytes_stream"` / `tests/live_ollama.rs`), reuse it.
- **R4 — External config.** In `src/config.rs`, add to `TuiConfig` the field
  `pub max_steps: Option<u32>` with `#[serde(default)]`, and initialize it in
  `Default` (to `None` — apply the default at the use site with
  `.unwrap_or(DEFAULT_MAX_STEPS)`, so `bounded.rs` stays the single source of
  the default). In `src/main.rs`, at the `Agent::new` call site, read
  `config.max_steps.unwrap_or(nxm_tui::bounded::DEFAULT_MAX_STEPS)` and pass it.
  + **test**: extend/add a `config` test that round-trips a `TuiConfig` with
    `max_steps: Some(7)` via `toml` (serialize→deserialize) and verifies the
    value survives; and that a toml WITHOUT `max_steps` loads `None` (thanks to
    `#[serde(default)]`).

## Work branch
- Create and work on a dedicated branch: `work/loopeng-01-01-bounded-execution`.
- Do NOT commit to `main`/develop. The work must be **validated before** merge.

## Available tools (use if needed)
- `nxm-mcp-exec` — validate the Rust code by passing `repo_path` (NOT `cargo`
  from shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`),
  `cargo_fmt_check`, `cargo_test`. **Before declaring "done/mergeable", call
  `run_gate` and proceed only on `verdict:"GO"`.** Fall back to `cargo` if the
  server is unavailable.
- `nxm-memory` — search rules/context/precedents (e.g. streaming test patterns,
  fake-server helpers, `ToolPart` conventions).
- `nxm-decision` — categorize any implementation forks.
- `nxm-session` — read/update session `loopeng-01` state if relevant.

## Prohibitions
- Do NOT implement the resume tool (`resume_loop`) nor any ask-to-continue:
  that is micro-task loopeng-01_02.
- Do NOT implement circuit breaker, Ralph loop, `run_gate` integration, token
  budget, context reset/compaction: out of scope.
- Do NOT let the `Agent` touch `App`, the filesystem or git at runtime.
- Do NOT add new inline `#[cfg(test)]` in `src/agent.rs`.
- Do NOT leave scope — BUT rules win: if a file exceeds 300 lines, split it;
  update CHANGELOG/CONTEXT; put tests under `tests/`. On a scope↔rule conflict,
  STOP and ask.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this micro-task, then STOP.
- Show the diff and how you verified (`run_gate` output / relevant tests).
- **Hand off to an external reviewer** — reference skill `code-review`. The
  branch is validated/merged **only after** approved review.
- If anything is ambiguous, STOP and ask.

## Expected verification
```bash
# Preferred: typed diagnostics via nxm-mcp-exec run_gate (repo_path = repo root)
# verdict must be "GO".
# Shell fallback:
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --test bounded
cargo test            # full suite, no regressions
```
