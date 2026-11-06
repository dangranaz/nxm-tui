//! [`McpClient`] — the client half of an MCP session over stdio.
//!
//! Responsibilities:
//! 1. **Lifecycle**: send `initialize` declaring the `sampling` capability,
//!    then the `notifications/initialized` notification.
//! 2. **Client→server calls**: send a request, await the matching response by
//!    id (e.g. `tools/list`, `tools/call` — used by callers, not needed for
//!    sampling itself).
//! 3. **Server→client dispatch**: the run loop reads every inbound line; a
//!    `sampling/createMessage` request is routed to the [`SamplingHandler`] and
//!    its result written back as a JSON-RPC response. This is what lets a
//!    server tool (e.g. `generate_handoff`) obtain an LLM completion.
//!
//! The design keeps a single owner of the transport (the run loop). Callers
//! enqueue outbound requests over a channel and await a `oneshot` reply, so no
//! lock is shared across `.await` points.

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::mcp::protocol::{
    CreateMessageParams, Notification, Request, RequestId, Response, RpcError, JSONRPC_VERSION,
    MCP_PROTOCOL_VERSION,
};
use crate::mcp::sampling::SamplingHandler;
use crate::mcp::transport::StdioTransport;

/// Tracing target per `RULES.md`.
const TARGET: &str = "nexum::mcp::client";

/// An outbound client→server call awaiting its response.
struct Pending {
    reply: oneshot::Sender<Result<Value, RpcError>>,
}

/// A request the caller wants sent, with a channel for its eventual response.
pub struct OutboundCall {
    pub method: String,
    pub params: Option<Value>,
    pub reply: oneshot::Sender<Result<Value, RpcError>>,
}

/// Handle used by the rest of the app to issue calls into a running session.
#[derive(Clone)]
pub struct McpHandle {
    tx: mpsc::UnboundedSender<OutboundCall>,
}

impl McpHandle {
    /// Issue a client→server request and await its result value.
    pub async fn call(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(OutboundCall {
                method: method.to_string(),
                params,
                reply,
            })
            .map_err(|_| anyhow::anyhow!("mcp run loop is gone"))?;
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => anyhow::bail!("mcp error {}: {}", e.code, e.message),
            Err(_) => anyhow::bail!("mcp response dropped"),
        }
    }
}

/// The MCP client session. Owns the transport and the sampling handler.
pub struct McpClient {
    transport: StdioTransport,
    handler: Box<dyn SamplingHandler>,
    next_id: i64,
    pending: HashMap<i64, Pending>,
}

impl McpClient {
    /// Create a client over an already-spawned transport with a sampling handler.
    pub fn new(transport: StdioTransport, handler: Box<dyn SamplingHandler>) -> Self {
        Self {
            transport,
            handler,
            next_id: 1,
            pending: HashMap::new(),
        }
    }

