---
id: design-acp-001
title: "ACP — Agent Client Protocol Implementation for nexum"
status: planned
created: 2026-06-25
updated: 2026-06-25T20:19:00Z
linked_to:
  - inferentia-v2-brainstorm
used_for:
  - nexum-local-srv
  - nexum-terminal
  - agent
  - integration
links:
  - https://agentclientprotocol.com/get-started/introduction
  - https://agentclientprotocol.com/get-started/architecture
  - https://agentclientprotocol.com/libraries/rust
  - https://github.com/agentclientprotocol/rust-sdk
  - https://crates.io/crates/agent-client-protocol
repository: https://github.com/agentclientprotocol/agent-client-protocol
---

# ACP — Agent Client Protocol per nexum

## Cos'è ACP

ACP (Agent Client Protocol) è lo standard per comunicazione tra **editor/IDE** e **coding agent**.
Creato da Zed, adottato da: GitHub Copilot, Claude Agent, Gemini CLI, Cursor, Hermes, Kiro, OpenHands, Goose, Cline, e altri 30+ agent.

È l'**LSP degli agent** — un protocollo unico per parlare con qualsiasi editor.

## Perché implementarlo in nexum

| Beneficio | Impatto |
|-----------|---------|
| **Nexum diventa usabile da Zed** | Zed è il primo client ACP — supporto nativo |
| **Qualsiasi IDE supportato** | JetBrains, VS Code (via adapter), Cursor, ecc. |
| **Ecosistema MCP incluso** | L'editor passa i suoi MCP server all'agent |
| **Multi-sessione** | Un processo nexum, più conversazioni parallele |
| **Standard industriale** | Non reinventiamo il wire protocol |

## Architettura ACP

```
┌─────────────────┐     JSON-RPC/stdio      ┌──────────────────┐
│   Editor/IDE    │ ◄──────────────────────► │   nexum (Agent)  │
│   (ACP Client)  │                          │   (ACP Server)   │
│                 │                          │                  │
│ • File system   │   ← tool calls →        │ • Inference      │
│ • MCP servers   │   ← streaming →         │ • Indexer        │
│ • User input    │   ← diff display →      │ • R-SWA context  │
│ • Permissions   │                          │ • Memory         │
└─────────────────┘                          └──────────────────┘
```

### Flusso di comunicazione

1. **Editor avvia nexum** come subprocess
2. **Handshake**: capabilities exchange (JSON-RPC)
3. **Sessioni**: l'editor apre N sessioni (conversazioni parallele)
4. **Prompt**: editor invia messaggio utente + contesto file + MCP config
5. **Agent**: nexum fa inference, streama risposta via notifications
6. **Tool calls**: nexum chiede permesso all'editor per azioni (edit file, run command)
7. **Diffs**: nexum invia diff strutturati che l'editor può renderizzare

### Trasporti

| Tipo | Protocollo | Uso |
|------|-----------|-----|
| **Locale** | JSON-RPC over stdio | Editor avvia nexum come processo |
| **Remoto** | HTTP/WebSocket (futuro) | Agent hosted su server |

## Implementazione in nexum

### Crate: `nexum-acp`

```
features/nexum-acp/
├── Cargo.toml
├── src/
│   ├── lib.rs          — re-exports
│   ├── agent.rs        — impl Agent trait (core logic)
│   ├── session.rs      — session management (multi-conversation)
│   ├── tools.rs        — tool definitions (file edit, shell, search)
│   └── transport.rs    — stdio JSON-RPC transport
└── tests/
    └── integration.rs
```

### Dependency

```toml
[dependencies]
agent-client-protocol = "0.1"  # Official Rust SDK (crates.io)
```

Il crate ufficiale fornisce:
- `Agent` trait da implementare
- `Client` trait (per testing)
- Tutti i tipi JSON-RPC (Request, Response, Notification)
- Transport layer (stdio)

### Agent trait — cosa implementare

```rust
use agent_client_protocol::{Agent, Session, Message, ToolCall, ToolResult};

pub struct NexumAgent {
    indexer: IndexStore,
    embedder: Box<dyn Embedder>,
    // Connection to nexum-local-srv for inference
    inference_url: String,
}

impl Agent for NexumAgent {
    /// Handle new session creation
    async fn create_session(&mut self, params: CreateSessionParams) -> Session;

    /// Handle incoming message from user
    async fn handle_message(&mut self, session_id: &str, message: Message) -> Stream<AgentEvent>;

    /// Execute a tool call (after editor grants permission)
    async fn execute_tool(&mut self, call: ToolCall) -> ToolResult;

    /// List available tools
    fn tools(&self) -> Vec<ToolDefinition>;
}
```

