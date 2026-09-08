//! Agentic streaming loop for `nexum-tui` (P2-proper).
//!
//! Replaces the single-turn `connection::chat_stream` path: [`Agent::run`]
//! streams a response, resolves any `tool_calls` through `nexum-tools`,
//! appends the `role:"tool"` results to its transcript, and re-queries until
//! the model emits `finish_reason: "stop"`.
//!
//! Concurrency model: the `Agent` lives inside a `tokio::spawn` task and owns
//! its own transcript (`Vec<Message>`, cloned from `app.messages` at spawn).
//! It never borrows `&mut App`; it emits [`ToolPart`]s over `tx` and the main
//! ratatui loop mirrors them into `app.messages` (see `main.rs`). Text deltas
//! are absorbed into the agent transcript with [`app::apply_token`] (the same
//! helper the main loop uses), so `<thinking>`-tag filtering stays consistent.
//!
//! Rule: no `unwrap`/`expect` in prod — fallible parsing maps to `String` errors.

use std::collections::HashMap;

use futures::StreamExt;
use nxm_tools::{default_registry, Registry};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::{apply_token, Message, Role};
use crate::tool_types::{ToolInvocation, ToolPart, ToolResult};

/// A streaming OpenAI-compatible agent backed by `nexum-tools`.
pub struct Agent<'a> {
    client: &'a Client,
    base_url: &'a str,
    model: &'a str,
    api_key: Option<String>,
    registry: Registry,
    /// Growing transcript. Cloned from `app.messages` at spawn and evolved
    /// across turns (assistant tool-call messages + their `role:"tool"` results)
    /// so each request serializes the full conversation.
    transcript: Vec<Message>,
    /// Local mirrors of `App.{in_thinking, thinking_content}`. Only fed to
    /// `apply_token` (which writes them) so the transcript is `<thinking>`-clean;
    /// the *live* reasoning display is driven by `App` via the main loop.
    #[allow(dead_code)]
    in_thinking: bool,
    #[allow(dead_code)]
    thinking_content: String,
}

impl<'a> Agent<'a> {
    pub fn new(
        client: &'a Client,
        base_url: &'a str,
        model: &'a str,
        api_key: Option<String>,
        initial_messages: Vec<Message>,
    ) -> Self {
        Self {
            client,
            base_url,
            model,
            api_key,
            registry: default_registry(),
            transcript: initial_messages,
            in_thinking: false,
            thinking_content: String::new(),
        }
    }

    /// Run a full agentic turn: stream → resolve tool calls via `nexum-tools` →
    /// append `role:"tool"` results → re-query; loops until the model stops.
    /// Emits `ToolPart`s for live UI rendering.
    pub async fn run(&mut self, tx: &mpsc::UnboundedSender<ToolPart>) -> Result<(), String> {
        loop {
            let calls = self.stream_turn(tx).await?;
            if calls.is_empty() {
                return Ok(());
            }
            self.dispatch_tool_calls(tx, calls).await?;
        }
    }

