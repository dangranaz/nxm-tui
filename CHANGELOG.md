# Changelog
# 2026-10-26

## feat(agent): bounded execution — hard iteration cap on `Agent::run` (loopeng-01-01)

`Agent::run` was an unbounded `loop {}`: it exited only when the model stopped
calling tools. It now has a configurable hard cap on loop iterations (default
`15`): when reached, the loop hard-stops, emits a new `ToolPart::LoopHalted`
towards the UI and returns `Ok(())` without panicking. What counts is model
turns (not tool calls / tokens — token budget is a later phase). No resume
tool / ask-to-continue here: that is loopeng-01-02.
- `src/bounded.rs` (new): pure cap logic — `DEFAULT_MAX_STEPS: u32 = 15`,
  `BudgetTracker::new(max_steps)` + `tick() -> Budget::Continue |
  Budget::Halted { steps }`. Zero I/O; `tracing` on `nexum::bounded::*`;
  doc `///` + doctest on both public fns.
- `src/tool_types.rs`: `ToolPart::LoopHalted { reason, steps }` variant
  (derives unchanged: Debug, Clone, PartialEq, Eq) + `render_lines()` branch
  (one yellow warning line with reason and step count).
- `src/agent.rs`: `Agent::new` gains `max_steps: u32`; `run()` ticks the
  budget each iteration and on `Halted` emits `ToolPart::LoopHalted` and
  returns `Ok(())` (hard stop). Module doc + `request_body()` doctest
  updated to the new signature. `#[allow(clippy::too_many_arguments)]` on
  `Agent::new` (9-arg spawn-time snapshot constructor).
- `src/main.rs`: main loop mirrors `LoopHalted` into `app.messages` + status
  line; passes `cfg.max_steps.unwrap_or(bounded::DEFAULT_MAX_STEPS)` to
  `Agent::new`.
- `src/config.rs`: `TuiConfig.max_steps: Option<u32>` with
  `#[serde(default)]`, `Default` = `None` (single source of the default stays
  `bounded.rs`, applied at the call site).
- `src/lib.rs`: `pub mod bounded;`.
- Tests: `tests/bounded.rs` (6 — R1 pure tracker x4, R2 `render_lines`, R3
  integration: fake SSE server answering ALWAYS `tool_calls`, `max_steps: 2`
  → run terminates after exactly 2 turns emitting one `LoopHalted`, guarded
  by `tokio::time::timeout` so a missing cap fails instead of hanging);
  `tests/config.rs` (2 — toml round-trip `Some(7)`, legacy toml without the
  key loads `None`). All existing `Agent::new` call sites updated (7 test
  files + `main.rs`).
- NOT touched: resume tool / ask-to-continue, circuit breaker, token budget,
  `run_gate` integration, `src/md/*`.
- Gate: `cargo test` 59 binaries green (incl. 49 doctests); clippy 0 NEW
  warnings (4 pre-existing on main remain: `agent.rs` plan-mode filter
  `.retain()`, `app.rs` complex type, `app.rs`/`metrics.rs` `Default`);
  `cargo fmt --check` ok. Branch `work/loopeng-01-01-bounded-execution`,
  pending external review before merge.

## feat(agent): circuit breaker on stagnation in `Agent::run` (loopeng-01-02)

