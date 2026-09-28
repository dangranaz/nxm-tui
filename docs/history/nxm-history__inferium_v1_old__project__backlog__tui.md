# TUI — Piano di Implementazione

## Riferimenti

- **OpenCode** (159k★): client/server, TUI-first, plan/build mode, AGENTS.md
- **DeepSeek TUI** (25.9k★): Rust + ratatui, 3 modalità, skills, sessions, LSP
- **Stack nostro**: ratatui + crossterm già in dipendenze, architettura client/server già funzionante

---

## Fase 1 — Setup TUI

### Obiettivo

Al primo avvio, l'utente vede un setup wizard nel terminale che:
1. Rileva hardware con feedback visivo
2. Scarica il bootstrap model con progress bar
3. Esegue il setup agent (scelta modelli)
4. Mostra i modelli scelti e avvia download in background
5. Transiziona alla chat quando il bootstrap è pronto

### Layout

```
┌─────────────────────────────────────────────────────────────┐
│  ╔══════════════════════════════════════════════════════════╗ │
│  ║           I N F E R I U M   S E T U P                   ║ │
│  ╚══════════════════════════════════════════════════════════╝ │
│                                                              │
│  [✓] Hardware Detection                                      │
│      CPU: AMD Ryzen 5 5500U (12 cores)                       │
│      RAM: 24.0 GB                                            │
│      GPU: none                                               │
│                                                              │
│  [✓] Bootstrap Model                                         │
│      Qwen3-4B Q4_K_M (2.5 GB)                              │
│      ████████████████████████████████████████ 100%           │
│                                                              │
│  [⟳] Analyzing pipelines...                                  │
│      Setup agent selecting models for your hardware          │
│                                                              │
│  [ ] Download Models                                         │
│      Waiting for setup agent...                              │
│                                                              │
│                                                              │
│  ─────────────────────────────────────────────────────────── │
│  Press Ctrl+C to cancel                                      │
└─────────────────────────────────────────────────────────────┘
```

### Dopo setup agent:

```
│  [✓] Model Selection                                         │
│      reasoning: qwen3-4b-q4_k_m [già scaricato ✓]           │
│      coding:    deepseek-coder-v2-lite-16b-q4_k_m            │
│                                                              │
│  [⟳] Downloading Models                                      │
│      deepseek-coder-v2-lite-16b-q4_k_m (9.2 GB)            │
│      ████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 28% 4.2 MB/s │
│      ETA: ~12 min                                            │
│                                                              │
│  ─────────────────────────────────────────────────────────── │
│  Download in background. Press Enter to start chatting →     │
```

### Componenti ratatui

| Widget | Cosa renderizza |
|--------|----------------|
| `Block` + `Paragraph` | Header "INFERIUM SETUP" |
| `List` con icone | Steps con stato [✓] [⟳] [ ] |
| `Gauge` | Progress bar download |
| `Paragraph` | Info hardware, modelli scelti |
| `Paragraph` (footer) | Istruzioni (Ctrl+C, Enter) |

### Struttura codice

```
inferium-desktop/tui/src/
├── main.rs              # CLI + routing
├── client.rs            # HTTP client per engine
├── setup.rs             # → diventa modulo
├── setup/
│   ├── mod.rs           # Setup flow orchestrator
│   ├── ui.rs            # Rendering ratatui del setup
│   └── steps.rs         # Logica step (detect, download, agent)
└── chat/                # (Fase 2)
    ├── mod.rs
    └── ui.rs
```

---

## Fase 2 — Chat TUI

### Obiettivo

Dopo il setup (o se già completato), l'utente entra in modalità chat:
- Input in basso
- Risposte in streaming sopra
- Status bar con modello attivo, token/s, RAM

### Layout

