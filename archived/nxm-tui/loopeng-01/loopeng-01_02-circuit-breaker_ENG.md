# Delegation — 02 Circuit Breaker (stagnation detection in Agent::run)

> **Status:** READY · **Created:** 2026-10-26
> Companion: `loopeng-01_02-circuit-breaker_ITA.md` (keep in sync).
> Design origin: nxm-session session `loopeng-01` (loop engineering) +
> `docs/wayfinder/research/loop-engineering.md` §3 pattern 8 (Circuit Breaker),
> §5 (P0) and §7 item 2.
> Consolidated precedent: `loopeng-01_01-bounded-execution` (already on `main`,
> commit `bed58d3`) — this task reuses its exact style and patterns.

## Objective (one micro-task, then STOP)
Give `Agent::run` (`src/agent.rs`) a **stagnation circuit breaker**: if the
model repeats the **same identical tool call** (same name + same arguments) for
**N consecutive cycles** (default **3**) with no progress, the loop trips (**hard
stop**), emits `ToolPart::LoopHalted` with a `reason` explaining the stagnation,
and returns `Ok(())` without panicking. It complements the iteration cap
(Bounded Execution) already present: the cap bounds total duration, the breaker
interrupts a *stuck* cycle before it burns the whole budget. No git, no agent
runtime branch, no `run_gate`, no Ralph loop: ONLY stagnation detection.

## Skill
**`tdd`** — the detector logic is pure and verifiable; write it test-first
(cycle signature → repeat counting → trip). An integration test that trips the
breaker with a server that always repeats the same tool call is the acceptance
gate (rule 7).

## Context (already decided — do not reopen)
- **Stagnation signal** = same **name + arguments** of the tool calls emitted in
  a cycle, repeated for N consecutive cycles. Compute a deterministic *signature*
  of an iteration's tool-call set (e.g. an ordered string/hash of each
  `ToolInvocation`'s `name` + `args`). Two cycles with the same signature count
  as a repeat; a different signature **resets** the counter (there was progress).
- **Default threshold N = 3** consecutive identical repeats → trip.
- On expiry: **hard stop** (return `Ok(())`). NOT ask-to-continue, NOT resume:
  those are separate micro-tasks.
- **Reuse the existing UI variant**: do NOT add new `ToolPart` variants. Reuse
  `ToolPart::LoopHalted { reason, steps }` (introduced by loopeng-01_01) with a
  `reason` describing the stagnation, e.g.
  `"stalled: identical tool call repeated 3× (<tool-name>)"`. `steps` = the
  current iteration index when the trip fires. Do NOT modify `src/tool_types.rs`
  unless strictly necessary.
- **MANDATORY reference pattern**: mirror `src/bounded.rs` as-built (pure logic,
  zero I/O, no clock/network, `tracing` with target `nexum::<module>::<feature>`,
  `///` docs with doctest, outcome enum). Read it before starting.