### Flusso dettagliato di una richiesta

```
1. Editor invia: { "method": "session/message", "params": { "text": "fix the bug in main.rs" } }
          │
          ▼
2. NexumAgent::handle_message()
   ├── indexer.hybrid_search("fix bug main.rs") → top-3 code chunks
   ├── build prompt con context (R-SWA mantiene reference)
   ├── POST http://localhost:11434/v1/chat/completions (streaming)
   └── stream tokens → editor via notifications
          │
          ▼
3. Se il modello vuole editare un file:
   ├── Agent invia: { "method": "tool/request", "params": { "name": "edit_file", "path": "main.rs", "diff": "..." } }
   ├── Editor chiede conferma all'utente
   ├── Editor risponde: { "result": { "approved": true } }
   └── Agent applica edit
          │
          ▼
4. Agent invia: { "method": "session/event", "params": { "type": "done" } }
```

### Tools da esporre

| Tool | Descrizione | Permesso richiesto |
|------|-------------|-------------------|
| `edit_file` | Applica diff a un file | Sì (l'editor mostra il diff) |
| `read_file` | Leggi contenuto file | No (già nel contesto editor) |
| `run_command` | Esegui comando shell | Sì |
| `search_code` | Cerca nel workspace via indexer | No |
| `list_files` | Lista file nel progetto | No |

### MCP Integration

L'editor passa la configurazione dei suoi MCP server:
```json
{
  "mcp_servers": [
    { "name": "github", "command": "mcp-github", "args": [] },
    { "name": "filesystem", "command": "mcp-fs", "args": ["/project"] }
  ]
}
```

Nexum si connette direttamente ai MCP server per usare i loro tools
(database, API, filesystem, ecc.) — aumentando le capacità dell'agent senza
reimplementare nulla.

## Integrazione con componenti esistenti

| Componente nexum | Ruolo in ACP |
|-----------------|--------------|
| `nexum-local-srv` | Backend inference (HTTP API) |
| `nexum-indexer` | Workspace search per contesto |
| R-SWA | Mantiene il contesto iniettato durante tutta la conversazione |
| `nexum-terminal` | **Diventa un ACP client** alternativo (non solo l'editor) |

## Piano di implementazione

| # | Task | Effort | Prerequisiti |
|---|------|--------|-------------|
| 1 | Add `agent-client-protocol` dep, scaffold `nexum-acp` crate | 1h | nessuno |
| 2 | Implement `Agent` trait con inference via HTTP a local-srv | 3h | server funzionante |
| 3 | Wire indexer per context injection nelle sessioni | 1h | indexer ✅ |
| 4 | Tool: `edit_file` (genera diff, chiede permesso) | 2h | 2 |
| 5 | Tool: `run_command` (shell con permesso) | 1h | 2 |
| 6 | Binary `nexum-agent` che lancia ACP over stdio | 1h | 1-5 |
| 7 | Test con Zed editor | 1h | 6 |
| 8 | Registrazione in ACP Registry | 30min | 7 |

**Stima totale: 1-2 giorni.**

## Risultato finale

```bash
# Zed settings.json (o qualsiasi ACP client)
{
  "agents": {
    "nexum": {
      "command": "nexum-agent",
      "args": ["--model", "qwen3-8b"]
    }
  }
}
```

L'utente apre Zed, scrive nel pannello agent → nexum risponde con contesto
del progetto, può editare file, eseguire comandi, cercare codice.
Tutto locale, 75 t/s, zero cloud.

## Valore strategico

1. **Distribuzione**: qualsiasi utente Zed/JetBrains/Cursor può usare nexum senza setup
2. **Differenziazione**: agent locale con context R-SWA + indexer = qualità migliore del generico
3. **Composabilità**: MCP tools dell'utente disponibili automaticamente
4. **Multi-model**: nexum può swap modelli in background (Adaptive Model Swap) — l'editor non se ne accorge
5. **Posizionamento**: nexum come alternativa locale a Claude Agent / Copilot / Cursor
