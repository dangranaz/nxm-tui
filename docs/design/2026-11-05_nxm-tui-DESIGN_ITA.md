# nxm-tui — Design (riassunto auto-contenuto)

> **Prodotto:** `nxm-tui` (repo `nxm-tui/`, mirror pubblico di `dangranaz/nxm-tui`).
> **Stato:** work-in-progress su `main` + feature branch (vedi § Status & branch).
> **Origine:** ticket wayfinder `docs/wayfinder/tickets/` + handoff `docs/handoffs/` + loop engineering `loopeng-01` (loop bounded, circuit breaker, continue-after-halt).
> **Companion ENG:** `2026-11-05_nxm-tui-DESIGN_ENG.md` (tenere in sync).
> **Target:** pubblicazione come WIP pubblico su nexum.framer.ai.

## Cos'è e perché

`nxm-tui` è un **client chat da terminale** per qualsiasi server LLM compatibile OpenAI (Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM). È un **client puro**: niente gestione server, niente rilevamento HW, niente caricamento modelli — si connette e si chatta.

Esiste perché i motori di inferenza NXM (`engine-mlx`, `engine-metal`, `nxm-memory`) avevano bisogno di un frontend leggero, guidato da tastiera, che funzioni interamente nel terminale, supporti streaming SSE, tool call agentiche e gestione delle API key per provider senza mai scrivere segreti su disco.

A differenza di una web UI, `nxm-tui` gira localmente, ridisegna solo al cambio (nessun redraw a 60 Hz), e tratta la conversazione come un transcript strutturato che un loop agentico può estendere con tool call.

## Interfaccia pubblica (comandi TUI + slash command)

Transport: **TUI nativo** (ratatui + crossterm, alternate screen, raw mode). Una sessione interattiva, niente JSON-RPC.

### Slash command