- The breaker is separate from `BudgetTracker`: do NOT merge them. They coexist
  in `run()`'s loop — each iteration goes first through `budget.tick()` (already
  present) and then through the detector after `stream_turn` (once you know the
  cycle's tool calls).
- Invariant architectural boundary: `Agent` lives in a `tokio::spawn`, **never
  touches `App`**, communicates only via `mpsc::UnboundedSender<ToolPart>`. The
  breaker lives INSIDE the Agent and emits via `ToolPart`. Do NOT let the Agent
  touch `App`/filesystem/git.
- `src/agent.rs` is ~483 lines, already over the 300-line `RULES.md` limit
  (pre-existing, known — pure logic lives outside, in modules): therefore the
  breaker logic goes in a **new module** `src/circuit_breaker.rs` (<300 lines),
  and the wiring in `agent.rs` must stay minimal (a few lines).
- `src/agent.rs` has a **pre-existing** inline `#[cfg(test)] mod tests`: do NOT
  add more inline tests there. New tests go under `tests/` (RULES.md).

## Rules to follow
- Current repo: `RULES.md` (max 300 lines/file; NO `unwrap`/`expect` in prod;
  structured `tracing` `nexum::<module>::<feature>` on every public function;
  tests in `tests/<module>.rs`, NEVER new inline `#[cfg(test)]`; `///` docs with
  an example on public APIs; pin exact versions; update `CHANGELOG.md` and
  `CONTEXT.md`; Conventional Commits).
- Ecosystem: `nxm-docs/RULES.md` and relevant docs (search with `nxm-memory`).
- Scope↔rule conflict already resolved in this brief: creating the new module
  `src/circuit_breaker.rs`, adding the field in `src/config.rs`, the wiring in
  `src/agent.rs`/`src/main.rs`, registering it in `src/lib.rs`, and updating
  `CHANGELOG.md`/`CONTEXT.md` **are authorized** and are NOT "leaving scope".

## Requirements (each with its test — see invariant rule 7)
- **R1 — Pure detector module.** New file `src/circuit_breaker.rs` (<300 lines)
  with the pure logic: a constant `DEFAULT_STALL_THRESHOLD: u32 = 3`, and a type
  (e.g. `StallDetector`) with `new(threshold: u32)` and a method that takes a
  cycle *signature* (e.g. `observe(&mut self, signature: &str) -> Stall`) and
  returns an enum `Stall::Ok` / `Stall::Tripped { repeats, signature }`.
  Semantics: signature equal to the previous → increment the repeat counter;
  different signature → reset to 1; when consecutive repeats reach `threshold` →
  `Tripped`. Zero I/O, zero network. Public functions with `tracing` + `///` doc
  with an example.
  + **test** `tests/circuit_breaker.rs`: with `StallDetector::new(3)`, observing
    the same signature `"A"` three times in a row → first two `Ok`, third
    `Tripped { repeats: 3, .. }`; inserting a different signature `"B"` in the
    middle **resets** (the sequence `A,A,B,A,A` does NOT trip). `new(1)` trips on
    the first repeat per the same rule (document and verify the low-threshold
    case you choose).
- **R2 — Deterministic signature.** A pure function (in `circuit_breaker.rs`)
  that, given a `&[ToolInvocation]` (or `&[(name, args)]` to avoid coupling the
  module to `tool_types` if you prefer — choose and document), produces a
  **deterministic, order-independent** `String` signature of the cycle's calls
  (sort before concatenating so two cycles with the same calls in different
  order collide correctly).
  + **test** `tests/circuit_breaker.rs`: two inputs with the same (name,args) in
    different order produce the **same** signature; changing a single arg
    produces a **different** signature.
- **R3 — Breaker wired into Agent::run.** `Agent::new` takes a
  `stall_threshold: u32` (add the parameter, after `max_steps`); `run()` builds
  a `StallDetector`; after `stream_turn` (once you have `calls`), compute the
  signature and call `observe`; if `Tripped`, emit
  `tx.send(ToolPart::LoopHalted { reason, steps })` with a stagnation `reason`
  and `steps` = current iteration index, then return `Ok(())` WITHOUT panicking.
  Normal behavior (exit on `calls.is_empty()`, or `BudgetTracker` halt) stays
  unchanged. The breaker must NOT trip when tool calls change between cycles.
  Update the doc-test / call-sites that invoke `Agent::new` with the new
  signature (there is a doctest in `request_body()` and several `tests/*` — find
  and update them all).
  + **test** `tests/circuit_breaker.rs` (integration): an `Agent` with a low
    `stall_threshold` (e.g. 2) and a `max_steps` high enough not to interfere,
    `approval_tx: None` (headless), pointed at a fake HTTP server that ALWAYS
    replies with the **same identical** `tool_calls` (same name + args, never
    `finish_reason:"stop"`), must terminate due to **stagnation** after the
    threshold repeats and emit a `ToolPart::LoopHalted` whose `reason` indicates
    stagnation (assert the text contains "stall"/"stalled"). Reuse the
    fake-server helper from the previous micro-task's tests if it exists (search
    with `nxm-memory` `search_code "LoopHalted"` / `tests/bounded.rs`).
- **R4 — External config.** In `src/config.rs`, add to `TuiConfig` the field
  `pub stall_threshold: Option<u32>` with `#[serde(default)]`, initialized to
  `None` in `Default` (the default applies at the use site via
  `.unwrap_or(DEFAULT_STALL_THRESHOLD)`, so `circuit_breaker.rs` stays the single
  source of the default — same scheme as `max_steps`). In `src/main.rs`, at the
  `Agent::new` call site, read
  `config.stall_threshold.unwrap_or(nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD)`
  and pass it.
  + **test**: extend `tests/config.rs` with a `toml` round-trip of a `TuiConfig`
    with `stall_threshold: Some(5)` (serialize→deserialize, value survives) and a
    toml WITHOUT the field loading `None` (thanks to `#[serde(default)]`).

## Work branch
- Create and work on a dedicated branch: `work/loopeng-01-02-circuit-breaker`.
- Do NOT commit to `main`/develop. The work must be **validated before** merge.

## Available tools (use if needed)
- `nxm-mcp-exec` — validate the Rust code by passing `repo_path` (NOT `cargo`
  from shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`),
  `cargo_fmt_check`, `cargo_test`. **Before declaring "done/mergeable", call
  `run_gate` and proceed only on `verdict:"GO"`.** NOTE: on `main` there is a
  pre-existing NO-GO for ~20 unformatted `src/md/*` files, **not yours and out
  of scope** — verify YOUR files are fmt-clean (`cargo_fmt_check` lists offending
  files: yours must NOT appear). Fall back to `cargo` if the server is
  unavailable.
- `nxm-memory` — search rules/context/precedents (`bounded.rs` patterns,
  fake-server helper in `tests/bounded.rs`, `ToolPart` conventions).
- `nxm-decision` — categorize any implementation forks.
- `nxm-session` — read/update session `loopeng-01` state if relevant.

## Prohibitions
- Do NOT merge the breaker with `BudgetTracker`: they are two distinct controls.
- Do NOT add new `ToolPart` variants (reuse `LoopHalted`).
- Do NOT implement resume/ask-to-continue, Ralph loop, `run_gate` integration,
  token budget, context reset/compaction: out of scope.
- Do NOT let the `Agent` touch `App`, the filesystem or git at runtime.
- Do NOT add new inline `#[cfg(test)]` in `src/agent.rs`.
- Do NOT touch `src/md/*` files (pre-existing fmt debt, not this task).
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
# Your files must NOT appear among cargo_fmt_check offending files.
# Shell fallback:
cargo fmt --check -- src/circuit_breaker.rs src/agent.rs src/config.rs src/main.rs src/lib.rs
cargo clippy --all-targets -- -D warnings
cargo test --test circuit_breaker
cargo test            # full suite, no regressions
```