```
┌─────────────────────────────────────────────────────────────┐
│  inferium v0.1.4 │ qwen3-4b-q4_k_m │ 24.0 GB │ ● ready     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  You: Refactora il modulo auth separando login da register   │
│                                                              │
│  AI: Analizzo la struttura del modulo auth...                │
│                                                              │
│  Ho identificato 3 task:                                     │
│  1. Creare src/auth/login.rs                                 │
│  2. Creare src/auth/registration.rs                          │
│  3. Aggiornare src/auth/mod.rs                               │
│                                                              │
│  Procedo con l'implementazione...█                           │
│                                                              │
│                                                              │
│                                                              │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│  > _                                                         │
├─────────────────────────────────────────────────────────────┤
│  /models  /switch  /status  /quit │ 18.2 tok/s │ Tab: plan  │
└─────────────────────────────────────────────────────────────┘
```

### Funzionalità

| Feature | Descrizione |
|---------|-------------|
| **Streaming** | Token appaiono uno alla volta (SSE dal server) |
| **History** | Scroll up per vedere messaggi precedenti |
| **Commands** | `/models`, `/switch`, `/status`, `/quit`, `/clear` |
| **Status bar** | Modello, RAM, tok/s, stato (ready/generating/loading) |
| **Tab** | Switch plan/build mode (come OpenCode) |
| **Ctrl+C** | Cancella generazione in corso |

### Client SSE

```rust
// Connessione streaming a /v1/chat/completions
// Ogni chunk SSE → append token al messaggio corrente
// "Done" → messaggio completo, torna a input
```

### Struttura codice

```
inferium-desktop/tui/src/chat/
├── mod.rs           # Chat loop principale
├── ui.rs            # Layout ratatui (header, messages, input, footer)
├── input.rs         # Gestione input + comandi /
├── streaming.rs     # Client SSE per token streaming
└── state.rs         # Stato conversazione (messages, mode, status)
```

---

## Dipendenze aggiuntive necessarie

```toml
# Già presenti:
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["json", "stream"] }

# Da aggiungere:
futures-util = "0.3"          # Per stream SSE
tokio-stream = "0.1"          # Stream utilities
unicode-width = "0.2"         # Per calcolo larghezza testo
```

---

## Stato Avanzamento

| # | Cosa | Stato |
|---|------|-------|
| 1 | Setup UI (detect + progress bar) | ✅ |
| 2 | Bootstrap download con Gauge | ✅ (stub — needs catalog integration) |
| 3 | Setup agent integration | ✅ (uses resolver, agent prompt ready) |
| 4 | Background download con progress | ✅ (stub — needs catalog integration) |
| 5 | Chat layout (header + messages + input) | ✅ |
| 6 | SSE streaming client | ✅ |
| 7 | Comandi / (models, switch, quit, clear, status) | ✅ |
| 8 | Status bar con metriche | ✅ |

## Contesto per Ripresa Sessione

### Dove siamo
- `inferium-desktop/tui/` ha già: CLI con clap, subcommands (Chat, Setup, Serve, Stop), client stub, setup stub
- Dipendenze ratatui + crossterm già nel Cargo.toml
- Il server (`inferium-engine`) funziona e risponde su porta 11435
- `inferium-pipeline` ha: parser YAML, resolver, setup agent, downloader, catalog refresh, self-upgrade
- `inferium-hwdetect` ha: `SystemSpecs::detect()`, `ram_estimate`

### Cosa fare per riprendere
1. Leggere questo file
2. Leggere `inferium-desktop/tui/src/main.rs` per il punto di partenza
3. Implementare il prossimo step non completato nella tabella sopra

### File da creare/modificare
```
inferium-desktop/tui/src/
├── main.rs              # Già esiste — aggiungere check setup_completed()
├── client.rs            # Già esiste — aggiornare ensure_engine_running
├── setup/
│   ├── mod.rs           # NUOVO — orchestrazione setup flow
│   ├── ui.rs            # NUOVO — rendering ratatui
│   └── steps.rs         # NUOVO — logica (detect, download, agent)
└── chat/
    ├── mod.rs           # NUOVO — chat loop
    ├── ui.rs            # NUOVO — layout ratatui
    ├── input.rs         # NUOVO — gestione input + comandi
    └── streaming.rs     # NUOVO — client SSE
```

### Dipendenze da aggiungere al Cargo.toml
```toml
inferium-pipeline = { path = "../../crates/inferium-pipeline" }
futures-util = "0.3"
```