| Comando | Azione |
|---|---|
| `/provider` | Elenca i provider disponibili |
| `/provider add <name> <url>` | Aggiunge un provider OpenAI-compatibile custom |
| `/provider use <name>` | Passa a un provider |
| `/provider remove <name>` | Rimuove un provider custom |
| `/provider set-key <name> <key>` | Memorizza API key per un provider (tramite KeyStore) |
| `/provider remove-key <name>` | Cancella una API key memorizzata |
| `/continue` (alias `/cont`) | Riprende dopo un hard-halt (cap budget o circuit breaker) con un nuovo Agent e lo stesso transcript |
| `/tools` (Ctrl+E) | Apre l'overlay strumenti: abilita/disabilita tool per checkbox; `a` = abilita tutti, `n` = disabilita tutti |
| `/todo` (Ctrl+D) | Attiva/disattiva l'overlay checklist todo (sola lettura) |
| `Ctrl+R` | Attiva/disattiva l'overlay ragionamento (stream `<thinking>` live) |
| `Ctrl+O` | Attiva/disattiva l'overlay dettaglio tool (ultima invocazione args + output) |
| `Ctrl+M` | Attiva/disattiva l'overlay metriche per-token |
| `Esc` | Chiude qualsiasi overlay aperto / cancella prompt passphrase in attesa |
| `Ctrl-C` / `Ctrl-Q` | Esci (aborta prima l'agent task in volo) |

### Provider built-in

| Provider | Endpoint | Auth |
|---|---|---|
| Nexum Inferentia | `http://127.0.0.1:11434` | none |
| Ollama | `http://127.0.0.1:11434/v1` | none |
| LM Studio | `http://127.0.0.1:1234/v1` | none |
| NVIDIA NIM | `https://integrate.api.nvidia.com/v1` | `NVIDIA_API_KEY` |

Le API key vengono lette dall'ambiente (`NEXUM_API_KEY`, poi `OPENAI_API_KEY`) e **non** vengono mai scritte su disco a meno che l'utente non le memorizzi esplicitamente tramite il KeyStore.

## Decisioni chiave (as-built, verificate)

1. **Client puro, niente lato server.** `nxm-tui` consuma solo `GET /v1/models` e `POST /v1/chat/completions` (SSE). Non avvia, compila né carica modelli. Questo è compito del motore (`engine-mlx`, `engine-metal`, `nxm-memory`).
2. **Draw-on-change + polling idle a 30 Hz.** Il frame viene ricostruito solo quando cambia qualcosa di visibile (tasto, resize, drenaggio token, cambio stato/salute, tick spinner). I tick idle non fanno lavoro di disegno — nessun redraw a 60 Hz.
3. **Cache di render per messaggio.** Durante lo streaming si ridisegna un solo messaggio, non O(N) sull'intera cronologia.
4. **Loop agentico con cap rigido + breaker di stagnazione.** `Agent::run` streama una risposta, risolve le `tool_calls` tramite `nexum-tools`, aggiunge i risultati `role:"tool"` al transcript e riesegue finché il modello non emette `finish_reason: "stop"` — o finché `bounded::BudgetTracker` (`DEFAULT_MAX_STEPS = 15`) ferma il loop, o `circuit_breaker::StallDetector` (`DEFAULT_STALL_THRESHOLD = 3`) scatta su tool call identiche consecutive. Entrambi emettono `ToolPart::LoopHalted { reason, steps }`.
5. **Niente `unwrap`/`expect` in produzione.** Il parsing fallibile mappa a errori `String`. `anyhow::Result` per errori a livello app, `thiserror` per errori di libreria tipizzati (per `RULES.md`).
6. **Seam KeyStore iniettabile (I1/R5).** Trait `KeyStore` con tre backend: `EnvStore` (solo env), `OsKeychainStore` (OS-nativo via `keyring`), `EncryptedFileStore` (AES-256-GCM + Argon2id, `keys.enc` 0600). I valori segreti sono avvolti in `Zeroizing` e mai loggati.
7. **Subsistema host MCP (stdio).** `nxm-tui` genera un processo figlio MCP server (`--transport stdio`), completa la handshake `initialize` dichiarando la capacità `sampling`, e serve `sampling/createMessage` collegandolo al backend LLM (auto-approvato). Clean no-op quando disabilitato o binario assente — non blocca mai la chat.
8. **Pipeline Markdown V3 (adottata da grok-build `xai-grok-markdown`).** Port puro-Rust di pulldown-cmark → output ratatui stilizzato, con evidenziazione sintassi syntect (`default-fancy`, niente onig/C), wrapping consapevole della larghezza cella (CJK/emoji), hyperlink OSC 8, source map per copia/selezione. Renderer V1 legacy mantenuto come fallback dietro `NXM_USE_NEW_MD=1` (default OFF).
9. **Selezione tool ortogonale (T1–MT3).** `Registry` traccia `enabled: HashSet<String>` ortogonale al gating (`needs_approval`). `manifest_for(task)` fa selezione automatica per-task (Core sempre incluso come fail-safe) unito ai tool abilitati manualmente. Set manuale default vuoto; l'utente aggiunge tramite overlay.

## Layout del codice

```
src/
  main.rs            — ingresso TUI: terminal ratatui, loop eventi principale, spawn agent, connessione MCP
  lib.rs             — grafo moduli pubblico (bin + lib condividono un unico albero crate)
  app.rs             — stato App: RunState, AgentMode, messaggi, metriche, provider, passphrase keystore
  agent.rs           — loop agentico streaming (budget bounded + circuit breaker)
  bounded.rs         — BudgetTracker: cap iterazioni rigido (DEFAULT_MAX_STEPS = 15)
  circuit_breaker.rs — StallDetector: tripping su tool call identiche consecutive (DEFAULT_STALL_THRESHOLD = 3)
  commands.rs        — parsing slash command (parse_command, enum Command)
  config.rs          — TuiConfig: persistenza TOML (~/.nexum/tui.toml), max_steps, stall_threshold
  connection.rs      — stream chat SSE (path single-turn, pre-agentico)
  event.rs           — AppEvent: Key / Resize / Tick polling
  handler.rs         — dispatch tasti verso comandi / overlay / azioni agent
  keys.rs            — modalità risoluzione chiavi (EnvFirst / KeychainFirst / EncryptedFile)
  keystore/          — trait KeyStore + EnvStore / OsKeychainStore / EncryptedFileStore
  mcp/               — host MCP: protocollo, transport (stdio), bridge sampling, session client
  md/                — renderer Markdown V3 (core, parse, render, buffers, wrap, link, syntax, theme)
  markdown.rs        — dispatcher: legacy V1 vs pipeline V3 (flag env NXM_USE_NEW_MD)
  message.rs         — tipi di dominio Message / Role
  metrics.rs         — metriche per-token (caratteri, token/sec)
  mode_bar.rs        — barra stato: modalità, provider, modello, indicatore halted
  models.rs          — fetch /v1/models + ModelEntry
  provider.rs        — preset provider + gestione provider custom
  session.rs         — salvataggio/caricamento sessione (~/.nexum/sessions/)
  sidebar.rs         — render sidebar sinistra (sessioni, provider, modelli)
  tool_types.rs      — ToolPart / ToolInvocation / ToolResult / ApprovalRequest
  tools_overlay.rs   — overlay abilita/disabilita strumenti (T1)
  todo.rs / todo_tool.rs / todo_overlay.rs — stato lista todo + tool + overlay sola lettura
  ui.rs              — render di alto livello: layout, overlay, mode bar
  utils/             — piccoli helper puri
tests/               — tutti i test di integrazione (mai #[cfg(test)] inline in src/, per RULES)
```

## Come verificare

```bash
cd nxm-tui

# Formattazione + lint + test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Build da sorgente
cargo build --release
./target/release/nxm-tui

# Install via script (scarica binario precompilato)
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh

# Accettazione (interattiva):
#   1. nxm-tui → appare il menu provider se nessun server rilevato
#   2. /provider add ollama http://127.0.0.1:11434/v1
#   3. /provider use ollama → stato = Running
#   4. Invia un messaggio → i token streaming SSE appaiono
#   5. Ctrl+E → overlay strumenti attiva/disabilita read_file/list_resources/web_search
#   6. Ctrl+D → overlay todo appare (checklist sola lettura)
#   7. Dopo un LoopHalted → /continue riprende con Agent nuovo + stesso transcript
```

Prerequisito per la build: Rust stabile (edition 2021). Binari precompilati per Apple Silicon macOS e Linux x86_64 pubblicati a ogni release.

## Fuori scope (follow-up noti)

- **Transport HTTP stateless** per il TUI stesso (è un TUI nativo stdio; MCP è processo figlio stdio).
- **Persistenza selezione tool** su TOML (stato overlay in-session solo oggi; fase 2).
- **MCP `tools/call`** — oggi viene servito solo `sampling/createMessage`; client MCP completo è nella roadmap di raffinamento (`docs/mcp/ARCHITECTURE.md`).
- **Configurabilità ricca dei gate** — loop bounded usa `DEFAULT_MAX_STEPS` / `DEFAULT_STALL_THRESHOLD` con override da config; nessun budget per-step per tool ancora.
- **Resume/ask-to-continue** oltre `/continue` (nessun loop Ralph, nessun gating budget token).
- **Install.sh + registrazione MCP** nelle config degli agent (documentata ma non automatizzata).

## Status & branch

| Branch | Stato |
|---|---|
| `main` | loopeng-01-01 merged (esecuzione bounded, cap rigido su Agent::run) |
| `work/loopeng-01-02-circuit-breaker` | in attesa di review esterna (breaker di stagnazione) |
| `work/loopeng-01-03-continue-after-halt` | in corso (comando `/continue`) |
| `feat/tools-select-mechanism` | MT-1..MT-3 (selezione tool per-task) in attesa di review esterna |
| `feat/tools-todo` | MT-1..MT-3 (tool todo + overlay) in attesa di review esterna |

## Storia della review (processo)

Ogni micro-task è implementato da agent esterni e **revisionato da un revisore esterno fisso** (due assi Standards + Spec) prima del merge. La review ha intercettato difetti che un auto-approve avrebbe lasciato passare (diff vuoti, gate dichiarati ma non eseguiti, invocazione backend non funzionante). Contromisura adottata: clausola di consegna che impone gate eseguiti sul commit finale più verifica indipendente con call reali. Tutti i branch in sospeso sopra attendono questa review prima del `--ff-only` nei branch target.
