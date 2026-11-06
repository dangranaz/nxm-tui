//! The sampling bridge: turn a server's `sampling/createMessage` into a real
//! LLM completion via the TUI's OpenAI-compatible backend, then back into an
//! MCP [`CreateMessageResult`].
//!
//! Per the user's decision this bridge **auto-approves** every request (no
//! human-in-the-loop overlay in this minimal cut — see docs for the refinement
//! path). The trait [`SamplingHandler`] is the seam the client calls; the
//! concrete [`LlmSamplingHandler`] talks HTTP. Tests inject a fake handler.
//!
//! No new dependency: object-safe async is expressed with a boxed future
//! (`futures::future::BoxFuture`) rather than the `async-trait` crate.

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, info};

use crate::mcp::protocol::{Content, CreateMessageParams, CreateMessageResult, SamplingMessage};

/// Tracing target per `RULES.md`.
const TARGET: &str = "nexum::mcp::sampling";

/// The seam the MCP client calls when a server requests `sampling/createMessage`.
/// Object-safe (`dyn SamplingHandler`) so the client can hold `Box<dyn ..>` and
/// tests can substitute a fake without a live server or network.
pub trait SamplingHandler: Send + Sync {
    /// Handle one sampling request, producing an MCP result (auto-approved).
    fn handle<'a>(
        &'a self,
        params: CreateMessageParams,
    ) -> BoxFuture<'a, Result<CreateMessageResult>>;
}

/// Production handler: forwards to an OpenAI-compatible `/v1/chat/completions`
/// endpoint (the same backend the TUI's [`crate::agent::Agent`] uses).
pub struct LlmSamplingHandler {
    client: Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl LlmSamplingHandler {
    /// Build a handler bound to a base URL + model (+ optional bearer key).
    pub fn new(
        client: Client,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            model: model.into(),
            api_key,
        }
    }

    /// Perform the actual (non-streaming) completion call.
    async fn complete(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        let messages = build_openai_messages(&params);
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        if let Some(max) = params.max_tokens {
            body["max_tokens"] = Value::from(max);
        }
        if let Some(temp) = params.temperature {
            body["temperature"] = Value::from(temp);
        }

        let mut req = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.header("authorization", format!("Bearer {key}"));
        }

        let resp = req.send().await.context("sampling request failed")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("sampling backend returned {status}");
        }
        let json: Value = resp.json().await.context("sampling response not JSON")?;

        let text = json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let stop = json
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .map(map_stop_reason)
            .unwrap_or("endTurn");

        Ok(CreateMessageResult::assistant_text(text, &self.model, stop))
    }
}

impl SamplingHandler for LlmSamplingHandler {
    fn handle<'a>(
        &'a self,
        params: CreateMessageParams,
    ) -> BoxFuture<'a, Result<CreateMessageResult>> {
        Box::pin(async move {
            info!(
                target: TARGET,
                messages = params.messages.len(),
                "ENTER handle sampling (auto-approved)"
            );
            let result = self.complete(params).await;
            debug!(target: TARGET, ok = result.is_ok(), "EXIT handle sampling");
            result
        })
    }
}

/// Map an OpenAI `finish_reason` to an MCP `stopReason`.
fn map_stop_reason(reason: &str) -> &'static str {
    match reason {
        "length" => "maxTokens",
        "stop" => "endTurn",
        _ => "endTurn",
    }
}

/// Convert MCP sampling messages (+ optional system prompt) into the OpenAI
/// `messages` array. Non-text content degrades to a text placeholder.
fn build_openai_messages(params: &CreateMessageParams) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    if let Some(sys) = &params.system_prompt {
        out.push(serde_json::json!({ "role": "system", "content": sys }));
    }
    for m in &params.messages {
        out.push(sampling_message_to_openai(m));
    }
    out
}

/// Map a single sampling message to an OpenAI role/content object.
fn sampling_message_to_openai(m: &SamplingMessage) -> Value {
    let role = match m.role.as_str() {
        "assistant" => "assistant",
        _ => "user",
    };
    let content = match &m.content {
        Content::Text { text } => text.clone(),
        other => other.as_text(),
    };
    serde_json::json!({ "role": role, "content": content })
}
