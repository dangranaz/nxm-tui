//! Bounded execution (loopeng-01): hard cap on `Agent::run` iterations.
//!
//! R1 — `BudgetTracker` pure logic: `new(3)` allows exactly 3 ticks then
//! reports `Halted { steps: 3 }`; `new(0)` halts on the first tick.
//! R2 — `ToolPart::LoopHalted` renders a readable warning line.
//! R3 — integration: an Agent against a server that ALWAYS answers with
//! `tool_calls` (never `finish_reason: "stop"`) terminates after exactly
//! `max_steps` model turns and emits one `LoopHalted` part.

use std::time::Duration;

use nxm_tui::agent::Agent;
use nxm_tui::app::{Message, Role};
use nxm_tui::bounded::{Budget, BudgetTracker, DEFAULT_MAX_STEPS};
use nxm_tui::plan_mode::plan_mode_handle;
use nxm_tui::tool_types::ToolPart;
use nxm_tui_tools::default_registry;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[test]
fn budget_tracker_allows_exactly_max_steps() {
    let mut tracker = BudgetTracker::new(3);
    assert_eq!(tracker.tick(), Budget::Continue);
    assert_eq!(tracker.tick(), Budget::Continue);
    assert_eq!(tracker.tick(), Budget::Continue);
    assert_eq!(tracker.tick(), Budget::Halted { steps: 3 });
}

#[test]
fn budget_tracker_zero_halts_immediately() {
    let mut tracker = BudgetTracker::new(0);
    assert_eq!(tracker.tick(), Budget::Halted { steps: 0 });
}

#[test]
fn budget_tracker_keeps_reporting_halted_after_cap() {
    // Once halted, further ticks must not restart the budget.
    let mut tracker = BudgetTracker::new(1);
    assert_eq!(tracker.tick(), Budget::Continue);
    assert_eq!(tracker.tick(), Budget::Halted { steps: 1 });
    assert_eq!(tracker.tick(), Budget::Halted { steps: 1 });
}

#[test]
fn default_cap_is_15() {
    assert_eq!(DEFAULT_MAX_STEPS, 15);
}

#[test]
fn loop_halted_renders_reason_and_steps() {
    let part = ToolPart::LoopHalted {
        reason: "max iterations reached".to_string(),
        steps: 15,
    };
    let lines = part.render_lines();
    assert!(!lines.is_empty(), "LoopHalted must render a line");
    let text = format!("{:?}", lines[0]);
    assert!(
        text.contains("max iterations reached"),
        "reason missing: {text}"
    );
    assert!(text.contains("15"), "step count missing: {text}");
}

/// Wrap an SSE `data:` payload in a minimal HTTP/1.1 response.
fn sse(payload: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
         data: {payload}\n\
         data: [DONE]\n\n"
    )
}

/// Read one full HTTP request (headers + body by Content-Length) off the socket.
async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut buf = [0u8; 4096];
    let mut req = Vec::new();
    loop {
        let n = socket.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
        if req.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let s = String::from_utf8_lossy(&req).to_string();
    let (headers, rest) = s.split_once("\r\n\r\n").unwrap_or((&s, ""));
    let len = headers
        .lines()
        .filter_map(|l| l.strip_prefix("content-length:"))
        .next()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(rest.len());
    let mut body = rest.to_string();
    while body.len() < len {
        let n = socket.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        body.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    format!("{headers}\r\n\r\n{body}")
}

/// Fake server answering EVERY request with a `tool_calls` SSE turn (never
/// `finish_reason: "stop"`), so the agent loop would spin forever without a cap.
async fn always_tool_calls_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}/", addr);
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let _ = read_http_request(&mut socket).await;
            // A tool the registry does not know: dispatch records the error as
            // the tool result — hermetic, no filesystem/network touch.
            let resp = sse(&serde_json::to_string(&serde_json::json!({
                "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_spin",
                    "type": "function",
                    "function": {"name": "nonexistent_tool", "arguments": "{}"}}]},
                    "finish_reason": "tool_calls"}]
            }))
            .unwrap());
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    (base, server)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_halts_at_max_steps_and_emits_loop_halted() {
    let (base, server) = always_tool_calls_server().await;

    let client = reqwest::Client::new();
    let mut agent = Agent::new(
        &client,
        &base,
        "mock-model",
        None,
        vec![Message::new(Role::User, "loop forever".to_string())],
        None, // headless: gated tools auto-approved, never hangs
        default_registry(),
        plan_mode_handle(),
        2,                                                 // max_steps
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD, // stall_threshold
    );
    let (tx, mut rx) = mpsc::unbounded_channel::<ToolPart>();

    // Without a cap this loop is infinite: fail fast instead of hanging.
    let result = tokio::time::timeout(Duration::from_secs(30), agent.run(&tx))
        .await
        .expect("agent.run must terminate: iteration cap missing?");
    drop(tx);

    assert!(result.is_ok(), "hard stop must return Ok, got: {result:?}");
    let mut invocations = 0;
    let mut halted = 0;
    while let Some(part) = rx.recv().await {
        match part {
            ToolPart::ToolInvocation(_) => invocations += 1,
            ToolPart::LoopHalted { reason, steps } => {
                halted += 1;
                assert_eq!(steps, 2, "steps must equal max_steps");
                assert!(!reason.is_empty(), "reason must explain the halt");
            }
            _ => {}
        }
    }
    assert_eq!(invocations, 2, "exactly max_steps model turns must run");
    assert_eq!(halted, 1, "exactly one LoopHalted part must be emitted");
    server.abort();
}
