# DeepSeek TUI

| Campo | Valore |
|-------|--------|
| **Data analisi** | 2026-05-12 |
| **Versione progetto** | v0.8.31 |
| **Repo** | https://github.com/Hmbown/DeepSeek-TUI |
| **Licenza** | MIT |
| **Rilevanza** | 🔴🔴 CRITICA — Riferimento #1 per la nostra TUI |
| **Area Inferium** | tui > chat-ui, commands; agents > tool-use, sub-agents |
| **Stato** | Da studiare in profondità |

## Cosa fa

Coding agent TUI in **Rust** per DeepSeek V4. Streaming reasoning blocks, edit file con approval gates, auto mode (sceglie modello + thinking level per turno), sub-agents, MCP, skills system, session save/resume, workspace rollback, HTTP/SSE API, LSP diagnostics. 25.9k stars, 1157 commits.

## Stack tecnico

- **Rust 95%** (workspace con crates/)
- ratatui per TUI
- OpenAI-compatible streaming client
- Tool registry tipizzato (shell, file ops, git, web, sub-agents, MCP)
- Skills system (`.agents/skills/`, community install da GitHub)
- Session state, turn tracking, durable task queue
- LSP subsystem (rust-analyzer, pyright, typescript-language-server, gopls, clangd)
- HTTP/SSE runtime API (`deepseek serve --http`)
- User memory (persistent note file cross-session)
- Multi-provider: DeepSeek, NVIDIA NIM, Fireworks, Ollama, vLLM, SGLang

## Cosa è DIRETTAMENTE rilevante per noi

### 1. Architettura TUI Rust con ratatui (IDENTICA alla nostra)
- Dispatcher CLI → TUI binary → ratatui interface ↔ async engine ↔ OpenAI-compatible client
- **Questo è esattamente il nostro `inferium` → `inferium-engine`!**

### 2. Tre modalità (Plan / Agent / YOLO)
- Plan: read-only, esplora e propone
- Agent: interattivo con approval gates
- YOLO: auto-approve tutto
- Pattern da adottare per la nostra TUI

### 3. Skills system compatibile
- Scopre skills da `.agents/skills/`, `.claude/skills/`, `.deepseek/skills/`
- Frontmatter YAML + markdown body
- Install da GitHub senza backend
- **Compatibile con il nostro `.inferium/skills/`!**

### 4. Session save/resume + workspace rollback
- Checkpoint e resume sessioni lunghe
- Side-git snapshots pre/post turno con `/restore`
- Durable task queue che sopravvive ai restart

### 5. Multi-provider con OpenAI-compatible
- Supporta Ollama, vLLM, SGLang su localhost
- **Il nostro engine è un provider OpenAI-compatible → DeepSeek TUI potrebbe connettersi a Inferium!**

### 6. Auto mode (routing intelligente)
- Piccola chiamata di routing per decidere modello + thinking level
- Pattern utile se supportiamo più modelli locali

### 7. User memory
- File persistente iniettato nel system prompt
- Cross-session preferences
- Esattamente il nostro L2 (Global Facts) della memoria stratificata

### 8. LSP diagnostics post-edit
- Dopo ogni edit, LSP fornisce errori/warning al modello
- Pattern per agent che scrivono codice con feedback loop

## Pro/Contro

| Pro | Contro |
|-----|--------|
| **Rust + ratatui** = stesso stack nostro | Dipende da API cloud DeepSeek |
| MIT = possiamo studiare/forkare | 25.9k stars = community enorme, difficile competere |
| Skills compatibili con il nostro formato | Non fa inferenza locale (solo client) |
| Multi-provider incluso Ollama | Molto maturo (1157 commits) — difficile da forkare |
| Session management completo | |
| HTTP/SSE API per headless | |

## Come integrarlo con Inferium

**Opzione A — DeepSeek TUI come client di Inferium:**
```
deepseek --provider ollama --model qwen3-4b
# oppure
VLLM_BASE_URL="http://localhost:11435/v1" deepseek --provider vllm --model qwen3-4b
```
Il nostro engine espone API OpenAI-compatible → DeepSeek TUI si connette senza modifiche.

**Opzione B — Studiare e replicare i pattern nella nostra TUI:**
- Tre modalità (Plan/Agent/YOLO)
- Skills system
- Session save/resume
- Tool registry tipizzato
- LSP integration

**Opzione C — Fork per TUI locale-first:**
- Rimuovere dipendenza da API cloud
- Collegare direttamente a `inferium-runtime` in-process
- Mantenere tutta la UX (skills, sessions, modes)

## Decisione

**Da studiare in profondità** — Questo è IL riferimento per la nostra TUI. Stessa architettura (Rust + ratatui + OpenAI-compatible client), stessi pattern (skills, sessions, modes). L'Opzione A (usarlo come client) funziona già oggi. L'Opzione B (replicare i pattern) è il percorso per la nostra TUI custom. Leggere `docs/ARCHITECTURE.md` e il codice in `crates/`.
