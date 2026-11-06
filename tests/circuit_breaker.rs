//! Circuit breaker on loop stagnation (loopeng-01-02).
//!
//! R1 — `StallDetector` pure logic: `new(3)`, same signature 3× → `Tripped`;
//! a different signature resets the counter. `new(1)` trips on the first
//! observation (degenerate low threshold, documented).
//! R2 — `signature()` is deterministic and order-independent within a cycle.
//! R3 — integration: an `Agent` against a server that ALWAYS answers with the
//! same `tool_calls` terminates by stagnation and emits a `LoopHalted` whose
//! `reason` reports the stall.

use std::time::Duration;

use nxm_tui::agent::Agent;
use nxm_tui::app::{Message, Role};
use nxm_tui::circuit_breaker::{signature, Stall, StallDetector, DEFAULT_STALL_THRESHOLD};
use nxm_tui::plan_mode::plan_mode_handle;
use nxm_tui::tool_types::ToolPart;
use nxm_tui_tools::default_registry;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

// ---- R1: pure detector ----

#[test]
fn stall_detector_trips_after_threshold_consecutive_repeats() {
    let mut d = StallDetector::new(3);
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 2 });
    assert_eq!(
        d.observe("A"),
        Stall::Tripped {
            repeats: 3,
            signature: "A".into()
        }
    );
}

#[test]
fn stall_detector_different_signature_resets() {
    // A,A,B,A,A must NOT trip with threshold 3: B is progress, resets to 1.
    let mut d = StallDetector::new(3);
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 2 });
    assert_eq!(d.observe("B"), Stall::Ok { repeats: 1 });
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    assert_eq!(d.observe("A"), Stall::Ok { repeats: 2 });
}

#[test]
fn stall_detector_threshold_one_trips_on_first_observation() {
    // Soglia 1: ogni ciclo è già una "ripetizione di 1" → trip immediato.
    let mut d = StallDetector::new(1);
    assert_eq!(
        d.observe("A"),
        Stall::Tripped {
            repeats: 1,
            signature: "A".into()
        }
    );
}

#[test]
fn default_stall_threshold_is_3() {
    assert_eq!(DEFAULT_STALL_THRESHOLD, 3);
}

// ---- R2: signature ----

#[test]
fn signature_is_order_independent() {
    let a = signature(&[
        ("read_file".into(), "{\"path\":\"a\"}".into()),
        ("list_resources".into(), "{}".into()),
    ]);
    let b = signature(&[
        ("list_resources".into(), "{}".into()),
        ("read_file".into(), "{\"path\":\"a\"}".into()),
    ]);
    assert_eq!(a, b);
}

#[test]
fn signature_changes_when_an_arg_changes() {
    let a = signature(&[("read_file".into(), "{\"path\":\"a\"}".into())]);
    let b = signature(&[("read_file".into(), "{\"path\":\"b\"}".into())]);
    assert_ne!(a, b);
}

#[test]
fn signature_changes_when_a_call_is_added() {
    let a = signature(&[("read_file".into(), "{}".into())]);
    let b = signature(&[
        ("read_file".into(), "{}".into()),
        ("list_resources".into(), "{}".into()),
    ]);
    assert_ne!(a, b);
}

// ---- helpers for the integration test ----

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

/// Fake server answering EVERY request with the SAME `tool_calls` turn
/// (same name + args, never `finish_reason: "stop"`), so the agent loop
/// would spin forever without the stagnation breaker.
async fn always_same_tool_calls_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}/", addr);
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let _ = read_http_request(&mut socket).await;
            let resp = sse(&serde_json::to_string(&serde_json::json!({
                "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_stall",
                    "type": "function",
                    "function": {"name": "stall_tool", "arguments": "{}"}}]},
                    "finish_reason": "tool_calls"}]
            }))
            .unwrap());
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    (base, server)
}

// ---- R3: integration ----

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_halts_on_stagnation_and_emits_loop_halted() {
    let (base, server) = always_same_tool_calls_server().await;

    let client = reqwest::Client::new();
    let mut agent = Agent::new(
        &client,
        &base,
        "mock-model",
        None,
        vec![Message::new(
            Role::User,
            "loop on the same tool".to_string(),
        )],
        None, // headless: gated tools auto-approved, never hangs
        default_registry(),
        plan_mode_handle(),
        50, // max_steps: alto, non deve interferire
        2,  // stall_threshold: trip alla 2ª iterazione identica
    );
    let (tx, mut rx) = mpsc::unbounded_channel::<ToolPart>();

    // Senza il breaker il loop è infinito: fail-fast invece di appendere.
    let result = tokio::time::timeout(Duration::from_secs(30), agent.run(&tx))
        .await
        .expect("agent.run must terminate: stagnation breaker missing?");
    drop(tx);

    assert!(result.is_ok(), "hard stop must return Ok, got: {result:?}");
    let mut halted = 0;
    while let Some(part) = rx.recv().await {
        if let ToolPart::LoopHalted { reason, steps } = part {
            halted += 1;
            assert!(
                reason.to_lowercase().contains("stall"),
                "reason must explain the stagnation, got: {reason}"
            );
            assert!(steps > 0, "steps must be the current iteration index");
        }
    }
    assert_eq!(halted, 1, "exactly one LoopHalted part must be emitted");
    server.abort();
}
