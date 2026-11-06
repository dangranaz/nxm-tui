# Delega — 03 Continue-after-halt (ripresa del loop su conferma utente)

> **Stato:** PRONTO · **Creato:** 2026-10-26
> Companion: `loopeng-01_03-continue-after-halt_ENG.md` (tenere in sync).
> Origine design: sessione nxm-session `loopeng-01` (loop engineering).
> Precedenti consolidati su `main`: `loopeng-01_01-bounded-execution`
> (commit `bed58d3`) e `loopeng-01_02-circuit-breaker` (commit `7c5c8cb`).

## Obiettivo (un micro-task, poi STOP)
Dare all'utente un modo esplicito di **riprendere il loop dopo un hard-stop**.
Quando il cap sulle iterazioni (`BudgetTracker`) o il breaker di stagnazione
(`StallDetector`) staccano, l'Agent emette `ToolPart::LoopHalted` e termina: il
`tokio::spawn` finisce, `inference_task` torna `None`, lo stato torna idle.
Serve un comando UI **`/continue`** che, **solo dopo un halt**, ri-innesca una
nuova inferenza (nuovo Agent) con il transcript corrente (`app.messages`) e un
**budget fresco** — cap e breaker ripartono da zero. La conferma è l'azione
dell'utente che digita il comando.

## Nota di design (perché UI e non un tool del modello)
Il requisito originale diceva "un tool per far ripartire su conferma utente", ma
l'analisi del codice mostra un paradosso: dopo l'hard-stop il loop è **già
terminato** e il modello non gira più, quindi non può invocare alcun tool per
far ripartire. Il meccanismo corretto per "riprendere dopo lo stop su conferma"
è **lato UI** (un comando/azione dell'utente), non un tool del modello. Questo è
già stato deciso (nxm-decision, conf. 0.955) — NON riaprirlo.

## Skill
**`implement`** — è un'integrazione UI/stato (handler + App + main spawn), non
logica algoritmica pura da TDD; ma ogni requisito resta con il suo test (regola
7). Usa i test d'integrazione esistenti come modello.

## Contesto (già deciso — non riaprire)
- Meccanismo: **comando UI `/continue`** (lato `handler.rs`/`App`/`main.rs`),
  NON un tool del modello, NON `needs_approval`.
- Ripresa = **ri-spawn di un nuovo Agent** con `app.messages` come
  `initial_msgs` e budget fresco (lo spawn in `main.rs` già fa
  `initial_msgs = app.messages.clone()` e costruisce `BudgetTracker`/
  `StallDetector` nuovi a ogni inferenza — quindi il budget riparte da zero
  automaticamente). NON serve toccare `Agent`/`bounded.rs`/`circuit_breaker.rs`.
- **Guardia di stato**: `/continue` ha effetto **solo se l'ultimo turno è finito
  con un halt**. In ogni altro caso (nessun halt, loop mai partito, loop in
  corso) deve essere un **no-op con messaggio di stato** chiaro (es. "nessun
  loop da riprendere"). Serve quindi un flag in `App` (es.
  `pub halted: bool` o `last_halt: Option<...>`) impostato a `true` quando la UI
  consuma `ToolPart::LoopHalted` (oggi in `main.rs` riga ~453) e azzerato quando
  parte una nuova inferenza.
- Riuso del flusso di spawn esistente: la ripresa imposta `pending_message` +
  `RunState::Thinking` esattamente come un invio utente, così il blocco "Start
  inference" già presente in `main.rs` fa il resto. NON duplicare la logica di
  spawn. (Se `pending_message` richiede un contenuto, usa un marcatore interno
  o un `continue` implicito: scegli l'approccio minimale che riusa il blocco
  esistente senza inviare un nuovo messaggio utente visibile nel transcript.)
- Confine architetturale invariante: l'`Agent` non tocca `App`; la UI mirrora i
  `ToolPart` in `app.messages`. Il comando `/continue` vive interamente
  UI-side.

## Rules da rispettare
- Repo corrente: `RULES.md` (max 300 righe/file; NO `unwrap`/`expect` in prod;
  `tracing` strutturato `nexum::<module>::<feature>` su ogni funzione pubblica
  nuova; test in `tests/<module>.rs`, MAI nuovo `#[cfg(test)]` inline; doc `///`
  con esempio sulle API pubbliche nuove; aggiornare `CHANGELOG.md` e
  `CONTEXT.md`; Conventional Commits).
- Ecosistema: `nxm-docs/RULES.md` e doc pertinenti (cerca con `nxm-memory`).
- Conflitto scope↔rule già risolto: aggiungere la variante `Command::Continue`
  in `src/commands.rs`, il ramo in `src/handler.rs`, il flag in `src/app.rs`, il
  reset del flag in `src/main.rs`, e aggiornare `CHANGELOG.md`/`CONTEXT.md`
  **sono autorizzati** e NON sono "uscire dallo scope".

## Requisiti (ognuno con il suo test — vedi regola invariante 7)
- **R1 — Parsing comando.** In `src/commands.rs`, aggiungere `Command::Continue`
  e farlo riconoscere da `parse_command` per l'input `/continue` (e un alias
  breve se coerente con gli altri, es. `/cont` — scegli e documenta).
  + **test** `tests/` (dove vivono i test di `parse_command`; cerca con
    `nxm-memory` `search_code "parse_command"`): `parse_command("/continue")`
    ritorna `Command::Continue`.
