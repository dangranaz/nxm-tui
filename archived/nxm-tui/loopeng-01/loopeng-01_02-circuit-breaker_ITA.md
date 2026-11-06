# Delega — 02 Circuit Breaker (rilevamento stagnazione in Agent::run)

> **Stato:** PRONTO · **Creato:** 2026-10-26
> Companion: `loopeng-01_02-circuit-breaker_ENG.md` (tenere in sync).
> Origine design: sessione nxm-session `loopeng-01` (loop engineering) +
> `docs/wayfinder/research/loop-engineering.md` §3 pattern 8 (Circuit Breaker),
> §5 (P0) e §7 punto 2.
> Precedente consolidato: `loopeng-01_01-bounded-execution` (già su `main`,
> commit `bed58d3`) — questo task ne riusa esattamente lo stile e i pattern.

## Obiettivo (un micro-task, poi STOP)
Dare a `Agent::run` (`src/agent.rs`) un **circuit breaker sulla stagnazione**:
se il modello ripete lo **stesso identico tool call** (stesso nome + stessi
argomenti) per **N cicli consecutivi** (default **3**) senza progresso, il loop
stacca (**hard stop**), emette `ToolPart::LoopHalted` con un `reason` che spiega
la stagnazione e ritorna `Ok(())` senza panicare. È il complemento del cap sulle
iterazioni (Bounded Execution) già presente: il cap limita la durata totale, il
breaker interrompe un ciclo *bloccato* prima che bruci tutto il budget.
Niente git, niente branch runtime dell'Agent, niente `run_gate`, niente Ralph
loop: SOLO il rilevamento di stagnazione.

## Skill
**`tdd`** — la logica del detector è pura e verificabile; va scritta test-first
(signature del ciclo → conteggio ripetizioni → trip). Un test d'integrazione che
fa scattare il breaker con un server che ripete sempre lo stesso tool call è il
gate di accettazione (regola 7).

