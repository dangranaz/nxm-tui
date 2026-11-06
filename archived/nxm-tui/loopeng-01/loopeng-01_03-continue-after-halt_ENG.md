# Delegation — 03 Continue-after-halt (resume the loop on user confirmation)

> **Status:** READY · **Created:** 2026-10-26
> Companion: `loopeng-01_03-continue-after-halt_ITA.md` (keep in sync).
> Design origin: nxm-session session `loopeng-01` (loop engineering).
> Consolidated precedents on `main`: `loopeng-01_01-bounded-execution`
> (commit `bed58d3`) and `loopeng-01_02-circuit-breaker` (commit `7c5c8cb`).

## Objective (one micro-task, then STOP)
Give the user an explicit way to **resume the loop after a hard-stop**. When the
iteration cap (`BudgetTracker`) or the stagnation breaker (`StallDetector`)
trip, the Agent emits `ToolPart::LoopHalted` and terminates: the `tokio::spawn`
ends, `inference_task` goes back to `None`, state returns to idle. We need a UI
command **`/continue`** that, **only after a halt**, re-triggers a new inference
(new Agent) with the current transcript (`app.messages`) and a **fresh budget**
— cap and breaker start from zero. The confirmation is the user's action of
typing the command.

## Design note (why UI, not a model tool)
The original requirement said "a tool to resume on user confirmation", but the
code analysis reveals a paradox: after the hard-stop the loop is **already
terminated** and the model no longer runs, so it cannot invoke any tool to
resume. The correct mechanism for "resume after the stop on confirmation" is
**UI-side** (a user command/action), not a model tool. This is already decided
(nxm-decision, conf. 0.955) — do NOT reopen it.

## Skill
**`implement`** — this is a UI/state integration (handler + App + main spawn),
not pure algorithmic logic for TDD; but each requirement still has its test
(rule 7). Use the existing integration tests as a model.

## Context (already decided — do not reopen)
- Mechanism: **UI command `/continue`** (in `handler.rs`/`App`/`main.rs`), NOT a
  model tool, NOT `needs_approval`.
- Resume = **re-spawn a new Agent** with `app.messages` as `initial_msgs` and a
  fresh budget (the spawn in `main.rs` already does
  `initial_msgs = app.messages.clone()` and builds fresh
  `BudgetTracker`/`StallDetector` on every inference — so the budget restarts
  from zero automatically). No need to touch
  `Agent`/`bounded.rs`/`circuit_breaker.rs`.
- **State guard**: `/continue` only has an effect **if the last turn ended with
  a halt**. In any other case (no halt, loop never started, loop in progress) it
  must be a **no-op with a clear status message** (e.g. "no loop to resume").
  This requires a flag in `App` (e.g. `pub halted: bool` or
  `last_halt: Option<...>`) set to `true` when the UI consumes
  `ToolPart::LoopHalted` (today in `main.rs` around line 453) and cleared when a
  new inference starts.
- Reuse the existing spawn flow: resume sets `pending_message` +
  `RunState::Thinking` exactly like a user send, so the existing "Start
  inference" block in `main.rs` does the rest. Do NOT duplicate the spawn logic.
  (If `pending_message` requires content, use an internal marker or an implicit
  `continue`: choose the minimal approach that reuses the existing block without
  sending a new user-visible message into the transcript.)
- Invariant architectural boundary: the `Agent` never touches `App`; the UI
  mirrors `ToolPart`s into `app.messages`. The `/continue` command lives
  entirely UI-side.

## Rules to follow
- Current repo: `RULES.md` (max 300 lines/file; NO `unwrap`/`expect` in prod;
  structured `tracing` `nexum::<module>::<feature>` on every new public
  function; tests in `tests/<module>.rs`, NEVER new inline `#[cfg(test)]`; `///`
  docs with an example on new public APIs; update `CHANGELOG.md` and
  `CONTEXT.md`; Conventional Commits).
- Ecosystem: `nxm-docs/RULES.md` and relevant docs (search with `nxm-memory`).
- Scope↔rule conflict already resolved: adding the `Command::Continue` variant
  in `src/commands.rs`, the arm in `src/handler.rs`, the flag in `src/app.rs`,
  the flag reset in `src/main.rs`, and updating `CHANGELOG.md`/`CONTEXT.md`
  **are authorized** and are NOT "leaving scope".

