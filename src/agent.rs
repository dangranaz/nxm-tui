//! Agentic streaming loop for `nexum-tui` (P2-proper).
//!
//! Replaces the single-turn `connection::chat_stream` path: [`Agent::run`]
//! streams a response, resolves any `tool_calls` through `nexum-tools`,
//! appends the `role:"tool"` results to its transcript, and re-queries until
//! the model emits `finish_reason: "stop"` — or until the hard iteration cap
//! (`bounded::BudgetTracker`) halts the loop with a `ToolPart::LoopHalted`.
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
use std::sync::atomic::Ordering;

use crate::plan_mode::PlanModeHandle;
use futures::StreamExt;
use nxm_tui_tools::{Registry, ToolKind};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::{apply_token, Message, Role};
use crate::bounded::{Budget, BudgetTracker};
use crate::circuit_breaker::{signature, Stall, StallDetector};
use crate::tool_types::{needs_approval, ApprovalRequest, ToolInvocation, ToolPart, ToolResult};

/// A streaming OpenAI-compatible agent backed by `nexum-tools`.
pub struct Agent<'a> {
    client: &'a Client,
    base_url: &'a str,
    model: &'a str,
    api_key: Option<String>,
    /// The tool registry the agent dispatches through. Built by `App` (which
    /// owns the T1 selection) and handed over as a snapshot at spawn time.
    registry: Registry,
    plan_mode: PlanModeHandle,
    /// Growing transcript. Cloned from `app.messages` at spawn and evolved
    /// across turns (assistant tool-call messages + their `role:"tool"` results)
    /// so each request serializes the full conversation.
    transcript: Vec<Message>,
    /// Channel back to the UI loop for tool approvals. `None` means headless
    /// (tests): the policy still applies but without a UI every gated tool is
    /// auto-approved so non-interactive runs never hang.
    approval_tx: Option<mpsc::UnboundedSender<ApprovalRequest>>,
    /// Local mirrors of `App.{in_thinking, thinking_content}`. Only fed to
    /// `apply_token` (which writes them) so the transcript is `<thinking>`-clean;
    /// the *live* reasoning display is driven by `App` via the main loop.
    #[allow(dead_code)]
    in_thinking: bool,
    #[allow(dead_code)]
    thinking_content: String,
    /// Hard cap on loop iterations (loopeng-01): when the budget is spent the
    /// loop hard-stops instead of spinning forever on a tool-calling model.
    budget: BudgetTracker,
    /// Stagnation breaker (loopeng-01-02): consecutive identical tool-call
    /// iterations before the breaker trips. Built into a `StallDetector` at
    /// the start of each `run()` invocation.
    stall_threshold: u32,
}