## Contesto (già deciso — non riaprire)
- **Segnale di stagnazione** = stesso **nome + argomenti** dei tool call emessi
  in un ciclo, ripetuti per N cicli consecutivi. Calcola una *signature*
  deterministica del set di tool call di un'iterazione (es. stringa/hash
  ordinato di `name` + `args` di ciascun `ToolInvocation`). Due cicli con la
  stessa signature contano come ripetizione; una signature diversa **azzera** il
  contatore (c'è stato progresso).
- **Soglia N default = 3** ripetizioni consecutive identiche → trip.
- Alla scadenza: **hard stop** (ritorna `Ok(())`). NON ask-to-continue, NON
  resume: quelli sono micro-task separati.
- **Riuso del variant UI esistente**: NON aggiungere nuovi variant a
  `ToolPart`. Riusa `ToolPart::LoopHalted { reason, steps }` (introdotto da
  loopeng-01_01) con un `reason` che descrive la stagnazione, es.
  `"stalled: identical tool call repeated 3× (<nome-tool>)"`. `steps` =
  l'indice di iterazione corrente quando scatta il trip. NON modificare
  `src/tool_types.rs` se non è strettamente necessario.
- **Pattern di riferimento OBBLIGATORIO**: imita `src/bounded.rs` as-built
  (logica pura, zero I/O, nessun clock/rete, `tracing` con target
  `nexum::<module>::<feature>`, doc `///` con doctest, enum di esito). Leggilo
  prima di iniziare.
- Il breaker è separato dal `BudgetTracker`: NON fonderli. Convivono nel loop di
  `run()` — ogni iterazione passa prima dal `budget.tick()` (già presente) e poi
  dal detector dopo `stream_turn` (quando conosci i tool call del ciclo).
- Confine architetturale invariante: `Agent` vive in `tokio::spawn`, **non tocca
  `App`**, comunica solo via `mpsc::UnboundedSender<ToolPart>`. Il breaker vive
  DENTRO l'Agent ed emette via `ToolPart`. NON far toccare `App`/filesystem/git
  all'Agent.
- `src/agent.rs` è ~483 righe, già oltre il limite di 300 righe di `RULES.md`
  (condizione preesistente e nota — la logica pura è fuori, in moduli): per
  questo la logica del breaker va in un **nuovo modulo** `src/circuit_breaker.rs`
  (<300 righe), il cablaggio in `agent.rs` deve restare minimo (poche righe).
- `src/agent.rs` ha un `#[cfg(test)] mod tests` inline **preesistente**: NON
  aggiungere altri test inline lì. I test nuovi vanno in `tests/` (RULES.md).

## Rules da rispettare
- Repo corrente: `RULES.md` (max 300 righe/file; NO `unwrap`/`expect` in prod;
  `tracing` strutturato `nexum::<module>::<feature>` su ogni funzione pubblica;
  test in `tests/<module>.rs`, MAI nuovo `#[cfg(test)]` inline; doc `///` con
  esempio sulle API pubbliche; pin versioni esatte; aggiornare `CHANGELOG.md` e
  `CONTEXT.md`; Conventional Commits).
- Ecosistema: `nxm-docs/RULES.md` e doc pertinenti (cerca con `nxm-memory`).
- Conflitto scope↔rule già risolto in questo brief: creare il nuovo modulo
  `src/circuit_breaker.rs`, aggiungere il campo in `src/config.rs`, il cablaggio
  in `src/agent.rs`/`src/main.rs`, registrarlo in `src/lib.rs`, e aggiornare
  `CHANGELOG.md`/`CONTEXT.md` **sono autorizzati** e NON sono "uscire dallo
  scope".

## Requisiti (ognuno con il suo test — vedi regola invariante 7)
- **R1 — Modulo detector puro.** Nuovo file `src/circuit_breaker.rs` (<300
  righe) con la logica pura: una costante `DEFAULT_STALL_THRESHOLD: u32 = 3`, e
  un tipo (es. `StallDetector`) con `new(threshold: u32)` e un metodo che riceve
  la *signature* di un ciclo (es. `observe(&mut self, signature: &str) -> Stall`)
  e ritorna un enum `Stall::Ok` / `Stall::Tripped { repeats, signature }`.
  Semantica: signature uguale alla precedente → incrementa il contatore di
  ripetizioni; signature diversa → azzera a 1; quando le ripetizioni consecutive
  raggiungono `threshold` → `Tripped`. Zero I/O, zero rete. Funzioni pubbliche
  con `tracing` + doc `///` con esempio.
  + **test** `tests/circuit_breaker.rs`: con `StallDetector::new(3)`, osservare
    la stessa signature `"A"` tre volte di fila → le prime due `Ok`, la terza
    `Tripped { repeats: 3, .. }`; inserire una signature diversa `"B"` in mezzo
    **azzera** (la sequenza `A,A,B,A,A` NON scatta). `new(1)` scatta alla prima
    osservazione ripetuta secondo la stessa regola (documenta e verifica il caso
    di soglia bassa che scegli).
- **R2 — Signature deterministica.** Una funzione pura (in
  `circuit_breaker.rs`) che, dato un `&[ToolInvocation]` (o `&[(name, args)]`
  per non accoppiare il modulo a `tool_types` se preferisci — scegli e
  documenta), produce una `String` signature **deterministica e indipendente
  dall'ordine** dei call entro il ciclo (ordina prima di concatenare, così due
  cicli con gli stessi call in ordine diverso collidono correttamente).
  + **test** `tests/circuit_breaker.rs`: due input con gli stessi (name,args) in
    ordine diverso producono la **stessa** signature; cambiare un singolo arg
    produce una signature **diversa**.
