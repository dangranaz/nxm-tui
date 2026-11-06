# Delega — 01 Bounded Execution (cap iterazioni in Agent::run)

> **Stato:** PRONTO · **Creato:** 2026-10-26
> Companion: `loopeng-01_01-bounded-execution_ENG.md` (tenere in sync).
> Origine design: sessione nxm-session `loopeng-01` (loop engineering) +
> `docs/wayfinder/research/loop-engineering.md` §5 (P0 Bounded Execution) e §7 punto 1.

## Obiettivo (un micro-task, poi STOP)
Dare a `Agent::run` (`src/agent.rs`) un **tetto duro sul numero di iterazioni**
del loop ReAct. Oggi `run()` è un `loop {}` senza cap: esce solo quando il
modello smette di chiamare tool. Serve un cap configurabile (default **15**):
quando viene raggiunto, il loop si ferma (**hard stop**), emette un nuovo
`ToolPart::LoopHalted { reason, steps }` verso la UI e ritorna `Ok(())` senza
panicare. Niente git, niente branch di lavoro dell'agente a runtime, niente
`run_gate`: SOLO il cap. (Il "branch misurabile" e il validatore esterno sono
il passo 5 del loop engineering, un altro micro-task.)

## Skill
**`tdd`** — la logica del cap è pura e verificabile; va scritta test-first
(contatore → condizione di stop → emissione del ToolPart). Un test
d'integrazione che fa scattare il cap è il gate di accettazione (regola 7).

## Contesto (già deciso — non riaprire)
- Cosa si conta: **iterazioni del loop** (= turni del modello), non tool-call
  né token. Il budget token è una fase successiva, FUORI scope qui.