`Agent::run` hard-stopped solo sul cap delle iterazioni (`bounded`): un
modello che ripete la stessa tool call poteva girare fino a esaurire il
budget. Il loop ora trippa anche sulla stagnazione: la stessa *signature*
di tool call (nome + args, indipendente dall'ordine) per N cicli
consecutivi (default 3) emette `ToolPart::LoopHalted` e restituisce
`Ok(())` senza panicare. Complemento del cap: il cap limita la durata
totale, il breaker interrompe un ciclo bloccato prima che bruci il budget.
Niente resume / ask-to-continue (micro-task separato).
- `src/circuit_breaker.rs` (new): logica pura dello stall —
  `DEFAULT_STALL_THRESHOLD: u32 = 3`,
  `StallDetector::new(threshold)` +
  `observe(signature) -> Stall::Ok { repeats } |
  Stall::Tripped { repeats, signature }` (nomi per spec), e
  `signature(calls)` puro (deterministico, indipendente dall'ordine).
  Zero I/O; `tracing` su `nexum::circuit_breaker::*`; doc `///` + doctest
  sulle API pubbliche.
- `src/bounded.rs`: `BudgetTracker::steps()` accessor (con tracing +
  doc example) così il breaker riporta l'indice di iterazione corrente
  senza accedere al campo privato del tracker (moduli separati).
- `src/agent.rs`: `Agent::new` guadagna `stall_threshold: u32` (dopo
  `max_steps`); `run()` costruisce uno `StallDetector` fresco all'ingresso
  (per spec) e, dopo `stream_turn`, calcola la signature e su `Tripped`
  emette `ToolPart::LoopHalted { reason, steps }` e hard-stoppa con
  `Ok(())`. Uscita normale (calls vuote / halt del budget) invariata.
- `src/tool_types.rs`: NON toccato — si riusa l'esistente
  `ToolPart::LoopHalted { reason, steps }` (nessun nuovo variant).
- `src/agent.rs`: `Agent::new` guadagna `stall_threshold: u32` (dopo
  `max_steps`); `run()` calcola la signature dopo `stream_turn` e, su
  `Tripped`, emette `ToolPart::LoopHalted { reason, steps }` e hard-stoppa
  con `Ok(())`. Uscita normale (calls vuote / halt del budget) invariata.
  Module doc + doctest `request_body()` aggiornati alla nuova firma.
  `#[allow(clippy::too_many_arguments)]` mantenuto (costruttore a 10 arg).
- `src/main.rs`: passa
  `cfg.stall_threshold.unwrap_or(circuit_breaker::DEFAULT_STALL_THRESHOLD)`
  a `Agent::new`; il main loop specchia già `LoopHalted` in messaggi +
  status line (nessun cambio necessario).
- `src/config.rs`: `TuiConfig.stall_threshold: Option<u32>` con
  `#[serde(default)]`, `Default` = `None` (unica fonte del default resta
  `circuit_breaker.rs`, applicata al call site).
- `src/lib.rs`: `pub mod circuit_breaker;`.
- Test: `tests/circuit_breaker.rs` (8 — R1 puro detector x4 incl. reset su
  signature diversa e soglia-1 degenera, R2 signature ordine-indipendente /
  cambio di arg, R3 integrazione: server SSE fake che risponde SEMPRE con
  lo stesso `tool_calls`, `stall_threshold: 2`, `max_steps: 50` → termina
  per stagnazione dopo 2 turni emettendo una `LoopHalted` con "stall" nel
  reason, protetta da `tokio::time::timeout`); `tests/config.rs` (+2 —
  toml round-trip `Some(5)`, toml legacy senza la chiave carica `None`).
- Tutti i call-site `Agent::new` aggiornati (7 file di test + `main.rs` +
  doctest).
- NON toccato: resume tool / ask-to-continue, fusione col
  `BudgetTracker`, `run_gate` integration, token budget, `src/md/*`
  (debito fmt pre-esistente, lasciato intatto).
- Gate: `cargo test` green (52 passati, 0 falliti, incl. doctest);
  clippy 0 NUOVI warning sul codice toccato (4 pre-existing su main
  restano in `metrics.rs`/`agent.rs`/`app.rs`);
  `cargo fmt --check` ok sui file toccati.
  Branch `work/loopeng-01-02-circuit-breaker`, pending external review
  prima del merge.

## feat(ui): continue-after-halt via `/continue` command (loopeng-01-03)

Dopo un hard-stop (cap iterazioni o breaker di stagnazione) il loop termina e
l'utente non ha modo di ripartire se non reinviare un messaggio. Aggiunto il
comando UI `/continue` (alias `/cont`) che, solo dopo un halt, ri-spawn un
nuovo `Agent` con il transcript corrente (`app.messages`) e budget fresco
(cap e breaker ripartono da zero grazie al ri-spawn esistente).
- `src/commands.rs`: nuova variant `Command::Continue`; `parse_command`
  riconosce `/continue` e `/cont`.
- `src/app.rs`: nuovo campo `pub halted: bool` (default `false`);
  `mark_halted()` lo imposta quando la UI consuma `ToolPart::LoopHalted`;
  `continue_loop() -> bool` riprende solo dopo un halt (stato → `Thinking`,
  `pending_message` valorizzato, `halted` azzerato) ed è no-op con messaggio
  di stato altrimenti. Doc `///` + doctest su `continue_loop`.
- `src/handler.rs`: ramo `Command::Continue` che delega a
  `app.continue_loop()`.
- `src/main.rs`: nel ramo `ToolPart::LoopHalted` chiama `app.mark_halted()`;
  nel blocco "Start inference" azzera `app.halted = false` quando parte una
  nuova inferenza (invio utente o `/continue`).
- NON tocca `Agent`/`bounded.rs`/`circuit_breaker.rs`: il budget fresco arriva
  gratis dal ri-spawn. NON duplica la logica di spawn: riusa il blocco
  "Start inference" tramite `pending_message` + `RunState::Thinking`. NON è
  un tool del modello (paradosso evitato: dopo l'halt il modello non gira).
- Test: `tests/continue_after_halt.rs` (5 — R1 parsing `/continue`/`cont`,
  R2 default `false` + `mark_halted`, R3 resume dopo halt e no-op senza
  halt). Suite completa green, nessuna regressione.
- Gate: `cargo test` green; clippy 0 NUOVI warning sui file toccati
  (2 pre-existing su `app.rs`: `type_complexity` + `new_without_default`);
  `cargo fmt --check` ok sui file toccati.
  Branch `work/loopeng-01-03-continue-after-halt`, pending external review
  prima del merge.

# 2026-10-21

## feat(plan): add enter/exit_plan_mode + write-abstaining manifest filter

Introduce plan mode: when ON, the Agent mechanically abstains from write tools
(Edit, Web) and can only analyze and produce a task list via the existing
`todo` tool. Two tools `enter_plan_mode`/`exit_plan_mode` toggle the flag;
the state is shown in the mode_bar; the abstention is mechanical (manifest
filter in `Agent::request_body`). Same shared-state pattern as `todo`/`goal`.
- `src/plan_mode.rs` (new): `PlanModeHandle = Arc<AtomicBool>` type +
  `plan_mode_handle()` factory with doc `///` + doctest.
- `src/enter_plan_mode.rs` (new): `struct EnterPlanMode { goal: PlanModeHandle }`
  implementing `nxm_tui_tools::Tool`. Sets flag to `true`. `kind() == Other`,
  keywords `plan, analyze, mode`. Doc `///` + doctest.
- `src/exit_plan_mode.rs` (new): `struct ExitPlanMode { goal: PlanModeHandle }`
  implementing `nxm_tui_tools::Tool`. Sets flag to `false`. `kind() == Other`,
  keywords `plan, exit, normal`. Doc `///` + doctest.
- `src/agent.rs`: `Agent` gains `plan_mode: PlanModeHandle` field (param in
  `Agent::new`). `request_body()` filters out `search_replace`/`web_fetch`/
  `web_search` (kind Edit/Web) from the manifest when plan mode is ON, by
  looking up each entry's name→kind via `registry.get(name).kind()`.
  `select`/`manifest_for` semantics unchanged. `tracing::info!` on request.
- `src/app.rs`: `App` creates one `plan_mode_handle`, clones it into the
  `enter_plan_mode`/`exit_plan_mode` tools registered in `tool_registry`,
  keeps it in `App.plan_mode_handle` for the Agent and mode_bar.
- `src/mode_bar.rs`: `render` appends ` 🅿️ PLAN` when `plan_mode_handle` is
  ON; nothing when OFF.
- `src/lib.rs`: `pub mod plan_mode;`, `pub mod enter_plan_mode;`,
  `pub mod exit_plan_mode;`.
- `src/main.rs`: passes `app.plan_mode_handle` to `Agent::new()`.
- Test: `tests/plan_mode.rs` (7 tests: `plan_mode_tools_schema` R1,
  `plan_mode_enter_exit` R2, `plan_mode_shares_state` R3,
  `request_body_plan_mode_excludes_writes` R4,
  `request_body_plan_mode_off_unchanged` R5, `mode_bar_shows_plan` +
  `mode_bar_no_plan_when_off` R6).
- NOT touched: `agent.rs` base `select`/`manifest_for` semantics, `todo`,
  `update_goal`, `src/md/`; no new dependency.
- Gate: `cargo test --test plan_mode` 7/7; `cargo test --workspace` green;
  `cargo clippy --all-targets` 0 new warnings on plan_mode code;
  `cargo fmt -- --check` ok.
- Branch `work/41-plan-mode` on `feat/tools-plan-mode`; to consolidate with
  `--ff-only` after external review. No commit on `main`/`develop`.

## feat(goal): add update_goal tool + current goal in mode_bar

Expose `update_goal` tool that sets the agent's current goal (single string) in
shared state (`App.goal_handle`, `Arc<Mutex<Option<String>>>`) and shows it
inline in the mode bar (no overlay/key). Same shared-state pattern as `todo`.
- `src/goal.rs` (new): `GoalHandle` type + `goal_handle()` factory with doc
  `///` + doctest; `tracing::debug!` on creation.
- `src/update_goal.rs` (new): `struct UpdateGoal { goal: GoalHandle }`
  implementing `nxm_tui_tools::Tool`. Schema: `goal` (string, required).
  `run` sets `Some(text)`, returns `ToolResult::ok` with confirmation; empty /
  missing `goal` → `ToolResult::err`, state unchanged; lock poisoning handled
  without `unwrap`/`panic`. `kind() == Other`, keywords
  `goal, objective, obiettivo, target, aim, plan`. `tracing::info!` on `run`
  logging only `goal_len` (never the full goal content, privacy). Doc `///` +
  doctest on `name`/`description`/`schema`/`run`.
- `src/app.rs`: `App` creates one `goal_handle`, clones it into the
  `UpdateGoal` tool registered in `tool_registry`, keeps it in
  `App.goal_handle` for the mode bar.
- `src/mode_bar.rs`: `render` appends ` 🎯 <goal>` (truncated to ~40 col with
  `…`) only when the goal is `Some` and non-empty; nothing when `None`.
- `src/lib.rs`: `pub mod goal;` + `pub mod update_goal;`.
- Test: `tests/update_goal.rs` (7 tests: `update_goal_schema` R1,
  `update_goal_sets` R2, `update_goal_empty` + `update_goal_missing` R3,
  `update_goal_shares_state` R4, `mode_bar_shows_goal` +
  `mode_bar_no_goal_when_none` R5).
- NOT touched: `agent.rs`, `nxm-tui-tools`, `src/md/`, `Todo`/`todo`; no new
  dependency; `select` classification `Other` does not change existing
  selection behavior (no keyword collision with Core tools).
- Gate: `cargo test --test update_goal` 7/7; `cargo test --workspace` green;
  `cargo clippy --all-targets` 0 new warnings; `cargo fmt -- --check` ok.
- Branch `work/40-update-goal` on `feat/tools-update-goal`; to consolidate
  with `--ff-only` after external review. No commit on `main`/`develop`.

## feat(todo): add todo overlay (TUI, read-only) (MT-3)

Read-only todo overlay for the agent's todo list. Shows `App.todo_handle`
(the shared `Arc<Mutex<TodoList>>` populated by the `todo` tool in MT-2) as
a `[x]`/`[ ]` checklist. No UI mutation in this MT — the agent mutates todos
via the tool; the TUI only observes.
- `src/todo_overlay.rs` (new): `render_todo(f, app)` renders the todo list
  with `[x]`/`[ ]` markers and a "Todo" title; empty list shows
  "(no todos)" placeholder; poisoned mutex shows an error placeholder
  instead of panicking. `handle_todo_key` swallows all keys while modal.
- `src/lib.rs`: `pub mod todo_overlay;`.
- `src/app.rs`: added `pub show_todo: bool` (default `false`) to `App`.
- `src/ui.rs`: `render()` draws `render_todo` when `app.show_todo` is true.
- `src/handler.rs`: Ctrl+D toggles the todo overlay; Esc closes it (Ctrl+D
  is free — not used by any other overlay/action).
- Test: `tests/todo_overlay.rs` (4 tests: `todo_overlay_hidden_by_default`
  R1, `todo_overlay_renders_items` R2, `todo_overlay_empty_placeholder` R3,
  `todo_overlay_poisoned_mutex_does_not_panic` bonus),
  `tests/todo_overlay_keys.rs` (3 tests: `todo_overlay_toggle_key` R4,
  `todo_overlay_toggle_key_idempotent` R4, `todo_overlay_no_key_conflict`
  R5).
- NOT touched: `agent.rs`, `nxm-tui-tools`, `src/md/`, `TodoList`/`Todo`
  (MT-1/MT-2); no new dependency.
- Gate: `cargo test --test todo_overlay` 4/4; `cargo test --test
  todo_overlay_keys` 3/3; `cargo test --workspace` green; `cargo clippy
  --all-targets` 0 new warnings; `cargo fmt` on touched files ok.
- Branch `work/39-todo-mt3` on `feat/tools-todo`; to consolidate with
  `--ff-only` after external review. No commit on `main`/`develop`.


## feat(todo): add todo tool mutating shared TodoList (MT-2)

Expose a `todo` tool that mutates the shared `Arc<Mutex<TodoList>>` state
(from MT-1) and returns `render()` to the model. No UI/overlay yet (MT-3).
- `src/todo_tool.rs` (new): `struct Todo { todos: TodoListHandle }`
  implementing `nxm_tui_tools::Tool`. Schema: `action` (required) + optional
  `text`/`index`. Actions: `add`, `complete` (by index or text), `list`.
  Missing/unknown action → `ToolResult::err`. Mutex lock poisoning handled
  without `unwrap`/`panic`. `tracing::info!` (target `nexum::todo::tool`) on
  `run` (action + outcome). Classified `kind() == Other` with keywords
  `todo, task, plan, checklist, lista, piano` so per-task `select` includes it
  on planning tasks.
- `src/lib.rs`: `pub mod todo_tool;`.
- `src/app.rs`: `App` creates one `TodoListHandle` (via `todo_list_handle()`),
  clones it into the `todo` tool registered in `tool_registry`, and keeps the
  handle in `App.todo_handle` for the MT-3 UI. Same handle shared App↔tool.
- Test: `tests/todo_tool.rs` (7 tests: `todo_tool_schema` R1,
  `todo_tool_add` R2, `todo_tool_complete` + `todo_tool_complete_invalid` R3,
  `todo_tool_list` R4, `todo_tool_bad_action` R5,
  `todo_tool_shares_state_with_handle` R6).
- NOT touched: `agent.rs`, overlay, `nxm-tui-tools`, `src/md/`, no new
  dependency, no change to existing tool behavior or per-task selection
  (`Todo` is not in `default_registry()`, so existing selection tests are
  unaffected).
- Gate: `cargo test --test todo` 7/7; `cargo test --test todo_tool` 7/7;
  `cargo test --workspace` green; `cargo clippy --all-targets` 0 new warnings;
  `cargo fmt` on touched files ok.
- Branch `work/38-todo-mt2` on `feat/tools-todo`; no commit on `main`/`develop`.


## feat(todo): add TodoList + shared state (MT-1, types & logic only)

Todo-list types and shared state for the agent, pure testable logic,
WITHOUT exposing a tool or touching the UI (MT-2 = tool, MT-3 = overlay).
- `src/todo.rs` (new, ~130 lines ≤ 300 cap): `TodoItem { text, done }` and
  `TodoList` (wrapper over `Vec<TodoItem>`) with `add(text)`,
  `complete_by_index(index)` / `complete_by_text(text)` (domain errors,
  no panic), `list() -> &[TodoItem]`, `render() -> String`
  (`- [x]` / `- [ ]`, registration order, deterministic). Typed `TodoError`
  (`IndexOutOfRange` / `NotFound`) without `thiserror`
  (not a workspace dependency; `std::error::Error` implemented by hand).
  `tracing::debug!` logging (target `nexum::todo::state`) on `add`/`complete_*`.
- `src/lib.rs`: `pub mod todo;`.
- Shared state: `TodoListHandle = Arc<Mutex<TodoList>>` + `todo_list_handle()`
  (same pattern as `tool_registry`, shared App↔Agent).
- Test: `tests/todo.rs` (7 tests: `todo_add_appends` R1,
  `todo_complete_by_index` + `todo_complete_out_of_range` R2,
  `todo_complete_by_text` R3, `todo_render_format` R4,
  `todo_complete_idempotent` (done idempotent edge case),
  `todo_shared_state_via_arc_mutex` R5).
- NOT touched: `agent.rs`, overlay, `nxm-tui-tools`, `src/md/`, no new
  dependency, no new tool in the registry (R6).
- Gate: `cargo test --test todo` 7/7; `cargo test --workspace` green;
  `cargo clippy --all-targets` 0 new warnings; `cargo fmt` on touched files ok.
- Branch `work/37-todo-mt1` on `feat/tools-todo` (from `main`); no commit on
  `main`/`develop`.


## fix(agent): do not log user message content in request_body (privacy)

`request_body` logged `task = %task` at `info` — i.e. the full user message
content on every request. Replaced with `task_len` (length only): diagnostics
preserved, no user content in logs. Verified via a `tracing` subscriber test
asserting a secret in the user message never reaches the log output.

## feat(tools): activate per-task tool selection — manifest_for wired, manual set empty by default (MT-3)

Switch atomico: attiva la selezione automatica per-task end-to-end (Opzione X).
- `nxm-tui-tools/src/lib.rs`: `register()` non abilita più automaticamente; il set manuale `enabled` parte vuoto. Doc-comment aggiornati (niente più "enabled by default").
- `src/agent.rs`: `request_body()` usa `registry.manifest_for(&task)` con `task` = ultimo messaggio `Role::User` del transcript (stringa vuota → fail-safe = soli Core). Doc-comment/doctest aggiornati (ora 2 tool = Core con transcript vuoto). Logging `tracing::info!` (target `nexum::agent::request`) su tool inviati e task.
- `src/tools_overlay.rs` / `src/handler.rs`: meccanismo invariato (`is_enabled`/`set_enabled`/`enable_all`/`disable_all`), ora di default tutti `[ ]` (l'utente *aggiunge* via overlay); nessun cambio di codice UI necessario oltre ai test.
- Test aggiornati alla nuova semantica: `nxm-tui-tools/tests/selection.rs` (`default_manual_set_is_empty`, `registering_a_tool_no_longer_auto_enables_it`, `manifest_for_default_registry_is_fail_safe_core_only` + i `set_enabled_*` riscritti), `grep`/`web_fetch`/`search_replace` `*_in_default_registry` (verifica via `tool_names()`), `tests/agent_request.rs` (`request_body_failsafe_core_only`, `request_body_uses_per_task_selection`, `request_body_includes_manual_addition`), `tests/tools_overlay_keys.rs` + `tests/tools_overlay.rs` (default `[ ]`, toggle che aggiunge).
- `manifest_selected()` mantenuto (ancora usato dai test); `agent.rs` non lo chiama più.
- NON toccate: `select`/`manifest_for` (MT-2), `manifest()`, default di `register()` oltre al cambio `enabled`, `src/md/`, nessuna nuova dipendenza.
- Gate: `cargo test -p nxm-tui-tools` verde; `cargo test --workspace` verde; `cargo clippy --all-targets` 0 nuovi warning; `cargo fmt -p nxm-tui-tools -- --check` + file nxm-tui toccati formattati.
- Branch `work/35-select-mt3` su `feat/tools-select-mechanism`; nessun commit su `main`/`develop`.


## feat(tools): add Registry::select + manifest_for per-task selection (MT-2)

Selezione automatica per-task in `nxm-tui-tools` come logica pura e testabile,
SENZA cambiamenti a runtime (`agent.rs`/overlay/default `enabled` restano a MT-3;
`manifest()`/`manifest_selected()` invariati).
- `nxm-tui-tools/src/selection.rs` (nuovo, ~115 righe ≤ cap 300): `select(registry, task)`
  include sempre i `ToolKind::Core` (`read_file`, `list_resources`) più i tool con
  almeno una `keywords()` contenuta come substring case-insensitive in `task`
  (`task.to_lowercase().contains(&kw.to_lowercase())`); fail-safe = solo Core quando
  nessun non-Core matcha; ordine = registrazione. `manifest_for(registry, task)` =
  unione `select(task) ∪ {tool abilitati}` per nome, niente duplicati, ordine di
  registrazione. Logging `tracing::debug!` (target `nexum::tools::runtime`) su quali
  tool auto-selezionati + se fail-safe.
- `nxm-tui-tools/src/lib.rs`: `mod selection`; `pub fn Registry::select` e
  `pub fn Registry::manifest_for` che delegano a `selection`, doc `///` con esempio
  doctest ciascuno.
- Test: `nxm-tui-tools/tests/selection.rs` (6 nuovi: `select_always_includes_core`
  R1, `select_keyword_case_insensitive` R2, `select_failsafe_core_only` R3,
  `manifest_for_unions_auto_and_manual` R4, `manifest_for_dedup_and_order` R5,
  `manifest_for_default_registry_returns_all_until_mt3` R6); isolamento della parte
  automatica con `disable_all()` + `set_enabled(...)` mirati. R6 è verifica sui
  test esistenti invariati, non un nuovo test.
- NON toccate: `agent.rs`, overlay, default di `enabled`/`register()`,
  `manifest()`/`manifest_selected()`, `src/md/`; nessuna nuova dipendenza.
- Gate: `cargo test -p nxm-tui-tools` verde; `cargo test --workspace` verde;
  `cargo clippy --all-targets` 0 nuovi warning; `cargo fmt -p nxm-tui-tools -- --check` ok.
- Branch `work/34-select-mt2` su `feat/tools-select-mechanism`; nessun commit su
  `main`/`develop`.


## feat(tools): add search_replace tool (exact unique find→replace)

Nuovo tool nativo `search_replace` in `nxm-tui-tools`: dato `path`, `old`,
`new`, sostituisce l'occorrenza **esatta e univoca** di `old` con `new` e
riscrive il file in modo **atomico** (tmp + rename same-dir). Registrato in
`default_registry()` (abilitato di default).
- `nxm-tui-tools/src/search_replace.rs` (nuovo, ~110 righe ≤ 300 cap):
  `SearchReplace` tool con schema `path`+`old`+`new` required (tutte string);
  exact-string (NON regex); `old` vuoto → errore; `old == new` → errore "no change";
  `old` 0 volte → `ToolResult::err("not found")`, nessuna scrittura;
  `old` >1 volta → `ToolResult::err("ambiguous: N matches")`, nessuna scrittura;
  `old` esattamente 1 volta → sostituzione + write atomico (temp file nella
  stessa directory + `std::fs::rename`); logging `tracing` su `run` (target
  `nexum::tools::runtime`); doc `///` con esempio doctest.
- `nxm-tui-tools/src/lib.rs`: `mod search_replace`, re-export `SearchReplace`,
  registrazione in `default_registry()`.
- Test: `nxm-tui-tools/tests/search_replace.rs` (8 test: `sr_schema`,
  `sr_missing_args` (3 casi), `sr_unique_replace`, `sr_no_match_no_write`,
  `sr_ambiguous_no_write`, `sr_empty_old`, `sr_no_temp_left`,
  `sr_in_default_registry`); `tempfile` per alberi temporanei, verifica
  contenuto riletto da disco, assenza file temporanei orfani.
- `nxm-tui-tools/tests/tools.rs`, `tests/selection.rs`, `tests/agent_request.rs`,
  `src/agent.rs`: conteggio manifest aggiornato a 6 tool + doctest.
- Gate: `cargo test -p nxm-tui-tools --test search_replace` 8/8; `cargo test --workspace`
  verde; `cargo clippy --all-targets` 0 nuovi warning (5 pre-esistenti);
  `cargo fmt -- --check` 0 diff sui file toccati.
- Branch `work/31-tools-search-replace` su `feat/tools-search-replace`; nessun commit
  su `main`/`develop`.


# 2026-10-20

## feat(tools): add ToolKind + kind()/keywords() taxonomy (MT-1, no behavior change)

Groundwork della selezione per-task (TOOLS-1 Parte A, MT-1): tassonomia dei tool
in `nxm-tui-tools` SENZA cambiamenti osservabili — `manifest()` e
`manifest_selected()` producono lo stesso output di prima.
- `nxm-tui-tools/src/taxonomy.rs` (nuovo, 43 righe ≤ cap 300): `enum ToolKind`
  (`Read`/`Search`/`Edit`/`Core`/`Web`/`Other`, derive `Debug, Clone, Copy,
  PartialEq, Eq`), doc `///` con esempio doctest. `Read` è riservato a futuri
  tool di sola lettura NON nel fail-safe (nessun tool built-in lo usa ora: i
  read/navigate sono `Core`).
- `nxm-tui-tools/src/lib.rs`: `mod taxonomy` + re-export `ToolKind`; due metodi
  default sul trait `Tool` — `kind() -> ToolKind` (default `Other`) e
  `keywords() -> &[&str]` (default `&[]`), entrambi con doc `///` + doctest.
  I tool futuri compilano senza override.
- Classificazione decisa (decisioni utente 2026-10-20): `read_file`/`list_resources`
  = `Core` (sempre inclusi dall'automatica + set fail-safe; `keywords()` vuoto —
  non dipendono dal match); `grep` = `Search`; `search_replace` = `Edit`;
  `web_fetch`/`web_search` = `Web`. Keywords EN+IT lowercase (match
  case-insensitivo a partire da MT-2; esempi per tool nel commit/ticket
  `prompts/33-*`); override `kind()`/`keywords()` direttamente nei blocchi
  `impl Tool` di ciascun tool (`builtins.rs`/`grep.rs`/`search_replace.rs`).
- NON implementate: `select(task)`/`manifest_for(task)`/fail-safe/unione
  automatica∪manuale, `agent.rs` — sono MT-2/MT-3.
- Test: `nxm-tui-tools/tests/taxonomy.rs` (5: `kind_defaults` con dummy tool
  senza override, `core_tools_classified`, `tools_kinds`,
  `keyword_tools_have_keywords`, `registry_tools_all_kinds_known`); suite
  esistenti (`tools.rs`, `selection.rs`, doctest) verdi SENZA modifiche —
  nessun bump di conteggi manifest.
- Gate: `cargo test -p nxm-tui-tools` verde (0 warning lib; i warning
  pre-esistenti in `tests/web_fetch_ssrf.rs` restano invariati);
  `cargo test --workspace` verde; `cargo clippy --all-targets` 0 nuovi warning;
  `cargo fmt -- --check` 0 diff sui file toccati (i diff pre-esistenti in
  `src/md/*.rs` — file verbatim-copy — sono su `main` e fuori scope).
  `run_gate` (nxm-mcp-exec): verdict NO-GO **solo** per `cargo_fmt_check` su
  20 file `src/md/*.rs` pre-esistenti — set di file difformi **identico** su una
  copia pulita di `main` (verificato con `git archive`): NO-GO è la baseline,
  non introdotto da questo cambio; `src/md` intoccabile per Divieti →
  eccezione documentata e rimessa alla revisione esterna.
- Branch `work/33-select-mt1` su `feat/tools-select-mechanism`; nessun commit
  su `main`/`develop`; da consolidare nel feature branch dopo review.


# 2026-10-20

## test(tools): serialize SSRF env-var tests to fix flakiness and 30s timeout

La suite `nxm-tui-tools/tests/web_fetch_ssrf.rs` impiegava ~30s invece di
~0.03s: i test girano in parallelo e `with_allow_local` (set/remove di
`NEXUM_WEBFETCH_ALLOW_LOCAL`, variabile globale di processo) poteva
contaminare un test di blocco in esecuzione contemporanea → guardia SSRF
saltata → fetch reale a indirizzo bloccato → timeout TCP ~30s. Il
doc-comment del file citava già `#[serial]`/`serial_test` ma la dipendenza
non c'era: il commento mentiva.
- `nxm-tui-tools/Cargo.toml`: dev-dependency `serial_test = "3"` (solo test,
  non entra nel binario distribuito).
- `nxm-tui-tools/tests/web_fetch_ssrf.rs`: `#[serial]` (gruppo default) su
  tutti e 7 i test che toccano la env var — sia quelli che la impostano via
  `with_allow_local` (`ssrf_allow_local_env`,
  `ssrf_web_fetch_allow_local_bypasses_guard`) sia quelli di blocco che
  passano da `WebFetch::run` → `fetch_with_ssrf_guard` (che la legge:
  `ssrf_blocks_non_http_scheme`, `ssrf_web_fetch_loopback_blocked`,
  `ssrf_web_fetch_metadata_blocked`, `ssrf_web_fetch_private_ip_blocked`,
  `ssrf_web_fetch_non_http_scheme_blocked`). I test puri su
  `is_blocked_addr`/`validate_url` restano paralleli. Nessun cambio di
  nomi/asserzioni/semantica; nessun tocco al codice di produzione.
- Misurato: prima 30.0s (timeout da contaminazione), dopo 0.05s; 3
  esecuzioni consecutive verdi.

## feat(tools): web_fetch SSRF guard (block private/loopback/metadata + redirects)

Aggiunta guardia SSRF a `WebFetch` (`nxm-tui-tools/src/builtins.rs`):
- Nuovo modulo `nxm-tui-tools/src/ssrf.rs` (~140 righe): `is_blocked_addr`
  classifica indirizzi (loopback, link-local, private-use, metadata
  169.254.169.254, unspecified, IPv6 ULA/link-local); `validate_url` verifica
  schema (solo http/https) + risoluzione host; `fetch_with_ssrf_guard` segue
  redirect manualmente (max 10 hop) validando **ogni hop** con
  `reqwest::Client` + `Policy::none()`; anti-DNS-rebinding: **tutti** gli IP
  risolti per un host devono passare il check.
- Allow-list dev: `NEXUM_WEBFETCH_ALLOW_LOCAL=1` salta la validazione
  dell'URL iniziale (ma valida comunque i redirect).
- `WebFetch::run` usa `fetch_with_ssrf_guard`; errori SSRF → `ToolResult::err`
  con catena cause completa; logging `warn` su `nexum::tools::runtime`.
- Test: `nxm-tui-tools/tests/web_fetch.rs` aggiornati (allow-list per server
  loopback); nuovo `tests/web_fetch_ssrf.rs` (10 test: R1-R7 — loopback,
  metadata, private, schema non-http, redirect→loopback, allow-list, IP
  pubblici, unspecified, IPv6 link-local, validazione schema).
- Gate: `cargo test -p nxm-tui-tools web_fetch` 15/15; `cargo test --workspace`
  verde; `cargo clippy --all-targets` 0 nuovi warning (5 pre-esistenti);
  `cargo fmt -- --check` 0 diff.
- Branch `work/29-web-fetch-ssrf` su `feat/tools-web-fetch-ssrf`; nessun commit
  su `main`/`develop`.

# 2026-10-20

## feat(tools): add grep tool (regex search over files)

Nuovo tool nativo `grep` in `nxm-tui-tools`: dato `pattern` (regex) e `path`
(file o directory), restituisce le righe matchanti come `file:line:testo`.
Registrato in `default_registry()` (abilitato di default).
- `nxm-tui-tools/src/grep.rs` (nuovo, ~150 righe ≤ 300 cap): `Grep` tool con
  schema `pattern`+`path` required; `regex` crate per matching vero;
  pattern invalido → `ToolResult::err` (non panico, non `Err`); argomenti
  mancanti → `anyhow::Err`; directory ricorsiva con esclusioni hard-coded
  `.git/` e `target/`; file binari/non-UTF8 saltati silenziosamente;
  output ordinato per path poi riga (1-based); cap 1000 match con troncatura;
  logging `tracing` su `run` (target `nexum::tools::runtime`); doc `///`
  con esempio doctest.
- `nxm-tui-tools/Cargo.toml`: `regex.workspace = true` (pin esatto
  `1.13.1` già nel workspace); `nxm-tui-tools/src/lib.rs`: `mod grep`,
  re-export `Grep`, registrazione in `default_registry()`.
- Test: `nxm-tui-tools/tests/grep.rs` (6: `grep_schema`, `grep_missing_args`,
  `grep_invalid_regex`, `grep_single_file`, `grep_directory_excludes`,
  `grep_in_default_registry`); alberi di file temporanei via `tempfile`.
- `nxm-tui-tools/tests/tools.rs`, `tests/selection.rs`, `tests/agent_request.rs`,
  `src/agent.rs`: conteggio manifest aggiornato a 5 tool + doctest.
- Gate: `cargo test -p nxm-tui-tools --test grep` 6/6; `cargo test --workspace`
  verde; `cargo clippy --all-targets` 0 nuovi warning; `cargo fmt -- --check`
  0 diff sui file toccati.
- Branch `work/28-tools-grep` su `feat/tools-grep`; nessun commit su
  `main`/`develop`.

# 2026-10-19

## chore(fmt): format the whole repo (CHORE1)

Applicato `cargo fmt` a tutto il codice nostro (63 file `src/*.rs` + `tests/*.rs`
+ 2 file `src/md/` nostri). I 20 file `src/md/**` con header `//! Derived from`
(copie verbatim di grok-build) sono **esclusi di proposito**: riformattarli
devierebbe dall'upstream (vedi `RULES.md` → Verbatim-copy exemption), complicando
i realign futuri. Dopo questo commit `cargo fmt -- --check` è verde su tutto il
codice nostro; gli unici residui sono i 20 verbatim, per scelta documentata.
Nessun cambiamento di semantica (build + 375 test invariati). Vedi
`docs/wayfinder/tickets/CHORE1-cargo-fmt-repo.md`.

## feat(tools): add web_fetch tool (HTTP GET → text)

Nuovo tool nativo `web_fetch` in `nxm-tui-tools`: dato un argomento `url`,
scarica via HTTP (GET sincrono, `reqwest::blocking`) e restituisce il body
come testo in un `ToolResult`. Nessuna estrazione HTML/markdown, nessuna
policy di sandbox. Registrato in `default_registry()` (abilitato di default).
- `nxm-tui-tools/src/builtins.rs`: `WebFetch` (forma clonata da `WebSearch`:
  schema con `url` required, status non-2xx → `ToolResult::err` con lo status,
  argomento mancante/non stringa → `anyhow::Err`); logging `tracing` su
  `run` (target `nexum::tools::runtime`). File a 170 righe (≤300 cap).
- `nxm-tui-tools/src/lib.rs`: re-export `WebFetch` + registrazione in
  `default_registry()`.
- Test: `nxm-tui-tools/tests/web_fetch.rs` (5: `web_fetch_schema`,
  `web_fetch_missing_url`, `web_fetch_ok`, `web_fetch_http_error`,
  `web_fetch_in_default_registry`); server HTTP grezzo su TCP locale,
  zero nuove dipendenze.
- `nxm-tui-tools/tests/tools.rs`, `tests/agent_request.rs`: conteggio
  manifest aggiornato a 4 tool.
- Gate: `cargo test -p nxm-tui-tools web_fetch` 5/5; `cargo test --workspace`
  verde; `cargo clippy --all-targets` 0 nuovi warning (5 pre-esistenti);
  `cargo fmt -- --check` 0 diff.
- Branch `work/27-tools-web-fetch` su `feat/tools-web-fetch`; nessun commit
  su `main`/`develop`.

# 2026-10-18

## feat(tools): in-memory tool selection + filtered manifest + Tools overlay (T1)

Selezione tool in-memory: al modello va solo il sottoinsieme attivo. Nessun MCP, nessun async (T2), nessuna persistenza (fase 2), `src/md/` intatto.
- `nxm-tui-tools`: `Registry` guadagna `enabled: HashSet<String>` (invariante: tutti attivi al `register()`), `set_enabled`/`is_enabled`/`enable_all`/`disable_all`/`tool_names`/`manifest_selected`; `manifest()` invariato (compat). I tool concreti (`ReadFile`/`ListResources`/`WebSearch`) spostati in `builtins.rs` + re-export alla radice (API invariata) per il cap 300 righe.
- `src/agent.rs`: `Agent::new` riceve la `Registry`; `request_body()` (estratta da `stream_turn`, pura) usa `manifest_selected()`. Un tool disattivato non compare nel body `tools`.
- UI: `App` possiede `tool_registry` + `show_tools`/`tools_cursor`; `main.rs` passa uno snapshot allo spawn; nuovo overlay `tools_overlay::render_tools` (checkbox `[x]`/`[ ]`, lista piatta), Ctrl+E apre (Ctrl+T era sessions), ↑/↓ cursore, Space toggle, `a`/`n` tutti/nessuno, Esc chiude.
- Test: `nxm-tui-tools/tests/selection.rs` (7), `tests/agent_request.rs` (3, incluso il DoD: tool disattivato assente dal body), `tests/tools_overlay.rs` (3, render su TestBackend), `tests/tools_overlay_keys.rs` (5, tasti + seam overlay→body). Doctest sui nuovi metodi Registry.
- Suite verde su tutti i target compilanti: `361 passed / 0 failed / 1 ignorato noto` (`cargo test --workspace`, esclusi i WIP md rotti dell'utente); `cargo clippy` 0 nuovi warning; i miei file a 0 diff `rustfmt --check`. `src/md/` non toccato (i WIP T14a/T14b dell'utente sono preesistenti e non compilano indipendentemente dal mio lavoro).
- Review pass (fix): overlay Tools modale — niente tasto filtra più nel prompt mentre è aperto (contratto ricerca esistente, Esc chiude); il routing tasti esce da `handler.rs` (53 righe in meno) e vive in `tools_overlay::handle_tools_key`; dedup del blocco `json!` del manifest via `manifest_entry`; doctest aggiunti (`tool_names`, `render_tools`, `handle_tools_key`); clamp altezza overlay corretto per terminali <8 righe.
- Branch `work/21-tools-selection` su `df3d690`.

# 2026-10-18

## feat(md): streaming prerequisites — latex normalizer stub + link_id render helper + parser builders (V3/T14a)

Prepara i 3 pezzi richiesti dallo streaming renderer (T14b) senza implementarlo (lo streaming vero resta T14b):
- `src/md/latex.rs`: `LatexDelimiterNormalizer` stub passthrough (`new`/`reset`/`push`/`finish`, `Clone` + `Default`); LaTeX resta stubbato, nessun vero normalizzatore copiato.
- `src/md/mod.rs`: `render_markdown_ratatui_with_link_id` (variante del render che gestisce il link_id counter; firma 9 argomenti imposta dallo spec, `#[allow(clippy::too_many_arguments)]` sul solo punto) + builder `open_code` su `MarkdownParser` (spostato fuori da `parse.rs` per rispettare il cap 300 righe).
- `src/md/parse.rs`: builder `link_id_start` su `MarkdownParser`.
- Test: `tests/md_stream_prereq.rs` (4 test: normalizer passthrough/finish/reset/clone, `link_id_start` usato come primo id, `open_code` chaina, `next_link_id` avanza oltre `link_id_start`).

- `cargo build` ok; suite `328 passed / 0 failed / 1 ignorato noto` (`--test-threads=1`); `cargo clippy --all-targets` = 3 warning baseline (pre-esistenti in `app.rs`/`metrics.rs`), 0 nuovi, 0 `unused`; `src/md/parse.rs` 300 / `latex.rs` 36 / `mod.rs` 139 righe (≤300 cap).
- Branch `work/T14a-md-stream-prereq` su `origin/main` (ecf798b); nessun commit su `main`/`develop`.

# 2026-10-17

## feat(md): wrap render output to pane width (V3/T10)

Closes Fase 2 (wrapping). Our own one-line integration (RULES piene), no
verbatim copy: after `render_ratatui`, `md::render` now applies
`crate::md::wrap_line::word_wrap_lines(output.lines, max_width)` so prose
stays inside the pane (was 1 row of 182 cols at width 40). No
`unwrap`/`expect`; doc comment updated.
- Non-regression by construction + tests: table lines are skipped by the
  wrap engine (`is_table_line`, T9b) — borders `┌─┬┐`/`│ │`/`└─┘` stay
  whole rows (fit_line_to_width pads them to `max_width`, trailing spaces
  invisible); code-fence marker rows (` ``` `) stay whole; long code
  content lines wrap like prose.
- Tests: `tests/md_wrap_integration.rs` (5) — the Fase-2 key test (long
  prose ~180 cols at width 40 → every row ≤ 40), short prose untouched,
  table rows never split (5 border rows intact), code fences whole, styles
  preserved across wrapped rows (heading fg colour carried to every
  continuation row).
- Verify: 324 passed / 0 failed / 1 ignored (single-threaded; ≥319 + new),
  clippy `--all-targets` 3 baseline warnings only, 0 unused,
  `src/md/mod.rs` = 94 lines ≤ cap.

## feat(md): copy word-wrap (V3/T9b)

Derived from xai-grok-pager-render/src/render/wrapping.rs (Apache-2.0).
Pager-render source, production lines 1-690 (upstream inline tests NOT
ported: they need `crate::theme`/`safe_buf`); zero xai-* refs. Copy-only:
wrap is NOT wired into the render pipeline yet (T10).
- Split for the 300-line cap: `src/md/wrap.rs` (byte-range helpers +
  `RtOptions` + relocated `is_table_line`/`blockquote_prefix_len`, made
  `pub(crate)` for the split), `src/md/wrap_line.rs` (word-wrap engine:
  `word_wrap_line`, `word_wrap_line_with_joiners`, `word_wrap_lines(_with_joiners)`,
  span slicing) and `src/md/wrap_input.rs` (`LineInput`/`IntoLineInput`,
  made `pub` for visibility-for-testability, + `wrap_header_hanging`/
  `wrap_header_flush`). All ≤300 lines; split documented in each header.
- Entry points resolve line_utils via `crate::md::line_utils` (T9a:
  `fit_line_to_width`, `push_owned_lines`, `line_to_static`). No
  `unwrap`/`expect` added; upstream non-panicking patterns kept.
- Tests: `tests/md_wrap.rs` (5) — behavioural only: long line wraps into
  rows of width ≤ width, short line unchanged, `word_wrap_lines` wraps
  every input line, styles preserved across wrap, table lines never
  wrapped. No invented upstream-exact strings (lezione T9a).
- Verify: 319 passed / 0 failed / 1 ignored (single-threaded; ≥314 + new),
  clippy `--all-targets` 3 baseline warnings only, 0 unused.

## feat(md): copy line_utils (V3/T9a)

Derived from xai-grok-pager-render/src/render/line_utils.rs (Apache-2.0).
Pager-render source (NOT xai-grok-markdown), production lines 1-209; first
Fase-2 wrapping prerequisite. Zero xai-* workspace refs.
- New `src/md/line_utils.rs` (196 lines ≤ cap): `line_to_static`,
  `push_owned_lines`, `truncate_str`, `truncate_line`, `fit_line_to_width`
  (grapheme-aware clip/pad, no ellipsis), `cascade_truncate` +
  private `take_width`. Bodies verbatim modulo documented substitutions.
- `floor_char_boundary`: NOT redefined — reused via `crate::md::buffers`
  (T4 `pub(crate)` polyfill, same behaviour).
- `truncate_str`/`take_width` backed by `crate::markdown_layout::truncate_to_width`
  (char-boundary, ellipsis-free): upstream's `crate::util::truncate_to_width`
  (+ `byte_offset_at_width`) appends `…` and is xai-* workspace code, both
  intent-consistent with the delegation's acceptance test
  (`truncate_str("日本語テスト", 4)` == "日本", 4 cells); documented in the
  header `Modified:` clause. `truncate_line`'s own `…`-appending logic is
  untouched (it appends the ellipsis itself).
- Upstream `pub use` re-exports of un-ported xai-* modules (tool_paths trio,
  `byte_offset_at_width`, `is_unsafe_display_char`) dropped (noted in header);
  `pub mod line_utils;` registered in `src/md/mod.rs`.
- No `unwrap`/`expect` in production; everything `pub` as upstream.
- Tests: `tests/md_line_utils.rs` (28) — adapted from the upstream inline
  tests (renderer-free port) + delegation contracts: CJK display-cell
  truncation, fit-to-width clip/pad (incl. emoji grapheme straddle), style
  preservation across `truncate_line`/`fit_line_to_width`,
  `push_owned_lines`/'`line_to_static` ownership, `cascade_truncate` matrix
  (recomputed for the ellipsis-free backing; dropped only from truncated
  pieces). Upstream `tool_paths` test dropped (helper not ported).
- Verify: 314 passed / 0 failed / 1 ignored (single-threaded; ≥286 required),
  clippy `--all-targets` 3 baseline warnings only, 0 unused.

## feat(md): wire render pipeline + render_markdown shim behind USE_NEW_MD (V3/T8)
First on-screen rich markdown output (flag-gated). Integration is OUR code
(RULES piene); no new md module. `history.rs` untouched (signature frozen).
- `src/md/mod.rs::render` replaces the T0 placeholder: real pipeline
  `MarkdownParser::new(content, fixed_style(), &mut buffers, syntect.as_ref())`
  → `.max_table_width(Some(max_width))` → `.parse()` →
  `.render_ratatui(true)` → `output.lines`. Fresh `MarkdownBuffers` per call;
  `Syntect::new()` is `Option` and is passed as-is (best-effort highlighting,
  no `unwrap`/`expect`). `max_width` applies to GFM table columns only —
  full prose wrapping lands with T9/T10 (documented divergence).
- `src/markdown.rs::render_markdown` becomes a shim behind the
  `NXM_USE_NEW_MD` env flag (`1|true|yes|on`, case-insensitive; default
  **OFF** for safety — the legacy path stays live; reviewers can flip the env
  var for visual testing). Flag ON: V3 pipeline + prefix re-applied to every
  `Line` (`with_prefix`); flag OFF: unchanged legacy fallback.
- V1 renderer moved verbatim to `src/markdown_legacy.rs` (entry
  `render_markdown_legacy`, pub(crate)); pure layout helpers `wrap`/
  `append_word`/`truncate_to_width` moved to `src/markdown_layout.rs` — both
  splits required by the RULES 300-line cap; behaviour unchanged.
- `tracing::debug!` (target `nexum::markdown::swap`) on both dispatch paths.
- `highlight.rs`, `history.rs`, `src/md/*` (beyond `mod.rs::render`)
  untouched.
- Tests: `tests/md_swap.rs` (6): md::render content baseline, edge-input
  no-panic (empty / whitespace / fence / table), table shrink at max_width,
  shim flag-ON prefix on every line, flag-OFF defaults to legacy, flag
  value "0" stays legacy.
- Verify: 286 passed / 0 failed / 1 ignored (single-threaded), clippy
  `--all-targets` 3 baseline warnings only (0 unused), files ≤300 lines.

## feat(md): render_ratatui — markdown to ratatui Lines (V3/T6d)
Derived from xai-grok-markdown/src/render.rs (Apache-2.0).
- New render files (all `impl ParsedMarkdown`, split per the 300-line cap,
  upstream bodies verbatim modulo mechanical `state.` renames):
  - `src/md/render.rs` (279 lines ≤ cap): `render_ratatui` skeleton
    (preamble, event dispatch, `Highlight` arm) + `emit_trailing_text`
    (upstream trailing-text path) + final line flush + output assembly.
  - `src/md/render_events.rs` (223): `RenderLoopState` (the ~15 loop locals,
    documented grouping), `apply_transforms`, `build_render_events(_into)`,
    `finalize_checkpoint` (upstream checkpoint tail).
  - `src/md/render_segments.rs` (247): `emit_text_ranges` (the main
    text-chunk path: hidden fences, transformed spans, link projection,
    checkpoint split) + upstream stateless closures `in_untagged_code` /
    `count_newlines_in_range` promoted to `pub(crate)` fns.
  - `src/md/render_blocks.rs` (240): `flush_code_block` (Replace arm),
    `flush_table` (Table arm), `flush_mermaid` (Mermaid arm).
- Scope: ratatui path ONLY — `render_ansi` (272-529), `render_replace_ansi`
  (95), `Stylize`/`StyledStr` (116-140) NOT copied (0 calls from
  `render_ratatui`, verified). No `unwrap`/`expect` in production code
  (upstream's let-else/`get`/`debug_assert` patterns preserved).
- Reuses (no duplication): `StyleInto` from parse.rs (already `pub(crate)`);
  `anstyle_to_ratatui_color` widened private→pub(crate) in parse.rs (header
  noted, 291 lines ≤ cap); `all_hidden`/`merge_styles` from style.rs;
  `adapt_style` via the T1 `style_adapt` stub; T6c hyperlinks projection
  (`chunk_link_offsets`/`emit_segment_hyperlinks`) wired into both text paths.
- Documented divergences: `crate::md` paths; let-chains→nested ifs (Rust
  2021, T5f precedent); private→pub(crate) for the cross-file impl split;
  arm-local `continue`→`return`; loop locals grouped into `RenderLoopState`
  (user-chosen split option after the decision model returned an incoherent
  verdict); `#[allow(clippy::too_many_arguments)]` on `flush_table` (the
  arm is one inseverable body).
- `tests/md_render.rs` (4 tests, renderer-free — parser + `.parse()` +
  `render_ratatui(true)` directly): heading/bold/inline-code text survives
  with pretty markers consumed; code fence renders body + `CodeBlockSpan`;
  GFM table rows; `[text](url)` produces one `HyperlinkTarget`. Mermaid
  tests not ported (mermaid parsing out of scope for this ticket).
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 280 passed,
  0 failed, 1 ignored noto (4 new); `cargo clippy --all-targets` = 3 lint
  baseline, 0 nuovi, `grep -c "^warning: unused"` = 0; `wc -l` per file:
  render 279, blocks 240, events 223, segments 247, parse 291 (all ≤300).

## feat(md): copy hyperlinks projection (V3/T6c)
Derived from xai-grok-markdown/src/hyperlinks.rs (Apache-2.0).
- New `src/md/hyperlinks.rs` (153 lines ≤ cap): VERBATIM copy of the upstream
  production section (lines 1-154; from line 155 it is `#[cfg(test)]`):
  module doc (the 3 coordinate systems: source bytes, transformed bytes,
  display cells), `ChunkLinkRange`, `source_to_chunk_offset`,
  `chunk_link_offsets`, `emit_segment_hyperlinks`. Mechanical verbatim
  verification (diff vs upstream with only the documented substitutions
  applied) = clean.
- Documented divergences (header `Modified:` clause): paths
  `crate::buffers`/`crate::output` → `crate::md::buffers`/`crate::md::output`;
  the 3 fns and `ChunkLinkRange` (+ fields) widened `pub(crate)` → `pub`
  (visibility-for-testability, tests/ is an external crate); inline tests
  not copied (all renderer-dependent → T16), adapted subset in tests/.
- `tests/md_hyperlinks.rs` (5 tests, hand-built `LinkTarget`/chunk — no
  renderer): projection inside chunk (out-of-chunk link excluded),
  `source_to_chunk_offset` identity without transforms, ASCII column_range,
  CJK prefix counts 2 cells per glyph (`col_start` after `你好 ` = 5),
  wrapped link across 2 segments → 2 `HyperlinkTarget`s sharing the same
  `id`.
- `src/md/mod.rs`: `pub mod hyperlinks;`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 276 passed,
  0 failed, 1 ignored noto (5 new); `cargo clippy --all-targets` = 3 lint
  baseline, 0 nuovi, `grep -c "^warning: unused"` = 0; `wc -l`
  src/md/hyperlinks.rs = 153 ≤300.

## feat(md): complete output.rs — render output types (V3/T6b)
Derived from xai-grok-markdown/src/output.rs (Apache-2.0).
- `src/md/output.rs` completed (180 lines ≤ cap): added the render output
  types from the upstream production section (lines 1-180), verbatim, AFTER
  the existing `HyperlinkTarget` (T5c, untouched — no duplication):
  `CodeBlockSpan` (info, body, output_line_range, source_byte_range),
  `MarkdownRenderOutput` (+ `new`/`clear`/`as_view`; fields lines,
  line_source_map, hyperlinks, code_blocks, tables), `MarkdownRenderView<'a>`
  (zero-copy slices + `line_count()`), and the pure mapping
  `build_code_block_spans` (monotonic newline cursor over
  `CodeBlockMeta.body_source_range`s → per-block pre-wrap
  `output_line_range`). Scope notes: the enumeration in the ticket listed
  the three types only; the declared production range (1-180) also contains
  `build_code_block_spans`, which depends solely on already-ported types —
  decision included it (nxm-decision include_builder, 97.5% conf.), keeping
  the copy at its natural whole. Header updated to the T6b `Modified:` form;
  `crate::buffers::…` adapted to our split: `crate::md::buffers::CodeBlockMeta`
  + `crate::md::buffers_table::TableCopyMeta`; visibility-for-testability:
  `build_code_block_spans` widened from `pub(crate)` to `pub` (tested
  directly in tests/ — upstream's inline tests all go through the not-yet-
  ported renderer and could not be adapted).
- `tests/md_output.rs` (7 tests): `new()` empty on all buffers; `clear()`
  empties after pushes; `as_view()`/`line_count()` slice coherence;
  `CodeBlockSpan` field round-trip; `build_code_block_spans` mapping (two
  blocks → disjoint increasing output ranges), empty-body anchoring + empty
  metas → no spans; rendered-lines slice contract.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 271 passed,
  0 failed, 1 ignored (7 new); `cargo clippy --all-targets` = 0 unused, 3
  baseline, 0 new (test-side `useless use of vec!` fixed → array);
  `wc -l` src/md/output.rs = 180 ≤300.

# 2026-10-16

## feat(md): copy SourceMap (V3/T6a)
Derived from xai-grok-markdown/src/source_map.rs (Apache-2.0).
- New `src/md/source_map.rs` (105 lines ≤ cap): VERBATIM copy of upstream
  `SourceMap` — rendered→source byte mapping for copy/selection. All methods
  copied with upstream docs: `new`, `add`, `to_source` (overlap-clamped min/max
  resolution, `None` when not covered), `is_empty`, `len`, `extend_with_offsets`
  (rend./source offsets for merging frozen + tail maps), `segments`, `truncate`,
  `clear`. Zero dependencies (`std::ops::Range` only), pure logic, private
  `segments` field (public API only — matches upstream visibility).
- Provenance header replaces the upstream doc-module first line; the rest of
  upstream module doc is kept Doc-verbatim. The trailing design-note comment
  (post-impl fragment about ratatui line-level mapping) was OMITTED
  (per-ticket "copia o ometti": it is a leftover fragment, not API).
- `src/md/mod.rs`: `pub mod source_map;`.
- New `tests/md_source_map.rs` (5 tests): add+to_source 1:1 mapping; None for
  unmapped ranges/gaps; partial-overlap → source slice; `extend_with_offsets`
  merging with both offsets (segments + resolution through merged map);
  len/is_empty/truncate/clear coherence.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 264 passed,
  0 failed, 1 ignored (5 new); `cargo clippy --all-targets` = 0 unused, 3
  baseline, 0 new (one `length comparison to zero` in the new test fixed);
  `wc -l` src/md/source_map.rs = 105 ≤300.

# 2026-10-16

## feat(md): table rendering — format_table (V3/T5g)
Derived from xai-grok-markdown/src/parse.rs (Apache-2.0).
- New `src/md/parse_table.rs` (300 lines ≤ cap): `MarkdownParser::format_table`
  verbatim from upstream — column widths from `plain_text()` display widths,
  `max_table_width` proportional reflow (word minimums → grapheme floors →
  proportional distribution), alignments, top/header-sep/row/row-sep/bottom
  borders via `TableBorders::BOX` and `format_border_line`, hyperlink
  accumulation (`TableHyperlink` line_offset adjusted per row), `TableCellCopy`s
  and source line offsets. Returns the `FormattedTable` destructured in
  `parse_end.rs::on_end` → `TableReplace` push into `buffers.table_replaces`.
- New `src/md/parse_table_fmt.rs` (269 lines): `wrap_cell_text_joins`
  (textwrap FirstFit + custom word separator, grapheme hard-split fallback for
  unbreakable words), `format_styled_content_lines` (multi-visual-line rows,
  span slicing across wrap boundaries, bold header/body styles, code/ strike/
  link styling, `allow(clippy::too_many_arguments)` per upstream),
  `format_border_line`.
- New `src/md/parse_table_word.rs` (231 lines): free fns `cell_word_separator`
  (punct-break heuristic with URL protection via `url::Url::parse`) and
  `cell_wrap_joins` (Tight/Gap reconstruction of omitted text between wrapped
  lines).
- `src/md/parse_end.rs`: `format_table` stub REMOVED (real implementation in
  `parse_table.rs`); `FormattedTable` stays there (documented in its doc
  comment) — reachable via `use crate::md::parse_end::FormattedTable`; dead_code
  allow dropped (all fields now used).
- `src/md/mod.rs`: + `pub mod parse_table; parse_table_fmt; parse_table_word;`.
- Divergences (documented in each `Modified:` header): Rust 2021 — upstream
  let-chains rewritten as nested `if`s; `sort_by` → `sort_by_key(Reverse)`
  (clippy parity, same ordering); visibility-for-testability —
  `wrap_cell_text_joins`/`format_border_line`/`cell_wrap_joins` `pub` (tested
  directly in tests/); latex/mermaid remain stubs of their dedicated tickets.
- New `tests/md_parse_table.rs` (7 tests): GFM table → exactly one
  `TableReplace` (n_cols=2, borders + cell text in plain lines),
  `format_border_line` box/ASCII chars (TableBorders), wrap-joins quartet
  adapted from upstream (`zebraone_space_before_slash`, `foo_bar_punct_wrap`,
  `hard_split_url`, `likes_long_walks`), `max_table_width()` cap (all lines ≤
  30 display columns, unicode-width measured).
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 259 passed,
  0 failed, 1 ignored (7 new); `cargo clippy --all-targets` = 0 unused import,
  3 baseline, 0 new; `wc -l` parse_table.rs=300, parse_table_fmt.rs=269,
  parse_table_word.rs=231, parse_end.rs=199.

# 2026-10-16

## feat(md): copy MarkdownStyle + TableBorders (V3/T1)
Derived from xai-grok-markdown/src/style.rs (Apache-2.0).
- New `src/md/style.rs` (265 lines): verbatim copy of grok-build's markdown
  styling types — `TableBorders` (BOX/ASCII/DOUBLE, all short+long accessors)
  and `MarkdownStyle` (24 scalar `anstyle::Style` fields + 2 `[Style; 6]`
  heading arrays = 26 struct fields, 36 Style values) with its identical
  `adapt()` method, plus the `all_hidden` and `merge_styles` helpers. Inline
  `#[cfg(test)]` test module stripped (RULES.md: no inline tests). Origin
  header `//! Derived from ...` prepended. `anstyle` already present (T0).
  Visibility `pub(crate)` matches upstream verbatim (T1-review Resolution 1).
- New `src/md/style_adapt.rs` (20 lines): `adapt_style` identity stub
  (placeholder — real truecolor→256/16 downgrade lands with the future
  `colors.rs` ticket). `style.rs` imports `adapt_style` from here; swapping in
  the real implementation is a one-line import change. `pub(crate)` per
  upstream.
- `src/md/mod.rs`: declares `pub mod style;` and `mod style_adapt;`.
- New `tests/md_style.rs` (8 tests, all via public API): moved the upstream
  `test_style::STYLE` const here as an external integration test + contract
  tests — MarkdownStyle field effects (BOLD/ITALIC/STRIKETHROUGH),
  `TableBorders::BOX/ASCII/DOUBLE` characters (+ default/new), and
  `adapt_style` identity exercised indirectly through `MarkdownStyle::adapt()`
  (which delegates to `adapt_style`; `all_hidden`/`merge_styles` are
  `pub(crate)` and tested in a future ticket when the renderer wires them).
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 201 passed,
  0 failed, 1 ignored (8 new); `cargo clippy --all-targets` = 3 baseline
  (type_complexity + Default×2), 0 new; `wc -l` style.rs=265 ≤300, style_adapt.rs=20 ≤300.

## feat(md): fixed tokyonight MarkdownStyle theme (V3/T2)
Derived from xai-grok-markdown .../theme/md_style.rs + theme/tokyonight.rs (Apache-2.0).
- New `src/md/theme.rs` (159 lines): `fixed_style() -> MarkdownStyle` returning
  a single fixed tokyonight theme, built from inlined RGB constants (no
  `Theme::current()`, no `struct Theme` — multi-theme deferred). Verbatim bridge
  functions copied from grok's `md_style.rs` (`to_anstyle` with `Reset→None`
  for NO_COLOR, `fg`, `bg`, `modifier_to_anstyle`, `heading_inner_styles`,
  `heading_outer_styles`). Color values from grok's `theme/tokyonight.rs`
  (md_heading_h1..h6 = TEAL/BLUE/ORANGE/RED/GREEN/MAGENTA all +BOLD,
  md_code=GREEN1, md_task_checked=CYAN, md_task_unchecked=BLUE,
  md_muted=COMMENT, md_code_bg=BG_HIGHLIGHT, md_text=FG, link_fg=BLUE).
- `src/md/mod.rs`: declares `pub mod theme;`.
- New `tests/md_theme.rs` (3 tests): `reset_maps_to_no_color` (NO_COLOR),
  `named_colors_map_concretely` (DarkGray→BrightBlack, Gray→White, Red→Red),
  `fixed_style_has_tokyonight_fields` (text fg truecolor, strong_inner BOLD,
  emphasis_inner ITALIC, link_text UNDERLINE, heading_inner[0] fg+BOLD).
- No `#[cfg(test)]` inline; no tracing/doc-esempio sulle funzioni-ponte
  copiate (esenzione copia-fedele, RULES.md).
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 204 passed,
  0 failed, 1 ignored (3 new); `cargo clippy --all-targets` = 3 baseline,
  0 new; `wc -l src/md/theme.rs` = 159 ≤300.

## feat(md): copy parsing core — parser_options + offset_events (V3/T3a)
Derived from xai-grok-markdown-core/src/lib.rs (Apache-2.0).
- New `src/md/core.rs` (108 lines): verbatim copy of grok-build's parsing core —
  `parser_options()` (GFM | STRIKETHROUGH | MATH | TASKLISTS | TABLES),
  `offset_events()` (events with source byte ranges, single-tilde demoted to
  literal `~`), `DoubleTildeOnlyStrike` filter with `is_double_tilde_strike` and
  `strike_delim_text` helpers, and the full `Iterator::next()` impl. Keeps
  `#![deny(clippy::indexing_slicing)]`. No `MarkdownStats`/`analyze` (deferred
  to T3b). No inline tests (RULES.md).
- `src/md/mod.rs`: declares `pub mod core;`.
- New `tests/md_core.rs` (4 tests): `parser_options_contains_all_flags` (5
  flags), `offset_events_double_tilde_produces_strikethrough`,
  `offset_events_single_tilde_not_strikethrough`, `offset_events_byte_ranges_consistent`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 205 passed,
  0 failed, 1 ignored (4 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `wc -l src/md/core.rs` = 108 ≤300.

## feat(md): copy MarkdownBuffers + output types (V3/T4)
Derived from xai-grok-markdown/src/buffers.rs (Apache-2.0).
- New `src/md/buffers.rs` (235 lines): core output types — `Highlight`,
  `Replace`, `LinkTarget`, `CodeBlockMeta`, `Transform`, `CellSpan`,
  `StyledCell`, `CellJoin`, `RenderEventKind`, `RenderEvent`,
  `MarkdownBuffers`, `unicode_display_width`, `floor_char_boundary`,
  `ceil_char_boundary`. Keeps `#![deny(clippy::indexing_slicing)]`.
- New `src/md/buffers_table.rs` (125 lines): table types — `TableState`,
  `TableCellCopy`, `TableCopyMeta`, `TableHyperlink`, `TableReplace`,
  `MermaidReplace`. Split per cap 300 righe (file sorgente upstream: 348).
- `src/md/mod.rs`: declares `pub mod buffers;` and `pub mod buffers_table;`;
  `buffers.rs` re-exports `MermaidReplace`/`TableReplace` publicly.
- New `tests/md_buffers.rs` (5 tests): `markdown_buffers_new_and_clear`,
  `styled_cell_new_plain_text_clear`, `unicode_display_width_ascii_and_cjk`,
  `table_state_new_and_push_text`, `cell_join_variants`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 213 passed,
  0 failed, 1 ignored (5 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `wc -l src/md/buffers.rs` = 235 ≤300, `buffers_table.rs` = 125 ≤300.

## feat(md): copy Checkpoint + CheckpointKind (V3/T5a)
Derived from xai-grok-markdown/src/checkpoint.rs (Apache-2.0).
- New `src/md/checkpoint.rs` (58 lines): verbatim copy of grok-build's checkpoint types —
  `struct Checkpoint` (3 fields: `source_bytes`, `output_lines`, `kind`) and
  `enum CheckpointKind` (8 variants: Heading, Paragraph, CodeBlock, BlockQuote,
  List, ThematicBreak, Table, HtmlBlock). All `Debug, Clone, Copy, PartialEq, Eq`.
  No dependencies; no logic (prerequisite for T5 parse). No inline tests (RULES.md).
- `src/md/mod.rs`: declares `pub mod checkpoint;`.
- New `tests/md_checkpoint.rs` (2 tests): `checkpoint_kind_has_all_variants`
  (8 variants + PartialEq), `checkpoint_fields_and_copy` (3 fields + Copy semantics).
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 215 passed,
  0 failed, 1 ignored (2 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `wc -l src/md/checkpoint.rs` = 58 ≤300.

## feat(md): syntax highlighting module on syntect defaults (V3/T5b, option B)
Derived from xai-grok-markdown/src/syntax.rs (Apache-2.0).
- New `src/md/syntax.rs` (149 lines): verbatim structure/API from grok-build's
  `syntax.rs`, adapted to option B — `SyntaxSet::load_defaults_newlines()`
  instead of the two-face `syntaxes.bin` build dump; `ThemeSet::load_defaults()`
  for the theme; no `.tmTheme` assets, no `build.rs`, no Swift grammar.
  `Syntect::new()` returns `Option<Self>` (no `expect`/`unwrap` in production).
  `parse_line_citation_fence_info` and `syntax_highlight_raw` copied verbatim.
  `find_syntax_by_token`, `find_syntax_by_file_path`,
  `highlight_lines_for_token`, `highlight_lines_by_file_path`,
  `highlight_lines_for_fence_info`, `find_syntax_for_fence_info` copied with
  Swift/patching logic removed.
- `src/md/mod.rs`: declares `pub mod syntax;`.
- New `tests/md_syntax.rs` (5 tests): `line_citation_fence_parses_start_end_path`,
  `line_citation_rejects_non_numeric_line`,
  `line_citation_rejects_plain_lang_token`, `syntect_default_builds_and_finds_rust`,
  `syntax_highlight_raw_produces_spans`, `syntax_highlight_raw_none_and_unknown_lang`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 221 passed,
  0 failed, 1 ignored (6 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `cargo tree | grep -iE "two-face|onig"` → vuoto;
  `wc -l src/md/syntax.rs` = 149 ≤300.
- Nota: opzione A (two-face + build.rs + assets + Swift) rimandata a ticket dedicato.

## feat(md): copy url_scan + anticipate HyperlinkTarget (V3/T5c)
Derived from xai-grok-markdown/src/{url_scan,output}.rs (Apache-2.0).
- New `src/md/url_scan.rs` (96 lines): verbatim copy of grok-build's plain-URL
  detection — `detect_plain_urls`, `detect_plain_urls_with_offset`,
  `for_each_plain_link`. Keeps `is_char_boundary` guards, scp-skip,
  email→`mailto:`, overlap dedup. No `unwrap`/`expect` in production.
- New `src/md/output.rs` (17 lines): only `HyperlinkTarget` anticipated from
  upstream `output.rs` (ticket T7 will complete the module). Fields:
  `line_index`, `column_range`, `url`, `id`; derives `Debug, Clone,
  PartialEq, Eq`.
- `src/md/mod.rs`: declares `pub mod output;` and `pub mod url_scan;`.
- New `tests/md_url_scan.rs` (10 tests): URL without trailing dot, email→mailto,
  scp not mailto, split-across-spans is one target, empty document, offset,
  dedup, incremental ids, HyperlinkTarget fields/eq.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 231 passed,
  0 failed, 1 ignored (10 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `wc -l src/md/url_scan.rs` = 96 ≤300, `output.rs` = 17 ≤300;
  `cargo tree | grep -iE "two-face|onig"` → vuoto.

## feat(md): copy OpenCodeHighlighter streaming cache (V3/T5d)
Derived from xai-grok-markdown/src/open_code_highlighter.rs (Apache-2.0).
- New `src/md/open_code_highlighter.rs` (243 lines): verbatim copy of grok-build's
  incremental syntax-highlighting cache for streamed code fences —
  `OpenCodeHighlighter` (`new`, `highlight_block`, `highlight_closed`, `highlight`,
  `committed_prefix_matches`), `HlLine` type alias, `CLOSED_MEMO_CAP_BYTES` const.
  Keeps `is_char_boundary` guards, `match ... Err => return None` (no panic in prod).
  `Syntect` sourced via `Syntect::new()` (option B defaults, T5b) — no two-face,
  no `build.rs`, no assets, no Swift grammar.
- `src/md/mod.rs`: declares `pub mod open_code_highlighter;`.
- Visibility-for-testability: `new`, `highlight_block`, `highlight_closed`,
  `highlight`, `committed_prefix_matches`, `closed_memo_bytes`, `CLOSED_MEMO_CAP_BYTES`,
  `HlLine` widened to `pub` for `tests/` (documented in header).
- New `tests/md_open_code.rs` (8 tests): `append_only_growth_matches_fresh_full_highlight`,
  `fence_change_invalidates`, `start_offset_change_invalidates`,
  `unknown_fence_returns_none`, `closed_memo_matches_batch_and_is_idempotent`,
  `closed_memo_distinguishes_fence_info_and_body`,
  `closed_memo_does_not_disturb_open_block_state`,
  `closed_memo_cap_overflow_keeps_output_correct`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 239 passed,
  0 failed, 1 ignored (8 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `cargo tree | grep -iE "two-face|onig"` → vuoto;
  `wc -l src/md/open_code_highlighter.rs` = 243 ≤300.

## feat(md): scaffold MarkdownParser (V3/T5e)
Derived from xai-grok-markdown/src/parse.rs (Apache-2.0).
- New `src/md/parse.rs` (298 lines): scaffold of grok-build's markdown parser —
  `MarkdownParser` struct (text, ms, buffers, syntect, open_code, tag_stack,
  table_state, depth, bq_depth, last_checkpoint, max_table_width,
  link_id_counter, collapse_soft_breaks, pending_code_block), `new()`,
  builders `collapse_soft_breaks()` / `max_table_width()`, `parse()` entry
  point (iterates `offset_events` → stub `on_event` → `ParsedMarkdown`),
  `ParsedMarkdown` struct + `new()`. Pure helpers: `StyleInto` trait + impl,
  `anstyle_to_ratatui_color`, `find_substring`, `decode_html_entity`,
  `has_blank_line_after`, `is_br_tag`, `PendingCodeBlock`.
- `on_event`/`on_start`/`on_end` are empty stubs (T5f); `format_table` omitted
  (T5g). `latex` deferred (dedicated ticket). `offset_events` imported from
  `crate::md::core` (T3a). No `unwrap`/`expect` in production; stubs use empty
  bodies, not `todo!()`.
- `src/md/mod.rs`: declares `pub mod parse;`.
- Visibility-for-testability: `ParsedMarkdown` fields widened to `pub` for
  `tests/` (documented in header).
- New `tests/md_parse.rs` (9 tests): `parser_new_and_builders`,
  `parse_simple_input_no_panic`, `parse_empty_input_no_panic`,
  `parse_no_syntect_no_panic`, `collapse_soft_breaks_false`,
  `max_table_width_some`, `max_table_width_none`, `parsed_markdown_fields`,
  `checkpoint_kind_variants`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 248 passed,
  0 failed, 1 ignored (9 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `wc -l src/md/parse.rs` = 298 ≤300.

## feat(md): parser event dispatch — on_event/on_start/on_end (V3/T5f)
Derived from xai-grok-markdown/src/parse.rs (Apache-2.0).
- New `src/md/parse_events.rs` (~200 lines): `on_event` dispatch (events:
  Start/End/Text/Code/InlineMath/SoftBreak/HardBreak/Html/InlineHtml/
  DisplayMath/FootnoteReference/Rule/TaskListMarker), `push_highlight`.
- New `src/md/parse_blocks.rs` (~250 lines): `on_start`/`on_end` with heading,
  blockquote, code block, HTML block, list, item, table, emphasis, strong,
  strikethrough, link/image handling; `format_table` stub (T5g);
  `try_push_mermaid` stub (T5g).
- New `src/md/parse_inline.rs` (~150 lines): `style_inline_code_span`,
  `scan_inline_html_entities`, `push_table_text_linking_urls`,
  `try_push_mermaid` stub, `push_display_math_block` stub (T-latex).
- New `src/md/latex.rs` (stub): `latex_to_unicode_inline`/`latex_to_unicode_display`
  return `None` (T-latex deferred).
- `src/md/mod.rs`: declares `pub mod parse;`, `pub mod parse_blocks;`,
  `pub mod parse_events;`, `pub mod parse_inline;`, `pub mod latex;`.
- `src/md/parse.rs`: stubs `on_event`/`on_start`/`on_end` removed; kept struct,
  new/builder/parse/ParsedMarkdown/pure helpers.
- Visibility-for-testability: `push_highlight`, `on_start`, `on_end`,
  `highlight_block`, `highlight_closed`, `highlight`,
  `committed_prefix_matches`, `closed_memo_bytes`, `CLOSED_MEMO_CAP_BYTES`,
  `HlLine` widened to `pub` for `tests/` (documented in headers).
- New `tests/md_parse_events.rs` (4 tests): `parse_produces_highlights_for_heading_and_bold`,
  `inline_code_produces_span_with_style`, `strikethrough_is_handled`,
  `math_inline_does_not_panic`.
- Verifica: `cargo build` ok; `cargo test -- --test-threads=1` = 252 passed,
  0 failed, 1 ignored (4 new); `cargo clippy --all-targets` = 3 baseline, 0
  new; `cargo tree | grep -iE "two-face|onig"` → vuoto;
  `wc -l src/md/parse*.rs` → tutti ≤300.

# 2026-10-15

## fix: wrap markdown a larghezza di display (V1 — adozione openviking `crates/ov_cli`)
- `src/markdown.rs` misurava in **byte** (`cur.len() + word.len() + 1 > max`) e in
  **char** (troncamento fence): con CJK/emoji (2 celle, 3–4 byte) il wrap sbagliava
  e i fence sfondavano il pane. Ora `wrap()` usa `unicode-width`, adattata da
  `openviking crates/ov_cli/src/commands/render_utils.rs::wrap_display_text`
  (Apache-2.0; la root del vendor è AGPL-3.0 — nessun codice preso da lì):
  append per parola finché entra, **split char-level** per parole più lunghe della
  riga, `UnicodeWidthStr::width` anche per `prefix`. Deviazioni documentate: niente
  `max_lines`/ellissi (la history mostra tutte le righe), `…` conservato sui fence.
- Nuova dep `unicode-width = "0.2.0"` (pinnata; era già nel lock via
  `unicode-truncate`, nessuna copia nuova). `THIRD-PARTY-NOTICES.md` attribuisce
  openviking (`crates/ov_cli`, snapshot `f6010a5`); corretta la nota di licenza
  errata in `docs/patterns/VENDOR-TUI-ANALYSIS.md` (root AGPL-3.0, non Apache-2.0).
- Doc comment + esempio su `render_markdown` (RULES: doc pubblica con esempio).
- Test: `tests/markdown.rs` (6, headless su `render_markdown`): nessuna riga
  renderizzata supera `max_width` per prose CJK, parole non spezzabili, fence
  multibyte; rosso sul renderer vecchio (4/6 falliscono), verde sul nuovo.
  Verifica: build ok, suite **180/180** verde + 15 doctest (1 ignored noto),
  `--test-threads=1`; clippy = 3 lint baseline identici, 0 nuovi; `markdown.rs`
  268 righe (≤300). V1 → Resolved.

## refactor: estrai Message/Role in src/message.rs (DEBT1)
- `Message` (struct) e `Role` (enum) con i loro `impl` (`new`, `with_tool`,
  `tool`) spostati da `src/app.rs` al nuovo modulo `src/message.rs` (≤300 righe),
  re-exportati da `app.rs` tramite `pub use crate::message::{Message, Role};`
  così tutti i call site esistenti (`app::Message`, `app::Role`) restano stabili
  senza modifiche. `lib.rs` dichiara `pub mod message;`.
- Nessun cambio di comportamento: `push_thinking`/`apply_token` restano in
  `app.rs` (dipendono dallo stato di thinking di `App`, per decisione A1+B1).
- Verifica: `cargo build`, `cargo test` (185/0/1, `--test-threads=1`),
  `cargo clippy --all-targets` (3 lint baseline, 0 nuovi),
  `grep "struct Message\|enum Role" src/` → solo in `message.rs`.

## feat: syntect syntax highlighting for code fences (V2)
- `src/highlight.rs` (nuovo, ≤300 righe): `highlight_code(code, lang) ->
  Vec<Vec<(Style, String)>>` per-token highlighting via `syntect` 5.3.0
  (`default-features = false`, `features = ["default-fancy"]`, pure-Rust
  `fancy-regex`, no `onig`/C, no `syntect-tui`). `SyntaxSet`/`ThemeSet`
  caricati una sola volta (`OnceLock`); selezione sintassi per token di lingua
  con fallback plain-text su lang vuoto/sconosciuto; mapping
  `syntect::highlighting::Color` → `ratatui::style::Color::Rgb`. Nessun
  `unwrap`/`expect` in produzione.
- `src/markdown.rs`: nel ramo `Segment::Code` la riga è costruita dagli span
  di `highlight_code` (più span colorati per riga), mantenendo box/border e
  applicando la troncatura cell-width V1 **per-span**; su lang sconosciuto o
  errore si fall back al vecchio `highlight_style` (colore unico) senza
  rompere il render. `highlight_style` resta per il fallback.
- Test: `tests/highlight.rs` (multi-span su Rust, fallback lang sconosciuto,
  fallback lang vuoto, coerenza OnceLock) + caso di troncatura cell-width in
  `tests/markdown.rs`.
- `THIRD-PARTY-NOTICES.md`: attribuzione syntect (MIT).
- Verifica: `cargo build`, `cargo test -- --test-threads=1` verde (≥185 +
  nuovi, 0 failed, 1 ignored noto), `cargo clippy --all-targets` = 3 lint
  baseline, `wc -l` entrambi i file ≤300, `cargo tree -i syntect` =
  default-fancy senza onig.

# 2026-10-02

# Changelog

# 2026-10-02

## feat: KeyStore cablato in produzione (I2 — wiring del seam I1)
- **Factory (TDD)**: `keystore::store_for(mode, passphrase) -> Result<Box<dyn KeyStore>, KeyStoreError>`
  con mapping `EnvFirst→EnvStore`, `KeychainFirst`/`OsKeychain→OsKeychainStore`,
  `EncryptedFile→EncryptedFileStore`; EncryptedFile senza (o con) passphrase
  vuota = `KeyStoreError::Unavailable` **pulito**, mai panic.
- **Passphrase 1x/sessione**: `app.passphrase: Option<Zeroizing<String>>` +
  prompt mascherato in `handler.rs` modellato su `set_key_pending` (buffer
  azzerato su Enter/Esc, mai loggato — D7) + overlay
  `ui.rs::render_passphrase_overlay` (bullet per char, mai il valore). Esc
  annulla tutte le azioni in attesa (models fetch, connect MCP, send accodata)
  così niente loop di re-prompt.
- **Read-path (4 call site) su `resolve_key_via` = factory + `resolve_key_with`**,
  con `spawn_blocking`/`block_in_place` mantenuti: `main.rs` models-fetch,
  `main.rs` agent spawn, `mcp/mod.rs::connect_from_config` (passphrase ora in
  firma; il connect MCP è **differito al loop** per il gate), `handler.rs`
  `/provider use`. Gate `needs_passphrase`: se serve la passphrase e manca →
  apre il prompt, **non spawna**; Enter riprende, Esc annulla.
- **Write-path sul trait**: `/provider set-key` (Enter) e `/provider
  remove-key` usano `store.set`/`store.delete` via factory — niente più
  `keyring::Entry` diretto in `handler.rs`. In modalità cifrata il set-key
  prompta la passphrase retenendo il buffer e salva alla seconda Enter.
- `keys.rs::resolve_key` legacy **non rimosso** (ancora coperto da
  `tests/keys.rs`); `current_mode()` estratto e condiviso.
- Tests: `tests/keystore_factory.rs` (6) + `tests/passphrase.rs` (6, incl.
  overlay masked); `tests/mcp_sampling.rs` adattato alla nuova firma. Split
  300-cap: le funzioni di risoluzione sono passate a `src/keys/resolve.rs`
  (re-export da `keys.rs`, percorsi stabili). Verifica:
  build ok, suite **173/173** verde con `--test-threads=1` + 15 doctest,
  clippy = 3 lint **identici** alla baseline pre-cambiamento (verificata prima
  di toccare il codice, senza `git stash` per non toccare le modifiche
  untracked in `docs/patterns/`), 0 nuovi, 0 unwrap/expect nuovi in prod.
  I2 → Resolved.

## refactor: split app.rs — commands.rs + metrics.rs (RULES.md debt, T6)
- Estrazione behavior-preserving: `src/commands.rs` (165 righe: `Command`,
  `ProviderCommand`, `ServerCommand`, `ConfigCommand`, `parse_command`) e
  `src/metrics.rs` (100 righe: `Metrics` + impl) spostati **verbatim** fuori da
  `app.rs`. `app.rs` 973 → 729 righe (diff +7/−251 = solo re-export + rimozioni).
- Compat: `app.rs` ri-esporta i tipi (`pub use crate::commands::{...}` /
  `crate::metrics::Metrics`), così gli import esistenti
  (`nxm_tui::app::{parse_command, Command, Metrics, ...}` in handler + test)
  restano validi **senza toccare un solo call site**.
- Dipendenze pulite: `commands.rs` usa solo `crate::app::AgentMode`,
  `metrics.rs` solo std (nessun ciclo).
- Fuori scope (ticket futuri): spostare `Message`/`Role` (60+ call site), split
  di `main.rs::run` (fn da 340 righe).
- Eseguito da free agent + review indipendente. Verifica: build ok, suite verde
  in parallelo, clippy = 3 lint IDENTICI alla baseline (type_complexity +
  Default×2, preesistenti, provati con git stash), 0 nuovi, 0 unwrap nuovi.
  T6 → Resolved.

## feat: interactive scrollback search in the TUI (T5 — wires src/search.rs)
- Nuovo `src/search_ui.rs` (181 righe, funzioni pure testabili): `line_matches`
  (indici righe che matchano), `scrollback_for_line` (indice-riga → `scrollback`
  così il match entra in viewport), `highlight_line` (restyle degli span
  matchanti via `compiled_regex`, slice byte-safe allineati al regex).
- Wiring: `/` (prompt vuoto) apre la ricerca; query live con highlight; `n`/`N`
  navigano i match con wrap; `Esc`/Backspace-su-vuoto chiudono. Stato
  `App::search: Option<SearchState>` (pattern modale come `set_key_pending`),
  query bar overlay in `ui.rs`. La ricerca opera sulle RIGHE FINALI renderizzate
  (`render_history`), così gli indici sono allineati a wrap/markdown; lo
  scrollback-per-match è calcolato in `render_history` dove `lines.len()` e
  `area.height` sono reali (il round-1 usava valori approssimati → non saltava).
- Collega il modulo `search` portato da grok (commit `31b6f72`) → chiude il
  cerchio sulla vendor-adoption prio #1.
- Eseguito da free agent, **2 round di review indipendente** (round 1: `n`/`N`
  non saltava al match per `scrollback_for_line` con `messages.len()`/height
  hardcoded; round 2: calcolo spostato in render con valori reali + semantica
  `current` chiarita). Verifica: `tests/search_ui.rs` 21/21 (incl. navigazione
  wrap + jump), clippy 0 sui file toccati, 0 unwrap prod. T5 → Resolved.

## feat: provider/model commands — /models, /model use, /provider remove-key (T2)
- Nuovo `src/models.rs` (212 righe): `ModelEntry` + `parse_models` (parser
  tollerante R2: due envelope OpenAI `data[]` / Ollama `models[]`, id→name→model,
  `created` int o ISO, chiavi ignote ignorate, cap 100) + `models_candidate_paths`
  (normalizza `/v1` → mai `.../v1/v1/models`) + `fetch_models` async (fallback
  path `/v1/models` → `/models` → `/api/tags`, 401 azionabile, server-down).
- `/models` — lista dal provider attivo (fetch async nel loop di `main.rs`,
  stato pending come `inference_task`, UI mai bloccata; key risolta per-provider
  via `resolve_key`+`spawn_blocking`, mai loggata).
- `/model use <id|numero>` — seleziona il modello (indice sulla lista o id),
  imposta `cfg.model_name` + `Provider.default_model`, salva config.
- `/provider remove-key <name>` (alias `rm-key`) — elimina la key dal keychain
  (`delete_credential`) con guided error se keychain non disponibile.
- Modello attivo mostrato nella mode bar (`src/mode_bar.rs`); `app.model_name`
  sincronizzato da `cfg.model_name` all'avvio (entrambi i rami → Running).
- Help (`overlays.rs`) + autocomplete aggiornati; sezione Security "API keys are
  NEVER printed or logged".
- Eseguito da free agent esterno, **2 round di review indipendente** (round 1:
  4 difetti trovati — mode bar `—`, doppio `/v1`, api_key None, report
  incompleto; round 2: tutti corretti). Verifica: `tests/models.rs` 16/16,
  clippy 0 sui file toccati, 0 unwrap/expect prod. T2 → Resolved.
- Debito confermato (non regressione): flakiness `keys` in parallelo (mock
  keychain) — la risolverà il seam `KeyStore` di R5.

## feat: dev-friendly key loading — .env fallback, no keychain prompt
- **Problema risolto**: `resolve_key` interrogava SEMPRE il keychain OS per
  primo → su macOS un dialog password a ogni spawn Agent (ogni messaggio),
  anche quando la key era in una env var. Fastidioso in sviluppo.
- **Nuovo**: `keys::KeySourceMode {EnvFirst, KeychainFirst}` + funzione pura
  `key_source_mode(env_present, override)`. `resolve_key` sceglie il mode:
  - `NXM_KEY_SOURCE=env` → **env-first**: keychain MAI toccato (zero prompt).
  - `NXM_KEY_SOURCE=keychain` → keychain-first (default produzione, D7).
  - `auto`/unset → env-first SE esiste un `.env` locale, altrimenti keychain-first.
- `main.rs`: `dotenvy::dotenv()` all'avvio carica un `.env` locale (gitignored)
  se presente → modalità dev automatica. Crea `.env` (template `.env.example`),
  metti le chiavi, nessun prompt. Prod senza `.env` resta keychain-first.
- Cargo: `dotenvy = "0.15.7"` (pin esatto). `.gitignore`: `/.env`. Nuovo
  `.env.example`.
- **Debito RULES.md risolto contestualmente**: i test inline di `keys.rs`
  spostati in `tests/keys.rs` (keys.rs 356→196 righe, sotto il cap 300). La
  FLAKINESS parallela del mock keychain è risolta: `ENV_LOCK` serializza i test
  che toccano stato globale; la suite completa è ora verde IN PARALLELO (prima
  richiedeva `--test-threads=1`).
- **Scoperta onesta**: il test `keychain_beats_env` è `#[ignore]` — la prod usa
  `keyring::Entry` (store nativo Apple), i test montano un mock su
  `keyring_core`: namespace separati, il mock non è visibile a `keyring`.
  Serve un seam `KeyStore` iniettabile per testarlo davvero (debito tracciato).
  Il path env-first — quello usato in dev — è coperto e verde.

## feat: wire MCP host into the runtime (startup session)
- `src/mcp/mod.rs`: new `connect_from_config(cfg, endpoint) -> Result<Option<McpHandle>>`
  helper — resolves the active provider's API key (keychain→env, off-runtime
  via `spawn_blocking`, value never logged) and delegates to `connect_if_present`.
  Clean no-op (`Ok(None)`) when `cfg.mcp.enabled=false` or the server binary is
  absent.
- `src/main.rs`: at startup, when the endpoint is `Running`, open an MCP session
  and keep the `McpHandle` alive for the whole loop (`_mcp` binding; drop stops
  the run loop). Connect failure logs a warning and continues — chat is never
  blocked. (Closes roadmap item #6 in `docs/mcp/ARCHITECTURE.md`.)
- Known limitation (noted, not addressed): a session connected via the NoServer
  menu *after* startup is not yet given an MCP session (handle is computed once
  pre-loop). Startup auto-detect path — the common case — is covered.
- Tests: `tests/mcp_sampling.rs` +2 — `connect_from_config` clean-skip when
  disabled and when the binary is absent (deterministic, headless). mcp_sampling
  6/6, mcp_protocol 5/5. clippy: 0 on touched files.

## feat: scrollback search primitives — ported from grok-build (search module)
- New `src/search.rs` (vendor adoption, prio #1 di VENDOR-TUI-ANALYSIS):
  `QueryKind {Substring, Regex}` + `TextMatcher` (smart-case alla Vim/ripgrep:
  case-insensitive salvo maiuscole nella query; substring via `regex::escape`)
  e `next_index_after`/`prev_index_before` (navigazione `n`/`N` con wrap su
  slice ordinata). Adattato da grok-build `xai-grok-pager-render/src/search`
  (xai-org/grok-build, Apache-2.0).
- **Conformità RULES.md** (il port corregge la fonte): niente `unwrap`/`expect`
  in prod — il campo `regex` è `Option<regex::Regex>` (regex invalida → `None`
  + `is_error`, `is_match` → false, nessun panic path); test spostati in
  `tests/search.rs` (no `#[cfg(test)]` inline); logging `tracing` target
  `nexum::search`; doc `///` con doctest; header licenza + `THIRD-PARTY-NOTICES.md`.
- Cargo: `regex = "1.13.1"` (pin esatto, versione realmente risolta).
- Verifica (revisore indipendente): `cargo test --test search` 9/9,
  `cargo test --doc` 3/3, clippy 0 menzioni di `search.rs`. NON ancora cablato
  nella UI (campo query in bottom bar + highlight match + tasti `n`/`N`) — step
  successivo.
- Debito preesistente confermato (non introdotto qui): i test inline di
  `keys.rs` falliscono in esecuzione parallela per mock-keychain globale
  condiviso (verdi con `--test-threads=1`); si lega alla violazione RULES.md
  "test inline" di `keys.rs`.

## docs: riconciliazione ticket wayfinder col codice (P3 ✅, T2 parziale)
- Audit tracker vs codice: l'handoff 2026-09-20 indicava "prossima sessione:
  implementa P3", ma P3 risulta **già implementato e committato** (arrivato con
  `cb05a5d` migrate, git pulito). Allineati i doc che erano rimasti indietro.
- **P3 → ✅ Resolved**: `keys.rs::resolve_key` (keychain→api_key_env→
  NEXUM_API_KEY→OPENAI_API_KEY→Missing), `ProviderCommand::SetKey`, overlay
  mascherato (`ui.rs`), `set_password` via keyring (`handler.rs:37`), buffer
  segreto dedicato azzerato dopo save/Esc, risoluzione per-provider allo spawn
  Agent (`main.rs` `spawn_blocking`). `keyring = "4.2.0"` pinnato.
- **T2 → resta Open · parziale**: `default_model` in `Provider` ✔ ma
  `GET /v1/models` è presente **solo come health-probe** in `detect_endpoint`;
  mancano comando `/models` (lista+selezione), `/model use <id>`,
  `/provider remove-key`, modello attivo in mode/bottom bar.
- Aggiornati: `tickets/index.md`, `tickets/P3-key-entry-prompt.md`.
- Debito annotato (non in scope): modulo test inline in `keys.rs` viola la
  regola RULES.md "test in tests/, no #[cfg(test)] inline".

# 2026-09-30

## feat: MCP host subsystem — sampling capability (minimal working cut)
- New `src/mcp/` subsystem: nxm-tui is now an MCP client/host. It spawns an
  MCP server (`nxm-session-mcp --transport stdio`) as its own child, completes
  the `initialize` handshake declaring `capabilities.sampling = {}`, and serves
  `sampling/createMessage` by forwarding to the OpenAI-compatible LLM backend.
  This unblocks server tools (e.g. `generate_handoff`) that need LLM sampling
  to populate their output sections.
  - `protocol.rs` (244): JSON-RPC 2.0 + sampling wire types (serde, spec
    2025-06-18). `transport.rs` (146): stdio spawn + newline framing +
    `binary_present`. `sampling.rs` (156): `SamplingHandler` seam +
    `LlmSamplingHandler` (auto-approved, no `async-trait` — uses `BoxFuture`).
    `client.rs` (273): lifecycle + id-correlated calls + inbound dispatch.
    `mod.rs` (72): `connect_if_present` (clean skip when binary absent).
  - Config: `[mcp]` table (`enabled`, `command`, `args`) in `TuiConfig`.
  - Cargo: added `anyhow` (workspace) to the bin/lib; enabled tokio `process`.
    No new external crates beyond features already in the tree.
  - Tests: `tests/mcp_protocol.rs` (5, serde shapes vs spec) +
    `tests/mcp_sampling.rs` (4, handler contract + live `initialize` against
    the real binary, which passed end-to-end).
  - Docs: `docs/mcp/ARCHITECTURE.md` (as-built + refinement roadmap: approval
    overlay, `tools/call`, streaming, socket transport, main.rs wiring).
  - NOT wired into `main.rs` runtime yet by design — see roadmap.

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
