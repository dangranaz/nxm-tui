---
id: vendor-jcode-001
title: "Vendor — jcode: Rust TUI AI Coding Agent"
status: completed
created: 2026-08-27
updated: 2026-08-27T17:00:00Z
linked_to:
  - nxm-tui
used_for:
  - tui-patterns
  - rust-agent-reference
  - streaming-architecture
links:
  - https://github.com/1jehuang/jcode
repository: https://github.com/1jehuang/jcode
---

# jcode — Vendor Document

## 1. Identity

| Field | Value |
|-------|-------|
| **Vendor** | 1jehuang |
| **Role** | AI coding agent TUI/CLI in Rust |
| **License** | MIT |
| **Stars** | 18.7k★ |
| **Language** | Rust (Cargo workspace, multi-crate) + TypeScript SDK |
| **Commits** | 7000+ |
| **Crates** | `handterm` (custom terminal renderer), `mermaid-rs-renderer` |

## 2. What It Provides

| Component | Description |
|-----------|-------------|
| **TUI agent** | Terminal UI per interazione con LLM, ~28 MB RAM, ~14 ms startup |
| **Multi-provider** | Claude, OpenAI, Gemini, Copilot, Ollama, vLLM — routing trasparente |
| **Swarm mode** | Multi-agent collaboration per task complessi |
| **Semantic memory** | Memoria persistente tra sessioni |
| **MCP support** | Client MCP integrato |
| **Browser automation** | Automazione browser da terminale |
| **Self-dev** | Può modificare il proprio codice |

## 3. Relevance for Nexum

| Aspect | Score | Note |
|--------|:-----:|------|
| TUI patterns per nxm-tui | ⭐⭐⭐⭐ | Custom terminal renderer in Rust — reference per nxm-tui (ratatui-based) |
| Streaming architecture | ⭐⭐⭐ | Pattern async per SSE streaming multi-provider |
| Multi-provider routing | ⭐⭐⭐ | Pattern per nxm-dispatch se si vuole routing verso provider esterni |
| Code reusability | ⭐⭐ | Architettura diversa, ma pattern estraibili |
| Priority | 💤 | Reference — non urgente, consultare quando si lavora su nxm-tui |

## 4. Pattern Interessanti

### 4.1 RAM Efficiency
28 MB vs centinaia per competitor (Cursor, VS Code agents). Il custom terminal renderer (`handterm`) evita l'overhead di framework generici. **Lezione per nxm-tui:** ratatui è già leggero, ma il pattern "custom renderer per il caso specifico" è valido.

### 4.2 Provider-Agnostic Routing
Interfaccia uniforme verso 10+ LLM provider. Simile a quello che `nxm-dispatch` fa per engine locali, ma per API remote. Pattern utile se Nexum deve mai parlare con provider cloud come fallback.

### 4.3 Swarm Multi-Agent
Orchestrazione di agenti paralleli per task complessi. Può ispirare pattern per coordinamento tra engine (es. prefill su engine-mlx, decode su engine-metal).

## 5. Cosa NON Copiare

- **Non fa inferenza locale** — è un client, non un engine
- **TypeScript SDK** — non rilevante per il nostro stack
- **Browser automation** — fuori scope per Nexum

## 6. Quando Consultare

- Durante sviluppo di `nxm-tui` — per pattern terminal rendering in Rust
- Se si implementa streaming SSE nel client — per pattern async
- Se `nxm-dispatch` deve supportare provider cloud — per routing pattern