- Default cap = **15** (l'utente ha scelto 15, non 25).
- Alla scadenza: **hard stop** (il loop termina e ritorna `Ok(())`), NON
  ask-to-continue. Il tool di ripresa su conferma utente è un micro-task
  separato (loopeng-01_02), NON farlo qui.
- Config in **file esterno**: esiste già `TuiConfig` persistito in
  `~/.nexum/tui.toml` (`src/config.rs`). Aggiungere lì `max_steps`.
- Confine architetturale invariante: `Agent` vive in `tokio::spawn`, **non
  tocca `App`**, comunica solo via `mpsc::UnboundedSender<ToolPart>`. Il cap e
  la sua segnalazione devono vivere DENTRO l'Agent ed emettere via `ToolPart`.
  NON far toccare `App`/filesystem/git all'Agent.
- `src/agent.rs` è **460 righe**, già oltre il limite di 300 righe di
  `RULES.md`. Per questo la logica pura del cap va estratta in un **nuovo
  modulo** `src/bounded.rs` (vedi Requisiti), così agent.rs non peggiora e la
  logica è testabile in isolamento.
- `src/agent.rs` ha un `#[cfg(test)] mod tests` inline **preesistente**: NON
  aggiungere altri test inline lì. I test nuovi vanno in `tests/` (RULES.md).

## Rules da rispettare
- Repo corrente: `RULES.md` (max 300 righe/file; NO `unwrap`/`expect` in prod;
  `tracing` strutturato con target `nexum::<module>::<feature>` su ogni funzione
  pubblica; test in `tests/<module>.rs`, MAI `#[cfg(test)]` inline nuovo; doc
  `///` con esempio sulle API pubbliche; pin versioni esatte; aggiornare
  `CHANGELOG.md` e `CONTEXT.md`; Conventional Commits).
- Ecosistema: `nxm-docs/RULES.md` e doc pertinenti (cerca con `nxm-memory`).
- Conflitto scope↔rule già risolto in questo brief: la creazione del nuovo
  modulo `src/bounded.rs`, l'aggiunta del variant in `src/tool_types.rs`, il
  campo in `src/config.rs`, il cablaggio in `src/main.rs`, e l'aggiornamento di
  `CHANGELOG.md`/`CONTEXT.md` **sono autorizzati** e NON sono "uscire dallo
  scope".

## Requisiti (ognuno con il suo test — vedi regola invariante 7)
- **R1 — Modulo bounded puro.** Nuovo file `src/bounded.rs` (<300 righe) con la
  logica pura del cap: una costante `DEFAULT_MAX_STEPS: u32 = 15`, e un tipo
  (es. `BudgetTracker`) con `new(max_steps: u32)` e un metodo che, chiamato a
  ogni iterazione, segnala se continuare o fermarsi (es. ritorna un enum
  `Budget::Continue` / `Budget::Halted { steps }`). Zero I/O, zero dipendenze
  dalla rete. Funzioni pubbliche con `tracing` + doc `///` con esempio.
  + **test** `tests/bounded.rs`: un `BudgetTracker::new(3)` ritorna `Continue`
    per i primi 3 tick e `Halted { steps: 3 }` al 4°; `new(0)` si ferma subito.
- **R2 — Variant UI.** In `src/tool_types.rs`, aggiungere
  `ToolPart::LoopHalted { reason: String, steps: u32 }` e il suo ramo in
  `render_lines()` (una riga leggibile, es. colore d'avviso, che riporta
  `reason` e `steps`). Mantenere `#[derive(Debug, Clone, PartialEq, Eq)]`.
  + **test** `tests/` (nuovo o `tests/agent_request.rs`-adiacente): costruire un
    `ToolPart::LoopHalted { reason: "...".into(), steps: 15 }`, chiamare
    `render_lines()` e asserire che l'output non è vuoto e contiene "15"
    (o il testo del reason).
- **R3 — Cap cablato in Agent::run.** `Agent::new` accetta un `max_steps: u32`
  (aggiungere il parametro); `run()` usa `BudgetTracker`: a ogni iterazione,
  prima o dopo `stream_turn`, incrementa; se il budget è `Halted`, emette
  `tx.send(ToolPart::LoopHalted { reason, steps })` e ritorna `Ok(())` SENZA
  panicare. Il comportamento normale (esce su `calls.is_empty()`) resta
  invariato quando il cap non scatta. Aggiornare la doc-test di
  `request_body()` che chiama `Agent::new(...)` con la nuova firma, e il
  doctest in testa a `run`/modulo se presente.
  + **test** `tests/bounded.rs` (integrazione): un `Agent` con `max_steps` basso
    (es. 2), con `approval_tx: None` (headless), puntato a un server HTTP fake
    (es. `wiremock` o un listener locale) che risponde SEMPRE con un
    `tool_calls` (mai `finish_reason:"stop"`), deve terminare dopo esattamente
    `max_steps` iterazioni ed emettere un `ToolPart::LoopHalted` sul canale.
    Il test fallisce (hang/timeout) se il cap non c'è. Se esiste già un helper
    di server fake nei test del repo (cerca con `nxm-memory`
    `search_code "bytes_stream"` / `tests/live_ollama.rs`), riusalo.
- **R4 — Config esterna.** In `src/config.rs`, aggiungere a `TuiConfig` il
  campo `pub max_steps: Option<u32>` con `#[serde(default)]`, e inizializzarlo
  in `Default` (a `None` o `Some(15)` — scegli `None` e applica il default al
  sito d'uso con `.unwrap_or(DEFAULT_MAX_STEPS)`, così `bounded.rs` resta la
  fonte unica del default). In `src/main.rs`, al sito di `Agent::new`, leggere
  `config.max_steps.unwrap_or(nxm_tui::bounded::DEFAULT_MAX_STEPS)` e passarlo.
  + **test**: estendere/aggiungere un test di `config` che round-trippa un
    `TuiConfig` con `max_steps: Some(7)` via `toml` (serialize→deserialize) e
    verifica che il valore sopravvive; e che un toml SENZA `max_steps` carica
    `None` (grazie a `#[serde(default)]`).

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/loopeng-01-01-bounded-execution`.
- NON committare su `main`/develop. Il lavoro va **validato prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — valida il codice Rust passando `repo_path` (NON `cargo` da
  shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`), `cargo_fmt_check`,
  `cargo_test`. **Prima di dichiarare "fatto/mergeabile", chiama `run_gate` e
  procedi solo su `verdict:"GO"`.** Fallback a `cargo` se il server non è
  disponibile.
- `nxm-memory` — cerca rules/contesto/precedenti (es. pattern dei test di
  streaming, helper di server fake, convenzioni `ToolPart`).
- `nxm-decision` — categorizza eventuali bivi di implementazione.
- `nxm-session` — leggi/aggiorna lo stato di sessione `loopeng-01` se pertinente.

## Divieti
- NON implementare il tool di ripresa (`resume_loop`) né alcun ask-to-continue:
  è il micro-task loopeng-01_02.
- NON implementare circuit breaker, Ralph loop, `run_gate` integration, budget
  token, reset/compaction del contesto: fuori scope.
- NON far toccare `App`, il filesystem o git all'`Agent` a runtime.
- NON aggiungere nuovi `#[cfg(test)]` inline in `src/agent.rs`.
- NON uscire dallo scope — MA le rules prevalgono: se un file supera 300 righe,
  splittalo; aggiorna CHANGELOG/CONTEXT; metti i test in `tests/`. In caso di
  conflitto scope↔rule, FERMATI e chiedi.
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
# verdict deve essere "GO".
# Fallback shell:
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --test bounded
cargo test            # suite completa, nessuna regressione
```
