//! Agentic streaming loop for `nexum-tui` (P2 spike).
//!
//! Today `connection::chat_stream` only forwards text deltas. This module is a
//! *spike* that proves the agentic loop compiles against the real `nxm-tui`
//! types and that the OpenAI tool-call delta parser is correct:
//! stream → parse `tool_calls` → stub-dispatch → append result → re-query →
//! repeat until the model stops. It is **not yet wired** into the ratatui event
//! loop (that is P2-proper); `#[allow(dead_code)]` keeps the build clean.
//!
//! Types used: `app::{App, Message, Role}`, `tool_types::{ToolPart,
//! ToolInvocation, ToolResult}`, the `mpsc::UnboundedSender<ToolPart>` the loop
//! will emit on, and `reqwest::Client` (already a dep).

#![allow(dead_code)] // wired into the event loop in P2-proper

use std::collections::HashMap;

use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::{App, Message, Role};
use crate::tool_types::{ToolInvocation, ToolPart, ToolResult};

/// A minimal streaming OpenAI-compatible agent.
pub struct Agent<'a> {
    client: &'a Client,
    base_url: &'a str,
    model: &'a str,
}

impl<'a> Agent<'a> {
    pub fn new(client: &'a Client, base_url: &'a str, model: &'a str) -> Self {
        Self { client, base_url, model }
    }

    /// Run an agentic turn: stream a response, resolve any tool calls, and keep
    /// going until the model emits no more tool calls. Sends `ToolPart`s over
    /// `tx` so the UI can render incrementally. Mutates `app.messages` with the
    /// transcript (assistant tool-call turns + stub tool results).
    pub async fn run(
        &self,
        app: &mut App,
        tx: &mpsc::UnboundedSender<ToolPart>,
    ) -> Result<(), String> {
        loop {
            let calls = self.stream_turn(app, tx).await?;
            if calls.is_empty() {
                return Ok(());
            }
            // Spike: every tool call resolves to a stub result. P2-proper
            // dispatches to real tools and maps results back to the server as
            // `role: "tool"` messages (nxm-tui's `Role` enum lacks a `Tool`
            // variant — that wiring is deferred).
            self.dispatch_tool_calls(app, tx, calls).await?;
        }
    }

    /// Stream one assistant turn; return any tool invocations emitted.
    async fn stream_turn(
        &self,
        app: &mut App,
        tx: &mpsc::UnboundedSender<ToolPart>,
    ) -> Result<Vec<ToolInvocation>, String> {
        let api_messages = serialize_messages(&app.messages);
        let body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
            "stream": true,
            "tools": tools_spec(),
        });

        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("server returned {}", resp.status()));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut pending: HashMap<usize, ToolInvocation> = HashMap::new();
        let mut finish_reason: Option<String> = None;
        let mut done = false;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(nl) = buf.find('\n') {
                let raw = buf[..nl].trim().to_string();
                buf = buf[nl + 1..].to_string();
                if raw.is_empty() || raw.starts_with(": keep-alive") {
                    continue;
                }
                let json_str = raw.strip_prefix("data: ").unwrap_or(&raw);
                if json_str == "[DONE]" {
                    done = true;
                    break;
                }
                let json: Value = match serde_json::from_str(json_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(reason) = json
                    .pointer("/choices/0/finish_reason")
                    .and_then(|v| v.as_str())
                {
                    finish_reason = Some(reason.to_string());
                    if reason == "stop" {
                        done = true;
                    }
                }
                if let Some(content) = json
                    .pointer("/choices/0/delta/content")
                    .and_then(|v| v.as_str())
                {
                    if !content.is_empty() {
                        let _ = tx.send(ToolPart::Text(content.to_string()));
                        app.push_token(content);
                    }
                }
                if let Some(arr) = json.pointer("/choices/0/delta/tool_calls").and_then(|v| v.as_array()) {
                    for tc in arr {
                        if let Some(idx) = apply_tool_call_delta(
                            pending.entry(0).or_insert_with(|| ToolInvocation {
                                id: String::new(),
                                name: String::new(),
                                args: String::new(),
                            }),
                            tc,
                        ) {
                            // re-borrow the freshly-merged invocation is in `pending`
                            let _ = idx;
                        }
                    }
                }
            }
            if done {
                break;
            }
        }

        let has_tool_calls = finish_reason.as_deref() == Some("tool_calls") || !pending.is_empty();
        if !has_tool_calls {
            return Ok(Vec::new());
        }
        let invocations: Vec<ToolInvocation> =
            pending.into_iter().map(|(_, v)| v).collect();
        for inv in &invocations {
            let _ = tx.send(ToolPart::ToolInvocation(inv.clone()));
        }
        let parts: Vec<ToolPart> = invocations
            .iter()
            .map(|i| ToolPart::ToolInvocation(i.clone()))
            .collect();
        app.messages
            .push(Message::with_tool(Role::Assistant, parts));
        Ok(invocations)
    }

    /// Spike: every tool call → a stub `ToolResult`. P2-proper runs the real tool.
    async fn dispatch_tool_calls(
        &self,
        app: &mut App,
        tx: &mpsc::UnboundedSender<ToolPart>,
        calls: Vec<ToolInvocation>,
    ) -> Result<(), String> {
        for inv in calls {
            let result = ToolResult {
                call_id: inv.id.clone(),
                output: format!("stub result for tool `{}` (args: {})", inv.name, inv.args),
            };
            let _ = tx.send(ToolPart::ToolResult(result.clone()));
            app.messages
                .push(Message::with_tool(Role::Assistant, vec![ToolPart::ToolResult(result)]));
        }
        Ok(())
    }
}