impl<'a> Agent<'a> {
    // Constructor mirrors the spawn-time snapshot (client, endpoint, key,
    // transcript, channels, registry, plan mode, budget); a builder would
    // add boilerplate without removing real coupling.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: &'a Client,
        base_url: &'a str,
        model: &'a str,
        api_key: Option<String>,
        initial_messages: Vec<Message>,
        approval_tx: Option<mpsc::UnboundedSender<ApprovalRequest>>,
        registry: Registry,
        plan_mode: PlanModeHandle,
        max_steps: u32,
        stall_threshold: u32,
    ) -> Self {
        Self {
            client,
            base_url,
            model,
            api_key,
            registry,
            transcript: initial_messages,
            plan_mode,
            approval_tx,
            in_thinking: false,
            thinking_content: String::new(),
            budget: BudgetTracker::new(max_steps),
            stall_threshold,
        }
    }

    /// Run a full agentic turn: stream → resolve tool calls via `nexum-tools` →
    /// append `role:"tool"` results → re-query; loops until the model stops
    /// calling tools, or until the hard iteration cap (`max_steps`) is hit —
    /// then it hard-stops, emits `ToolPart::LoopHalted` and returns `Ok(())`.
    /// Emits `ToolPart`s for live UI rendering.
    pub async fn run(&mut self, tx: &mpsc::UnboundedSender<ToolPart>) -> Result<(), String> {
        // Stagnation detector is built here (per spec) rather than at
        // construction, so each `run()` invocation starts with a fresh count.
        let mut stall = StallDetector::new(self.stall_threshold);
        loop {
            if let Budget::Halted { steps } = self.budget.tick() {
                let reason = format!("max iterations reached ({steps})");
                tracing::warn!(
                    target: "nexum::agent::budget",
                    steps,
                    "halting ReAct loop: iteration cap reached"
                );
                let _ = tx.send(ToolPart::LoopHalted { reason, steps });
                return Ok(());
            }
            let calls = self.stream_turn(tx).await?;
            if calls.is_empty() {
                return Ok(());
            }
            // Circuit breaker on stagnation (loopeng-01-02): the same
            // tool-call signature N consecutive iterations → hard stop before
            // the iteration budget burns out on a stuck loop.
            let sig = signature(
                &calls
                    .iter()
                    .map(|c| (c.name.clone(), c.args.clone()))
                    .collect::<Vec<_>>(),
            );
            match stall.observe(&sig) {
                Stall::Ok { .. } => {}
                Stall::Tripped { repeats, signature } => {
                    let reason =
                        format!("stalled: identical tool call repeated {repeats}× ({signature})");
                    tracing::warn!(
                        target: "nexum::agent::stall",
                        repeats,
                        signature = %signature,
                        steps = self.budget.steps(),
                        "halting ReAct loop: stagnation detected"
                    );
                    let _ = tx.send(ToolPart::LoopHalted {
                        reason,
                        steps: self.budget.steps(),
                    });
                    return Ok(());
                }
            }
            self.dispatch_tool_calls(tx, calls).await?;
        }
    }

    /// Build the JSON body for one streaming chat turn. Pure (no I/O) —
    /// extracted from [`run`](Agent::run) so tests can assert the `tools`
    /// payload without a server.
    ///
    /// `tools` is `registry.manifest_for(task)`: the automatic per-task
    /// selection (Core tools + keyword match) union the tools the user
    /// explicitly enabled in the overlay (MT-3). `task` is the content of
    /// the last [`Role::User`] message in the transcript; when there is
    /// none, `select` fails-safes to Core only.
    ///
    /// ```
    /// use nxm_tui::agent::Agent;
    /// use nxm_tui::app::{Message, Role};
    /// use nxm_tui_tools::default_registry;
    /// use reqwest::Client;
    ///
    /// let client = Client::new();
    /// let agent = Agent::new(&client, "http://localhost:1", "m", None, Vec::new(), None, default_registry(), nxm_tui::plan_mode::plan_mode_handle(), nxm_tui::bounded::DEFAULT_MAX_STEPS, nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD);
    /// let body = agent.request_body();
    /// // No user message → fail-safe: Core tools only.
    /// assert_eq!(body["tools"].as_array().map(|a| a.len()), Some(2));
    /// ```
    pub fn request_body(&self) -> Value {
        let task = self
            .transcript
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let mut tools = self.registry.manifest_for(task);

        // Plan mode: when active, mechanically abstain from write tools
        // (Edit, Web) so the Agent can only analyze and produce a task list
        // via `todo`. Filtering is done per entry by looking up each tool's
        // kind in the registry (name → kind), without changing the semantics
        // of `select`/`manifest_for` themselves.
        if self.plan_mode.load(Ordering::Relaxed) {
            tools = tools
                .into_iter()
                .filter(|entry| {
                    let name = entry
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    match self.registry.get(name) {
                        Some(tool) => !matches!(tool.kind(), ToolKind::Edit | ToolKind::Web),
                        None => true, // keep unknown entries (fail-safe)
                    }
                })
                .collect();
        }

        // Privacy: log the task length, never its content (the user message can
        // carry sensitive data). Content stays out of logs by design.
        tracing::info!(
            target: "nexum::agent::request",
            tool_count = tools.len(),
            task_len = task.len(),
            "request body built (per-task selection)"
        );
        serde_json::json!({
            "model": self.model,
            "messages": serialize_messages(&self.transcript),
            "stream": true,
            "tools": tools,
        })
    }

    /// Stream one assistant turn; return tool invocations emitted by the model.
    async fn stream_turn(
        &mut self,
        tx: &mpsc::UnboundedSender<ToolPart>,
    ) -> Result<Vec<ToolInvocation>, String> {
        let body = self.request_body();

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
                if let Some(arr) = json
                    .pointer("/choices/0/delta/tool_calls")
                    .and_then(|v| v.as_array())
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
        let invocations: Vec<ToolInvocation> = pending.into_values().collect();
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
    /// `role:"tool"` transcript messages + UI `ToolResult` parts. Gated tools
    /// wait for the UI approval overlay; a denial is recorded as a tool result
    /// so the model can react to it on the next turn.
    async fn dispatch_tool_calls(
        &mut self,
        tx: &mpsc::UnboundedSender<ToolPart>,
        calls: Vec<ToolInvocation>,
    ) -> Result<(), String> {
        for inv in calls {
            let args: Value = serde_json::from_str(&inv.args).unwrap_or_else(
                |e| serde_json::json!({ "_raw": &inv.args, "_parse_error": e.to_string() }),
            );
            if !self.approve(&inv).await {
                let denied = format!(
                    "denied by user — tool `{}` was not executed; proceed without its output",
                    inv.name
                );
                let tpart = ToolResult {
                    call_id: inv.id.clone(),
                    output: denied.clone(),
                };
                let _ = tx.send(ToolPart::ToolResult(tpart));
                self.transcript.push(Message::tool(&inv.id, denied));
                continue;
            }
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

    /// Ask the UI whether `inv` may run. Fail-safe deny: if the UI is gone
    /// (channel closed, reply dropped, quit) the tool does not run.
    async fn approve(&self, inv: &ToolInvocation) -> bool {
        if !needs_approval(&inv.name) {
            return true;
        }
        let Some(tx) = &self.approval_tx else {
            return true; // headless (tests): never hang without a UI
        };
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let req = ApprovalRequest {
            invocation: inv.clone(),
            reply: reply_tx,
        };
        if tx.send(req).is_err() {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }
}

/// Merge one OpenAI `tool_call` delta fragment into an in-progress invocation.
/// Returns the delta's `index` (0 when absent).
fn apply_tool_call_delta(inv: &mut ToolInvocation, tc: &Value) -> Option<usize> {
    let idx = tc
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|u| u as usize)?;
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