    /// Perform the `initialize` handshake, declaring the `sampling` capability,
    /// then send `notifications/initialized`. Returns the server's info value.
    pub async fn initialize(&mut self) -> Result<Value> {
        info!(target: TARGET, "ENTER initialize");
        let params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": { "sampling": {} },
            "clientInfo": { "name": "nxm-tui", "version": env!("CARGO_PKG_VERSION") },
        });
        let id = self.alloc_id();
        let req = Request::new(id, "initialize", Some(params));
        self.write_json(&serde_json::to_string(&req)?).await?;

        // Read until we get the response to our initialize id, servicing any
        // inbound requests that arrive in the meantime.
        let result = self.await_response(id).await?;

        let note = Notification::new("notifications/initialized", None);
        self.write_json(&serde_json::to_string(&note)?).await?;
        info!(target: TARGET, "EXIT initialize (ok)");
        Ok(result)
    }

    /// Run the session loop: serve inbound server→client requests (sampling)
    /// and dispatch caller-issued outbound requests, until the server closes
    /// or `outbound` is dropped.
    pub async fn run(mut self, mut outbound: mpsc::UnboundedReceiver<OutboundCall>) {
        info!(target: TARGET, "ENTER run loop");
        loop {
            tokio::select! {
                line = self.transport.next_line() => {
                    match line {
                        Some(l) => {
                            if let Err(e) = self.on_line(&l).await {
                                warn!(target: TARGET, error = %e, "failed handling inbound line");
                            }
                        }
                        None => {
                            debug!(target: TARGET, "server closed; ending run loop");
                            break;
                        }
                    }
                }
                call = outbound.recv() => {
                    match call {
                        Some(c) => {
                            if let Err(e) = self.dispatch_outbound(c).await {
                                warn!(target: TARGET, error = %e, "failed dispatching outbound call");
                            }
                        }
                        None => {
                            debug!(target: TARGET, "outbound channel closed; ending run loop");
                            break;
                        }
                    }
                }
            }
        }
        self.transport.shutdown().await;
        info!(target: TARGET, "EXIT run loop");
    }

    /// Send a caller's outbound request, registering its pending reply.
    async fn dispatch_outbound(&mut self, call: OutboundCall) -> Result<()> {
        let id = self.alloc_id();
        self.pending.insert(id, Pending { reply: call.reply });
        let req = Request::new(id, call.method, call.params);
        self.write_json(&serde_json::to_string(&req)?).await
    }

    /// Classify one inbound line: response to a pending call, or an inbound
    /// request/notification from the server.
    async fn on_line(&mut self, line: &str) -> Result<()> {
        let value: Value = serde_json::from_str(line).context("inbound line not JSON")?;

        // A response carries `id` + (`result` | `error`) and no `method`.
        let is_response = value.get("method").is_none()
            && (value.get("result").is_some() || value.get("error").is_some());
        if is_response {
            self.complete_pending(&value);
            return Ok(());
        }

        // Otherwise it is a request (has `method`); server→client requests we
        // understand: `sampling/createMessage`. Others get a method-not-found.
        let method = value.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = value.get("id").cloned();
        match method {
            "sampling/createMessage" => self.handle_sampling(value).await,
            other => {
                if let Some(id) = id {
                    let rid: RequestId = serde_json::from_value(id)?;
                    let err = RpcError::new(-32601, format!("method not found: {other}"));
                    self.write_json(&serde_json::to_string(&Response::err(rid, err))?)
                        .await?;
                }
                Ok(())
            }
        }
    }

    /// Route a `sampling/createMessage` request to the handler and reply.
    async fn handle_sampling(&mut self, value: Value) -> Result<()> {
        let rid: RequestId = serde_json::from_value(
            value
                .get("id")
                .cloned()
                .context("sampling request missing id")?,
        )?;
        let params: CreateMessageParams = serde_json::from_value(
            value
                .get("params")
                .cloned()
                .context("sampling request missing params")?,
        )
        .context("invalid sampling params")?;

        let response = match self.handler.handle(params).await {
            Ok(result) => Response::ok(rid, serde_json::to_value(result)?),
            Err(e) => Response::err(rid, RpcError::new(-1, e.to_string())),
        };
        self.write_json(&serde_json::to_string(&response)?).await
    }

    /// Match a response value to its pending caller and deliver it.
    fn complete_pending(&mut self, value: &Value) {
        let id = match value.get("id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => return,
        };
        if let Some(p) = self.pending.remove(&id) {
            if let Some(err) = value.get("error") {
                if let Ok(e) = serde_json::from_value::<RpcError>(err.clone()) {
                    let _ = p.reply.send(Err(e));
                    return;
                }
            }
            let result = value.get("result").cloned().unwrap_or(Value::Null);
            let _ = p.reply.send(Ok(result));
        }
    }

    /// Blocking-ish read until the response for `id` arrives, servicing inbound
    /// requests in the meantime. Used only during `initialize`.
    async fn await_response(&mut self, id: i64) -> Result<Value> {
        loop {
            let line = self
                .transport
                .next_line()
                .await
                .context("server closed during initialize")?;
            let value: Value = serde_json::from_str(&line).context("inbound line not JSON")?;
            let is_response = value.get("method").is_none()
                && (value.get("result").is_some() || value.get("error").is_some());
            if is_response && value.get("id").and_then(|v| v.as_i64()) == Some(id) {
                if let Some(err) = value.get("error") {
                    let e: RpcError = serde_json::from_value(err.clone())?;
                    anyhow::bail!("initialize error {}: {}", e.code, e.message);
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            // Not our response: could be an inbound request; service it.
            if value.get("method").is_some() {
                self.on_line(&line).await?;
            }
        }
    }

    /// Write a pre-serialized JSON message through the transport, framed.
    async fn write_json(&mut self, json: &str) -> Result<()> {
        debug_assert!(json.starts_with('{'));
        let _ = JSONRPC_VERSION; // documents the framing version in scope
        self.transport.send_line(json).await
    }

    /// Allocate the next monotonic request id.
    fn alloc_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// Spawn the run loop on the tokio runtime, returning a clonable handle.
pub fn spawn_session(client: McpClient) -> McpHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(client.run(rx));
    McpHandle { tx }
}
