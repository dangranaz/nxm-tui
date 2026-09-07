//! P3 verification — headless *real* round-trip (no TTY, no provider key).
//!
//! `tests/roundtrip.rs` compiles the SAME `src/agent.rs` the binary links
//! against (via `#[path]`) and drives it against a mock OpenAI-compatible SSE
//! server. The agent streams a `tool_call` → `nexum-tools::read_file` reads a
//! real temp file → the result is sent back as `role:"tool"` → the agent emits
//! final assistant text. It also asserts the `Authorization: Bearer` header is
//! forwarded when `api_key` is set (the NVIDIA/Ollama auth wiring).
//!
//! Module closure {agent, app, tool_types, server_proc, session} is
//! self-contained (reference each other only), so no `lib.rs`/`pub` refactor.

#![allow(dead_code, unused_imports)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[path = "../src/tool_types.rs"]
mod tool_types;
#[path = "../src/app.rs"]
mod app;
#[path = "../src/session.rs"]
mod session;
#[path = "../src/server_proc.rs"]
mod server_proc;
#[path = "../src/prompt.rs"]
mod prompt;
#[path = "../src/prompt_lines.rs"]
mod prompt_lines;
#[path = "../src/agent.rs"]
mod agent;

use agent::Agent;
use app::{Message, Role};
use tool_types::ToolPart;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_real_round_trip_read_file() {
    // Real temp file the agent's `read_file` tool will read for real.
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("nxm_tui_roundtrip_{id}.txt"));
    let payload = String::from("hello from the mock-model round-trip");
    std::fs::write(&path, &payload).unwrap();
    let path_str = path.display().to_string();
    let mock_path = path_str.clone();

    // Bind once on the test runtime, hand the bound listener to the mock task.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}/", addr);

    // Mock OpenAI server: turn 0 -> `tool_call(read_file)`; turn 1 -> final text.
    let mock = tokio::spawn(async move {
        for turn in 0..2usize {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let req = read_http_request(&mut socket).await;
            assert!(
                req.to_ascii_lowercase().contains("authorization: bearer test-key"),
                "bearer token must be forwarded to the OpenAI-compatible endpoint"
            );
            let resp = if turn == 0 {
                let args = serde_json::to_string(&serde_json::json!({"path": &mock_path})).unwrap();
                sse(&serde_json::to_string(&serde_json::json!({
                    "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_read",
                        "type": "function", "function": {"name": "read_file", "arguments": args}}]},
                               "finish_reason": "tool_calls"}]
                })).unwrap())
            } else {
                sse(&serde_json::to_string(&serde_json::json!({
                    "choices": [{"delta": {"content": "read_file OK"}, "finish_reason": "stop"}]
                })).unwrap())
            };
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    let client = reqwest::Client::new();
    let model = String::from("mock-model");
    let initial = vec![Message::new(Role::User, String::from("read the temp file"))];
    let mut agent = Agent::new(&client, &base, &model, Some(String::from("test-key")), initial);

    let (tx, mut rx) = mpsc::unbounded_channel::<ToolPart>();
    agent.run(&tx).await.expect("agent turn must complete");
    drop(tx);

    println!("--- round-trip parts ---");
    let mut parts = Vec::new();
    while let Ok(p) = rx.try_recv() {
        println!("  -> {p:?}");
        parts.push(p);
    }
    println!("--- end round-trip parts ---");

    assert!(
        parts.iter().any(|p| matches!(
            p,
            ToolPart::ToolInvocation(i) if i.name == "read_file" && i.args.contains(path_str.as_str())
        )),
        "expected a read_file ToolInvocation carrying the temp path"
    );
    let result = parts
        .iter()
        .find_map(|p| match p {
            ToolPart::ToolResult(t) => Some(t.clone()),
            _ => None,
        })
        .expect("expected a ToolResult from read_file");
    assert_eq!(result.call_id, "call_read");
    assert_eq!(result.output, payload, "read_file returned the file contents");
    assert!(
        parts
            .iter()
            .any(|p| matches!(p, ToolPart::Text(t) if t.contains("read_file OK"))),
        "expected final assistant text after the tool round-trip"
    );
    assert!(
        !parts.iter().any(|p| matches!(p, ToolPart::Error(_))),
        "no errors in the round-trip"
    );

    let _ = mock.await;
    let _ = std::fs::remove_file(&path);
}