- **R2 — Flag di halt in App.** Aggiungere a `App` un flag (es.
  `pub halted: bool`, default `false`) impostato a `true` quando la UI consuma
  `ToolPart::LoopHalted` (`main.rs`, ramo esistente) e **riportato a `false`**
  quando parte una nuova inferenza (nel blocco "Start inference").
  + **test** `tests/`: un test su `App` (costruzione + transizione) che verifica
    il default `false`; se la logica di set/reset è estraibile in un metodo
    pubblico di `App` (es. `App::mark_halted()` / reset), testalo direttamente.
    Se il set avviene solo nel loop di `main.rs` (non testabile in unità),
    estrai la transizione in un metodo di `App` e testa quel metodo.
- **R3 — `/continue` riprende solo dopo un halt.** Nel ramo
  `Command::Continue` di `handler.rs`: se `app.halted == true`, imposta lo stato
  per far ripartire una nuova inferenza (il flusso `pending_message` +
  `RunState::Thinking` riusato dal blocco di spawn), azzera `halted`, e dà un
  feedback di stato ("ripresa del loop"). Se `app.halted == false`, **no-op**
  con messaggio ("nessun loop da riprendere"). NON deve crashare né partire a
  vuoto.
  + **test** `tests/`: chiamando la gestione del comando con `halted=true` lo
    stato passa a pronto-per-inferenza (es. `RunState::Thinking` e
    `pending_message` valorizzato) e `halted` torna `false`; con `halted=false`
    lo stato resta invariato e viene impostato un messaggio di stato. (Se il
    dispatch del comando non è direttamente invocabile dai test, estrai la
    logica del ramo in un metodo pubblico di `App`, es.
    `App::continue_loop() -> bool`, e testa quello.)

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/loopeng-01-03-continue-after-halt`.
- NON committare su `main`/develop. Il lavoro va **validato prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — valida il codice Rust passando `repo_path` (NON `cargo` da
  shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`), `cargo_fmt_check`,
  `cargo_test`. **Prima di dichiarare "fatto/mergeabile", chiama `run_gate` e
  procedi solo su `verdict:"GO"`.** NOTA: su `main` esiste un NO-GO preesistente
  per ~20 file `src/md/*` non formattati, **non tuo e fuori scope** — verifica
  che i TUOI file NON compaiano tra gli offending di `cargo_fmt_check`. Fallback
  a `cargo`.
- `nxm-memory` — cerca rules/contesto/precedenti (pattern dei comandi in
  `handler.rs`, test di `parse_command`, come `main.rs` consuma `LoopHalted` e
  avvia l'inferenza).
- `nxm-decision` — categorizza eventuali bivi di implementazione.
- `nxm-session` — leggi/aggiorna lo stato di sessione `loopeng-01` se pertinente.

## Divieti
- NON implementare un tool `resume_loop` del modello: è il paradosso descritto
  nella Nota di design.
- NON toccare `Agent`/`bounded.rs`/`circuit_breaker.rs`: il budget fresco arriva
  gratis dal ri-spawn.
- NON duplicare la logica di spawn dell'Agent in `main.rs`: riusa il blocco
  "Start inference" tramite `pending_message` + `RunState::Thinking`.
- NON implementare auto-continue (ripresa automatica senza l'utente): la
  conferma utente è il punto.
- NON toccare i file `src/md/*` (debito fmt preesistente).
- NON aggiungere nuovi `#[cfg(test)]` inline nei file di produzione.
- NON uscire dallo scope — MA le rules prevalgono (split >300 righe, CHANGELOG,
  test in `tests/`). In caso di conflitto scope↔rule, FERMATI e chiedi.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo micro-task, poi FERMATI.
- Mostra il diff e come hai verificato (output di `run_gate` / test pertinenti).
- **Consegna a un revisore esterno** per la review — riferimento skill
  `code-review`. Il branch si valida/mergia **solo dopo** review approvata.
- Se c'è ambiguità, FERMATI e chiedi.

## Verifica attesa
```bash
# Preferito: diagnostica tipata via nxm-mcp-exec run_gate (repo_path = root del repo)
# I tuoi file NON devono comparire tra gli offending di cargo_fmt_check.
# Fallback shell:
cargo fmt --check -- src/commands.rs src/handler.rs src/app.rs src/main.rs
cargo clippy --all-targets -- -D warnings
cargo test            # suite completa, nessuna regressione; i nuovi test verdi
```
