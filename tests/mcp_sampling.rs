//! Behavioural tests for the sampling bridge seam.
//!
//! We exercise the [`SamplingHandler`] contract with a fake handler (no
//! network, no live MCP server): a `sampling/createMessage` params object goes
//! in, an auto-approved [`CreateMessageResult`] comes out. This is the exact
//! seam `McpClient` calls when a server (e.g. `nxm-session`) requests a
//! completion to populate a handoff.

use futures::future::BoxFuture;
use nxm_tui::mcp::protocol::{Content, CreateMessageParams, CreateMessageResult, SamplingMessage};
use nxm_tui::mcp::sampling::SamplingHandler;

/// A fake handler that echoes the last user message back as the assistant,
/// standing in for a real LLM backend. Records that it was invoked.
struct EchoHandler {
    model: String,
}

impl SamplingHandler for EchoHandler {
    fn handle<'a>(
        &'a self,
        params: CreateMessageParams,
    ) -> BoxFuture<'a, anyhow::Result<CreateMessageResult>> {
        Box::pin(async move {
            let last = params
                .messages
                .last()
                .map(|m| m.content.as_text())
                .unwrap_or_default();
            Ok(CreateMessageResult::assistant_text(
                format!("echo: {last}"),
                &self.model,
                "endTurn",
            ))
        })
    }
}

fn user_text(t: &str) -> SamplingMessage {
    SamplingMessage {
        role: "user".into(),
        content: Content::text(t),
    }
}

#[tokio::test]
async fn handler_auto_approves_and_returns_assistant_result() {
    let handler = EchoHandler {
        model: "fake-model".into(),
    };
    let params = CreateMessageParams {
        messages: vec![user_text("summarize the session decisions")],
        system_prompt: Some("You are a handoff writer.".into()),
        model_preferences: None,
        max_tokens: Some(256),
        temperature: None,
    };

    let result = handler.handle(params).await.expect("handler succeeds");

    assert_eq!(result.role, "assistant");
    assert_eq!(result.model, "fake-model");
    assert_eq!(result.stop_reason.as_deref(), Some("endTurn"));
    match result.content {
        Content::Text { text } => {
            assert_eq!(text, "echo: summarize the session decisions");
        }
        _ => panic!("expected text content"),
    }
}

#[tokio::test]
async fn handler_handles_empty_conversation() {
    let handler = EchoHandler {
        model: "fake-model".into(),
    };
    let params = CreateMessageParams {
        messages: vec![],
        system_prompt: None,
        model_preferences: None,
        max_tokens: None,
        temperature: None,
    };
    let result = handler.handle(params).await.expect("handler succeeds");
    match result.content {
        Content::Text { text } => assert_eq!(text, "echo: "),
        _ => panic!("expected text content"),
    }
}

#[tokio::test]
async fn handler_is_object_safe_behind_box() {
    // The client stores `Box<dyn SamplingHandler>`; prove the fake fits.
    let handler: Box<dyn SamplingHandler> = Box::new(EchoHandler {
        model: "boxed".into(),
    });
    let params = CreateMessageParams {
        messages: vec![user_text("hi")],
        system_prompt: None,
        model_preferences: None,
        max_tokens: None,
        temperature: None,
    };
    let result = handler.handle(params).await.expect("boxed handler works");
    assert_eq!(result.model, "boxed");
}

/// Live handshake against the real `nxm-session-mcp` binary. Skips (passes)
/// when the binary is not present so CI without it stays green. Proves the
/// stdio transport + `initialize` (declaring `sampling`) work end-to-end.
#[tokio::test]
async fn live_initialize_against_real_server_if_present() {
    use nxm_tui::mcp::transport::binary_present;

    let command = "nxm-session-mcp";
    if !binary_present(command) {
        eprintln!("skip: {command} not on PATH");
        return;
    }

    let http = reqwest::Client::new();
    let args = vec!["--transport".to_string(), "stdio".to_string()];
    let handle = nxm_tui::mcp::connect_if_present(
        command,
        &args,
        http,
        "http://127.0.0.1:11434",
        "default",
        None,
    )
    .await
    .expect("connect_if_present should not error");

    assert!(
        handle.is_some(),
        "expected a live session handle when the binary is present"
    );
}

/// `connect_from_config` is a clean no-op (`Ok(None)`) when MCP is disabled in
/// config — it must never spawn anything or touch the network. Deterministic,
/// headless-safe (no binary / PATH dependency).
#[tokio::test]
async fn connect_from_config_skips_when_disabled() {
    let mut cfg = nxm_tui::config::TuiConfig::default();
    cfg.mcp.enabled = false;

    let result = nxm_tui::mcp::connect_from_config(&cfg, "http://127.0.0.1:11434", None)
        .await
        .expect("disabled MCP must not error");

    assert!(result.is_none(), "disabled MCP must yield Ok(None)");
}

/// `connect_from_config` cleanly skips (`Ok(None)`) when enabled but the server
/// binary is absent — the normal chat path must never be blocked by a missing
/// MCP server. Uses an implausible binary name so the probe deterministically
/// fails regardless of host.
#[tokio::test]
async fn connect_from_config_skips_when_binary_absent() {
    let mut cfg = nxm_tui::config::TuiConfig::default();
    cfg.mcp.enabled = true;
    cfg.mcp.command = "nxm-nonexistent-mcp-binary-xyzzy".to_string();

    let result = nxm_tui::mcp::connect_from_config(&cfg, "http://127.0.0.1:11434", None)
        .await
        .expect("absent binary must be a clean skip, not an error");

    assert!(result.is_none(), "absent binary must yield Ok(None)");
}