## Requirements (each with its test — see invariant rule 7)
- **R1 — Command parsing.** In `src/commands.rs`, add `Command::Continue` and
  make `parse_command` recognize the input `/continue` (and a short alias if
  consistent with the others, e.g. `/cont` — choose and document).
  + **test** under `tests/` (where `parse_command` tests live; search with
    `nxm-memory` `search_code "parse_command"`): `parse_command("/continue")`
    returns `Command::Continue`.
- **R2 — Halt flag in App.** Add to `App` a flag (e.g. `pub halted: bool`,
  default `false`) set to `true` when the UI consumes `ToolPart::LoopHalted`
  (`main.rs`, existing arm) and **reset to `false`** when a new inference starts
  (in the "Start inference" block).
  + **test** under `tests/`: an `App` test (construction + transition) verifying
    the default `false`; if the set/reset logic is extractable into a public
    `App` method (e.g. `App::mark_halted()` / reset), test it directly. If the
    set only happens in the `main.rs` loop (not unit-testable), extract the
    transition into an `App` method and test that method.
- **R3 — `/continue` resumes only after a halt.** In the `Command::Continue` arm
  of `handler.rs`: if `app.halted == true`, set the state to restart a new
  inference (the `pending_message` + `RunState::Thinking` flow reused by the
  spawn block), clear `halted`, and give a status feedback ("resuming the
  loop"). If `app.halted == false`, **no-op** with a message ("no loop to
  resume"). It must NOT crash nor start empty.
  + **test** under `tests/`: invoking the command handling with `halted=true`
    moves state to ready-for-inference (e.g. `RunState::Thinking` and
    `pending_message` populated) and `halted` returns to `false`; with
    `halted=false` state stays unchanged and a status message is set. (If the
    command dispatch is not directly callable from tests, extract the arm's
    logic into a public `App` method, e.g. `App::continue_loop() -> bool`, and
    test that.)

## Work branch
- Create and work on a dedicated branch: `work/loopeng-01-03-continue-after-halt`.
- Do NOT commit to `main`/develop. The work must be **validated before** merge.

## Available tools (use if needed)
- `nxm-mcp-exec` — validate the Rust code by passing `repo_path` (NOT `cargo`
  from shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`),
  `cargo_fmt_check`, `cargo_test`. **Before declaring "done/mergeable", call
  `run_gate` and proceed only on `verdict:"GO"`.** NOTE: on `main` there is a
  pre-existing NO-GO for ~20 unformatted `src/md/*` files, **not yours and out
  of scope** — verify YOUR files do NOT appear among `cargo_fmt_check` offending
  files. Fall back to `cargo`.
- `nxm-memory` — search rules/context/precedents (command patterns in
  `handler.rs`, `parse_command` tests, how `main.rs` consumes `LoopHalted` and
  starts inference).
- `nxm-decision` — categorize any implementation forks.
- `nxm-session` — read/update session `loopeng-01` state if relevant.

## Prohibitions
- Do NOT implement a model `resume_loop` tool: it is the paradox described in
  the Design note.
- Do NOT touch `Agent`/`bounded.rs`/`circuit_breaker.rs`: the fresh budget comes
  for free from the re-spawn.
- Do NOT duplicate the Agent spawn logic in `main.rs`: reuse the "Start
  inference" block via `pending_message` + `RunState::Thinking`.
- Do NOT implement auto-continue (automatic resume without the user): the user
  confirmation is the whole point.
- Do NOT touch `src/md/*` files (pre-existing fmt debt).
- Do NOT add new inline `#[cfg(test)]` in production files.
- Do NOT leave scope — BUT rules win (split >300 lines, CHANGELOG, tests under
  `tests/`). On a scope↔rule conflict, STOP and ask.
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
# Your files must NOT appear among cargo_fmt_check offending files.
# Shell fallback:
cargo fmt --check -- src/commands.rs src/handler.rs src/app.rs src/main.rs
cargo clippy --all-targets -- -D warnings
cargo test            # full suite, no regressions; new tests green
```