    /// Stream one assistant turn; return tool invocations emitted by the model.
    async fn stream_turn(
        &mut self,
        tx: &mpsc::UnboundedSender<ToolPart>,
    ) -> Result<Vec<ToolInvocation>, String> {
        let api_messages = serialize_messages(&self.transcript);
        let body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
            "stream": true,
            "tools": self.registry.manifest(),
        });

        let mut req = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("authorization", format!("Bearer {}", key));
        }
        let resp = req
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
                        // Absorb into the transcript (strips <thinking>), then
                        // forward the raw delta to the main loop for live render.
                        apply_token(
                            &mut self.transcript,
                            &mut self.in_thinking,
                            &mut self.thinking_content,
                            content,
                        );
                        let _ = tx.send(ToolPart::Text(content.to_string()));
                    }
                }
                // `reasoning_content` delta (o1 / QwQ / NVIDIA-Thinking /
                // nemotron): forward raw to the UI thinking buffer so live
                // reasoning renders without polluting the transcript.
                if let Some(thinking) = json
                    .pointer("/choices/0/delta/reasoning_content")
                    .and_then(|v| v.as_str())
                {
                    if !thinking.is_empty() {
                        let _ = tx.send(ToolPart::Reasoning(thinking.to_string()));
                    }
                }
                if let Some(arr) =
                    json.pointer("/choices/0/delta/tool_calls").and_then(|v| v.as_array())
                {
                    for tc in arr {
                        let entry = pending
                            .entry(
                                tc.get("index")
                                    .and_then(|v| v.as_u64())
                                    .map(|u| u as usize)
                                    .unwrap_or(0),
                            )
                            .or_insert_with(|| ToolInvocation {
                                id: String::new(),
                                name: String::new(),
                                args: String::new(),
                            });
                        let _ = apply_tool_call_delta(entry, tc);
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
        let invocations: Vec<ToolInvocation> = pending.into_iter().map(|(_, v)| v).collect();
        let parts: Vec<ToolPart> = invocations
            .iter()
            .map(|i| ToolPart::ToolInvocation(i.clone()))
            .collect();
        for inv in &invocations {
            let _ = tx.send(ToolPart::ToolInvocation(inv.clone()));
        }
        self.transcript
            .push(Message::with_tool(Role::Assistant, parts));
        Ok(invocations)
    }

    /// Resolve each tool call through `nexum-tools` and record the results as
    /// `role:"tool"` transcript messages + UI `ToolResult` parts.
    async fn dispatch_tool_calls(
        &mut self,
        tx: &mpsc::UnboundedSender<ToolPart>,
        calls: Vec<ToolInvocation>,
    ) -> Result<(), String> {
        for inv in calls {
            let args: Value = serde_json::from_str(&inv.args).unwrap_or_else(|e| {
                serde_json::json!({ "_raw": &inv.args, "_parse_error": e.to_string() })
            });
            let output = match self.registry.call(&inv.name, &args) {
                Ok(r) => r.content,
                Err(e) => format!("tool dispatch error: {e}"),
            };
            let tpart = ToolResult {
                call_id: inv.id.clone(),
                output: output.clone(),
            };
            let _ = tx.send(ToolPart::ToolResult(tpart));
            self.transcript.push(Message::tool(&inv.id, output));
        }
        Ok(())
    }
}

/// Merge one OpenAI `tool_call` delta fragment into an in-progress invocation.
/// Returns the delta's `index` (0 when absent).
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

/// Serialize the transcript for the OpenAI request, emitting `tool_calls` on
/// assistant messages that carry invocations and `role:"tool"` for results.
fn serialize_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| match m.role {
            Role::User => serde_json::json!({ "role": "user", "content": &m.content }),
            Role::System => serde_json::json!({ "role": "system", "content": &m.content }),
            Role::Assistant => {
                let invs: Vec<&ToolInvocation> = m
                    .tool_parts
                    .iter()
                    .filter_map(|p| match p {
                        ToolPart::ToolInvocation(i) => Some(i),
                        _ => None,
                    })
                    .collect();
                if invs.is_empty() {
                    serde_json::json!({ "role": "assistant", "content": &m.content })
                } else {
                    serde_json::json!({
                        "role": "assistant",
                        "content": &m.content,
                        "tool_calls": invs.iter().map(|i| serde_json::json!({
                            "id": &i.id,
                            "type": "function",
                            "function": { "name": &i.name, "arguments": &i.args }
                        })).collect::<Vec<_>>()
                    })
                }
            }
            Role::Tool => serde_json::json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id,
                "content": &m.content,
            }),
        })
        .collect()
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
        let mut inv = ToolInvocation {
            id: "call_1".into(),
            name: String::new(),
            args: "{\"p".into(),
        };
        let delta = serde_json::json!({ "index": 0, "function": { "arguments": "ath\":\"b\"}" } });
        apply_tool_call_delta(&mut inv, &delta);
        assert_eq!(inv.args, "{\"path\":\"b\"}");
        assert!(inv.name.is_empty()); // name arrived in an earlier (missing) delta
    }

    #[test]
    fn ignores_non_tool_delta() {
        let mut inv = ToolInvocation {
            id: String::new(),
            name: String::new(),
            args: String::new(),
        };
        let delta = serde_json::json!({ "index": 0 });
        let idx = apply_tool_call_delta(&mut inv, &delta);
        assert_eq!(idx, Some(0));
        assert!(inv.id.is_empty() && inv.args.is_empty());
    }
}