- **R3 — Breaker cablato in Agent::run.** `Agent::new` accetta un
  `stall_threshold: u32` (aggiungere il parametro, dopo `max_steps`); `run()`
  costruisce uno `StallDetector`; dopo `stream_turn` (quando hai i `calls`),
  calcola la signature e chiama `observe`; se `Tripped`, emette
  `tx.send(ToolPart::LoopHalted { reason, steps })` con `reason` di stagnazione e
  `steps` = indice iterazione corrente, poi ritorna `Ok(())` SENZA panicare. Il
  comportamento normale (esce su `calls.is_empty()`, oppure l'halt del
  `BudgetTracker`) resta invariato. Il breaker NON deve scattare quando i tool
  call cambiano tra un ciclo e l'altro. Aggiornare le doc-test / call-site che
  invocano `Agent::new` con la nuova firma (c'è un doctest in `request_body()` e
  vari `tests/*` — cercali e aggiornali tutti).
  + **test** `tests/circuit_breaker.rs` (integrazione): un `Agent` con
    `stall_threshold` basso (es. 2) e `max_steps` abbastanza alto da non
    interferire, `approval_tx: None` (headless), puntato a un server HTTP fake
    che risponde SEMPRE con lo **stesso identico** `tool_calls` (stesso name +
    args, mai `finish_reason:"stop"`), deve terminare per **stagnazione** dopo le
    ripetizioni di soglia ed emettere un `ToolPart::LoopHalted` il cui `reason`
    indica la stagnazione (asserisci che il testo contiene "stall"/"stalled").
    Riusa l'helper di server fake dei test del micro-task precedente se esiste
    (cerca con `nxm-memory` `search_code "LoopHalted"` / `tests/bounded.rs`).
- **R4 — Config esterna.** In `src/config.rs`, aggiungere a `TuiConfig` il campo
  `pub stall_threshold: Option<u32>` con `#[serde(default)]`, inizializzato a
  `None` in `Default` (il default si applica al sito d'uso con
  `.unwrap_or(DEFAULT_STALL_THRESHOLD)`, così `circuit_breaker.rs` resta la fonte
  unica del default — stesso schema di `max_steps`). In `src/main.rs`, al sito di
  `Agent::new`, leggere
  `config.stall_threshold.unwrap_or(nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD)`
  e passarlo.
  + **test**: estendere `tests/config.rs` con un round-trip `toml` di un
    `TuiConfig` con `stall_threshold: Some(5)` (serialize→deserialize, il valore
    sopravvive) e un toml SENZA il campo che carica `None`
    (grazie a `#[serde(default)]`).

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/loopeng-01-02-circuit-breaker`.
- NON committare su `main`/develop. Il lavoro va **validato prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — valida il codice Rust passando `repo_path` (NON `cargo` da
  shell): `cargo_check`, `cargo_clippy` (`deny_warnings:true`), `cargo_fmt_check`,
  `cargo_test`. **Prima di dichiarare "fatto/mergeabile", chiama `run_gate` e
  procedi solo su `verdict:"GO"`.** NOTA: su `main` esiste un NO-GO preesistente
  per ~20 file `src/md/*` non formattati, **non tuo e fuori scope** — verifica
  che i TUOI file siano fmt-clean (`cargo_fmt_check` elenca gli offending files:
  i tuoi NON devono comparire). Fallback a `cargo` se il server non è
  disponibile.
- `nxm-memory` — cerca rules/contesto/precedenti (pattern di `bounded.rs`,
  helper di server fake in `tests/bounded.rs`, convenzioni `ToolPart`).
- `nxm-decision` — categorizza eventuali bivi di implementazione.
- `nxm-session` — leggi/aggiorna lo stato di sessione `loopeng-01` se pertinente.

## Divieti
- NON fondere il breaker col `BudgetTracker`: sono due controlli distinti.
- NON aggiungere nuovi variant a `ToolPart` (riusa `LoopHalted`).
- NON implementare resume/ask-to-continue, Ralph loop, `run_gate` integration,
  budget token, reset/compaction del contesto: fuori scope.
- NON far toccare `App`, il filesystem o git all'`Agent` a runtime.
- NON aggiungere nuovi `#[cfg(test)]` inline in `src/agent.rs`.
- NON toccare i file `src/md/*` (debito fmt preesistente, non è questo task).
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
# I tuoi file NON devono comparire tra gli offending di cargo_fmt_check.
# Fallback shell:
cargo fmt --check -- src/circuit_breaker.rs src/agent.rs src/config.rs src/main.rs src/lib.rs
cargo clippy --all-targets -- -D warnings
cargo test --test circuit_breaker
cargo test            # suite completa, nessuna regressione
```
