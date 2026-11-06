# MCP Host Subsystem — Architecture & Refinement Roadmap

Status: **minimal working cut** (sampling host). Last verified: 2026-09-30.

nxm-tui acts as an MCP **client/host**. It spawns an MCP server (today
`nxm-session-mcp --transport stdio`) as its own child, completes the MCP
`initialize` handshake **declaring the `sampling` capability**, and serves
`sampling/createMessage` requests by forwarding them to the TUI's
OpenAI-compatible LLM backend. This is what lets a server-side tool such as
`generate_handoff` obtain completions and populate every section of its
output (Decisions / Files / Failed approaches / todo), instead of leaving them
blank because no client offered sampling.

## Why the TUI spawns the server (and does not "attach")

A server started with `--transport stdio` communicates over the stdin/stdout of
**its own process**. Whoever holds those pipes owns the conversation. A server
launched by an external script is bound to that script's pipes — nothing else
can attach to it after the fact, and there is no shared socket.

Consequence: "is the server present?" is a **launchability** check
(`transport::binary_present`: the binary resolves on `PATH` or as an absolute
path). When present, the host spawns its **own** child and owns the pipes for
the duration of the session. This is the same model every MCP host uses
(Kiro, Claude Desktop, etc.). If a truly shared/attachable server is ever
required, that needs a socket transport — see the roadmap below.

## Module layout (`src/mcp/`)

| File | Responsibility | Lines |
|------|----------------|-------|
| `protocol.rs` | JSON-RPC 2.0 envelopes + sampling wire types (serde, `camelCase`). Shapes follow MCP spec `2025-06-18`. | 244 |
| `transport.rs` | Spawn the server child, newline-delimited stdio framing, async read loop, `binary_present` probe. | 146 |
| `sampling.rs` | `SamplingHandler` seam + `LlmSamplingHandler` (HTTP bridge to `/v1/chat/completions`). Auto-approves. | 156 |
| `client.rs` | `McpClient`: `initialize`, request/response correlation by id, inbound `sampling/createMessage` dispatch, run loop. | 273 |
| `mod.rs` | `connect_if_present` high-level entry; re-exports. | 72 |

All files are under the 300-line `RULES.md` cap.

## Message flow

```
nxm-tui (host + client)                     nxm-session-mcp (server)
        │  spawn `--transport stdio`                 │
        │───────────── initialize ──────────────────▶│  capabilities.sampling = {}
        │◀──────────── result ───────────────────────│
        │────────── notifications/initialized ───────▶│
        │                                             │
        │           (a server tool needs an LLM)      │
        │◀────────── sampling/createMessage ──────────│  server→client REQUEST
        │  handler.handle(params):                    │
        │    params → OpenAI messages                 │
        │    POST /v1/chat/completions                │
        │    reply → CreateMessageResult              │
        │────────────── response ────────────────────▶│
```

The run loop (`McpClient::run`) is the single owner of the transport. It
`select!`s between inbound lines (server→client) and outbound calls enqueued by
the app via `McpHandle`. Inbound responses are matched to pending calls by
numeric id; inbound requests with method `sampling/createMessage` go to the
handler; any other inbound method gets a JSON-RPC `-32601 method not found`.

## Decisions (this cut)

- **Auto-approve sampling.** No human-in-the-loop overlay. The MCP spec says a
  client *SHOULD* let a human review; we consciously defer that (see roadmap).
  The seam for it already exists (`SamplingHandler` is the natural gate point).
- **No new dependencies.** JSON-RPC hand-rolled over `serde_json`; stdio over
  `tokio::process`; object-safe async via `futures::future::BoxFuture` instead
  of the `async-trait` crate.
- **Spawn, don't attach** (see rationale above).
- **Non-streaming completion** for sampling (simpler; `stream:false`).

## Configuration (`~/.nexum/tui.toml`)

```toml
[mcp]
enabled = true
command = "nxm-session-mcp"
args = ["--transport", "stdio"]
```

Defaults live in `config::McpConfig::default()`. When `enabled` is false or the
binary is absent, `connect_if_present` returns `Ok(None)` — a clean skip.

## What is intentionally NOT here (refinement roadmap)

1. **Human-in-the-loop approval.** Add an approval overlay (mirror
   `tool_overlay.rs` / `ApprovalRequest`) gating `SamplingHandler::handle`, with
   an "always allow for this server" memory. Preview + editable prompt per spec.
2. **`tools/list` + `tools/call`.** So the TUI can *invoke* server tools
   (e.g. call `generate_handoff` itself), not only serve sampling. `McpHandle`
   already supports outbound calls; add typed wrappers + UI.
3. **Streaming sampling** (`stream:true`) forwarded incrementally for live
   rendering, reusing `agent.rs`'s SSE parsing.
4. **Persistent / multiplexed sessions.** Today a session is short-lived per
   operation. A long-lived host holding several servers (roots, resources)
   would need lifecycle management + reconnection.
5. **Socket transport.** If an externally-launched, shared server must be
   reachable, add `--transport socket` on the server side and a Unix-socket
   transport here. `StdioTransport` is the interface to generalize.
6. **Wiring into the app runtime.** ✅ Done (2026-10-02): `connect_from_config`
   is called from `main.rs` at startup when the endpoint is `Running`; the
   `McpHandle` is kept alive for the loop. Open sub-item: a session connected
   via the NoServer menu *after* startup is not yet given an MCP session (the
   handle is computed once, pre-loop) — wire it on the `Connecting→Running`
   transition when needed. Status in the bottom bar is still TODO.
7. **Model preferences → model selection.** `ModelPreferences` (hints +
   priorities) is parsed but not yet used to pick a model; map hints to the
   configured provider's models.
8. **Image/audio content.** Currently projected to a text placeholder; a
   multimodal backend could carry them through.

## Tests

- `tests/mcp_protocol.rs` — serde round-trips locked to the spec request/result
  shapes; capability declaration.
- `tests/mcp_sampling.rs` — `SamplingHandler` contract with a fake handler
  (auto-approve, empty conversation, object-safety behind `Box`), plus a live
  `initialize` against the real binary (skips when absent).

Run: `cargo test --test mcp_protocol --test mcp_sampling`.