/// One OpenAI tool_call delta → merge into an in-progress `ToolInvocation`.
/// Returns the delta's `index` so the caller can key its pending map.
fn apply_tool_call_delta(inv: &mut ToolInvocation, tc: &Value) -> Option<usize> {
    let idx = tc.get("index").and_then(|v| v.as_u64()).map(|u| u as usize)?;
    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
        inv.id = id.to_string();
    }
    if let Some(f) = tc.get("function") {
        if let Some(name) = f.get("name").and_then(|v| v.as_str()) {
            inv.name = name.to_string();
        }
        if let Some(args) = f.get("arguments").and_then(|v| v.as_str()) {
            inv.args.push_str(args);
        }
    }
    Some(idx)
}

/// Serialize the transcript for the OpenAI `/chat/completions` request.
/// Spike note: tool results carry no `role: "tool"` (nxm-tui `Role` lacks it);
/// the stub records them as assistant `tool_parts` — full mapping is P2.
fn serialize_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };
            serde_json::json!({ "role": role, "content": &m.content })
        })
        .collect()
}

/// Stub tool the model may emit (P2-proper registers the real set).
fn tools_spec() -> Value {
    serde_json::json!([{
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a file (stub).",
            "parameters": {
                "type": "object",
                "properties": { "path": { "type": "string" } }
            }
        }
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_first_tool_call_delta() {
        let mut inv = ToolInvocation {
            id: String::new(),
            name: String::new(),
            args: String::new(),
        };
        let delta = serde_json::json!({
            "index": 0,
            "id": "call_abc",
            "function": { "name": "read_file", "arguments": "{\"path\":\"a" }
        });
        let idx = apply_tool_call_delta(&mut inv, &delta);
        assert_eq!(idx, Some(0));
        assert_eq!(inv.id, "call_abc");
        assert_eq!(inv.name, "read_file");
        assert_eq!(inv.args, "{\"path\":\"a");
    }

    #[test]
    fn appends_argument_fragments_across_deltas() {
        let mut inv = ToolInvocation { id: "call_1".into(), name: String::new(), args: "{\"p".into() };
        let delta = serde_json::json!({ "index": 0, "function": { "arguments": "ath\":\"b\"}" } });
        apply_tool_call_delta(&mut inv, &delta);
        assert_eq!(inv.args, "{\"path\":\"b\"}");
        assert!(inv.name.is_empty()); // name arrived in an earlier (missing) delta
    }

    #[test]
    fn ignores_non_tool_delta() {
        let mut inv = ToolInvocation { id: String::new(), name: String::new(), args: String::new() };
        let delta = serde_json::json!({ "index": 0 });
        let idx = apply_tool_call_delta(&mut inv, &delta);
        assert_eq!(idx, Some(0));
        assert!(inv.id.is_empty() && inv.args.is_empty());
    }
}
