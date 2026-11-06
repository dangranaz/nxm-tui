//! Live end-to-end check against a running Ollama (127.0.0.1:11434).
//! Ignored by default (needs the server). Run with:
//!   cargo test --test live_ollama -- --ignored --nocapture
//!
//! Drives the real `Agent::run` and collects streamed tokens, proving the
//! TUI's inference path works end-to-end (independent of vhs timing).

use nxm_tui::agent::Agent;
use nxm_tui::app::{Message, Role};
use nxm_tui::plan_mode::plan_mode_handle;
use nxm_tui::tool_types::ToolPart;
use nxm_tui_tools::default_registry;
use reqwest::Client;
use tokio::sync::mpsc;

#[tokio::test]
#[ignore]
async fn live_answer_streams_from_ollama() {
    let client = Client::new();
    let mut agent = Agent::new(
        &client,
        "http://127.0.0.1:11434",
        "qwen2.5:1.5b",
        None,
        vec![Message::new(
            Role::User,
            "Reply with exactly: hello world".to_string(),
        )],
        None, // headless: gated tools auto-approved
        default_registry(),
        plan_mode_handle(),
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    );

    let (tx, mut rx) = mpsc::unbounded_channel::<ToolPart>();

    // Unbounded channel: run can complete fully without a reader. Run first
    // (fills the channel), then drop the sender and drain the tokens.
    let result = agent.run(&tx).await;
    drop(tx);

    let mut text = String::new();
    while let Some(part) = rx.recv().await {
        if let ToolPart::Text(t) = part {
            text.push_str(&t);
        }
    }

    println!("=== agent result: {result:?}");
    println!("=== streamed text: {text:?}");
    assert!(result.is_ok(), "agent.run failed: {result:?}");
    assert!(!text.trim().is_empty(), "no tokens streamed from model");
}
