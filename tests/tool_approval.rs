//! Tool approval gating (codex patch_approval / kimi approval-preview pattern).
//!
//! - Policy: only `list_resources` is auto-approved; `read_file`,
//!   `web_search` and unknown tools require a Y/N decision.
//! - Deny path: the tool is skipped and the denial is recorded as the tool
//!   result so the model can react on the next turn.
//! - Approve path: the tool executes for real.
//! - `App::resolve_approval` answers the oneshot and clears the slot.

#![allow(dead_code, unused_imports)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;


use nxm_tui::agent::Agent;
use nxm_tui::app::{App, Message, PendingApproval, Role};
use nxm_tui::tool_types::{self, needs_approval, risk_badge, ApprovalRequest, ToolInvocation, ToolPart};

#[test]
fn policy_auto_approves_only_list_resources() {
    assert!(!needs_approval("list_resources"));
    assert!(needs_approval("read_file"));
    assert!(needs_approval("web_search"));
    assert!(needs_approval("something_future"));
}

fn badge_for(name: &str, args: &str) -> Option<&'static str> {
    risk_badge(&ToolInvocation {
        id: "c1".to_string(),
        name: name.to_string(),
        args: args.to_string(),
    })
}

#[test]
fn risk_badge_flags_network_egress() {
    let badge = badge_for("web_search", "{\"query\": \"rust news\"}");
    assert!(badge.is_some());
    assert!(badge.unwrap().contains("network"));
}

#[test]
fn risk_badge_flags_sensitive_paths() {
    for path in [
        "/home/u/.env",
        "/home/u/proj/.env.local",
        "/home/u/.ssh/id_rsa",
        "C:\\Users\\u\\.aws\\credentials",
        "/etc/shadow",
        "../outside.txt",
        "/tmp/api_TOKEN.json",
        "/var/Private Keys/x.pem",
    ] {
        let args = serde_json::json!({"path": path}).to_string();
        assert!(
            badge_for("read_file", &args).is_some(),
            "expected sensitive badge for {path}"
        );
    }
}

#[test]
fn risk_badge_stays_quiet_for_plain_paths() {
    for path in [
        "src/main.rs",
        "/tmp/notes.txt",
        "/home/u/projects/demo/README.md",
    ] {
        let args = serde_json::json!({"path": path}).to_string();
        assert!(
            badge_for("read_file", &args).is_none(),
            "unexpected badge for {path}"
        );
    }
    assert!(badge_for("read_file", "not-json-at-all").is_none());
    assert!(badge_for("list_resources", "{\"path\": \"/tmp\"}").is_none());
    assert!(badge_for("list_resources", "{\"path\": \"~/.gnupg\"}").is_some());
}

#[test]
fn risk_badge_flags_unknown_tools() {
    let badge = badge_for("delete_everything", "{}");
    assert!(badge.is_some());
    assert!(badge.unwrap().contains("unrecognized"));
}

#[test]
fn resolve_approval_answers_and_clears() {
    let mut app = App::new();
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    app.pending_approval = Some(PendingApproval {
        invocation: tool_types::ToolInvocation {
            id: "c1".to_string(),
            name: "read_file".to_string(),
            args: "{}".to_string(),
        },
        reply: reply_tx,
    });
    app.show_approval = true;
    app.resolve_approval(false);
    assert!(app.pending_approval.is_none());
    assert!(!app.show_approval);
    assert!(!reply_rx.blocking_recv().unwrap());
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

/// Mock OpenAI server: turn 0 -> `tool_call(read_file <path>)`; turn 1 -> final text.
async fn mock_two_turn_server(path: String) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{}/", addr);
    let mock = tokio::spawn(async move {
        for turn in 0..2usize {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let _ = read_http_request(&mut socket).await;
            let resp = if turn == 0 {
                let args =
                    serde_json::to_string(&serde_json::json!({"path": &path})).unwrap();
                sse(&serde_json::to_string(&serde_json::json!({
                    "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_read",
                        "type": "function", "function": {"name": "read_file", "arguments": args}}]},
                               "finish_reason": "tool_calls"}]
                }))
                .unwrap())
            } else {
                sse(&serde_json::to_string(&serde_json::json!({
                    "choices": [{"delta": {"content": "all done"}, "finish_reason": "stop"}]
                }))
                .unwrap())
            };
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    (base, mock)
}

/// Drive one full agent turn, answering the (single) approval with `allow`.
/// Returns all emitted parts.
async fn run_turn_with_decision(
    base: &str,
    path: &str,
    allow: bool,
) -> Vec<ToolPart> {
    let client = reqwest::Client::new();
    let model = String::from("mock-model");
    let initial = vec![Message::new(Role::User, String::from("read the temp file"))];
    let (atx, mut arx) = mpsc::unbounded_channel::<ApprovalRequest>();
    let mut agent = Agent::new(&client, base, &model, None, initial, Some(atx));
    let (tx, mut rx) = mpsc::unbounded_channel::<ToolPart>();
    let agent_tx = tx.clone();
    let run_fut = agent.run(&agent_tx);
    tokio::pin!(run_fut);
    let mut approvals = 0;
    loop {
        tokio::select! {
            res = &mut run_fut => {
                res.expect("agent turn must complete");
                break;
            }
            req = arx.recv() => {
                let req = req.expect("expected exactly one approval request");
                assert_eq!(req.invocation.name, "read_file");
                assert!(req.invocation.args.contains(path));
                req.reply.send(allow).unwrap();
                approvals += 1;
            }
        }
    }
    assert_eq!(approvals, 1, "gated read_file must ask exactly once");
    drop(tx);
    let mut parts = Vec::new();
    while let Ok(p) = rx.try_recv() {
        parts.push(p);
    }
    parts
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denied_tool_is_skipped_and_reported() {
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("nxm_tui_deny_{id}.txt"));
    std::fs::write(&path, "TOP SECRET").unwrap();
    let path_str = path.display().to_string();

    let (base, mock) = mock_two_turn_server(path_str.clone()).await;
    let parts = run_turn_with_decision(&base, &path_str, false).await;

    let result = parts
        .iter()
        .find_map(|p| match p {
            ToolPart::ToolResult(t) => Some(t.clone()),
            _ => None,
        })
        .expect("expected a ToolResult even when denied");
    assert_eq!(result.call_id, "call_read");
    assert!(
        result.output.contains("denied"),
        "denial must be recorded as the tool result, got: {}",
        result.output
    );
    assert!(
        !result.output.contains("TOP SECRET"),
        "denied tool must not leak file contents"
    );
    assert!(
        parts
            .iter()
            .any(|p| matches!(p, ToolPart::Text(t) if t.contains("all done"))),
        "agent must continue to the final turn after a denial"
    );
    let _ = mock.await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_tool_executes_for_real() {
    let id = std::process::id();
    let path = std::env::temp_dir().join(format!("nxm_tui_allow_{id}.txt"));
    let payload = String::from("hello from the approval test");
    std::fs::write(&path, &payload).unwrap();
    let path_str = path.display().to_string();

    let (base, mock) = mock_two_turn_server(path_str.clone()).await;
    let parts = run_turn_with_decision(&base, &path_str, true).await;

    let result = parts
        .iter()
        .find_map(|p| match p {
            ToolPart::ToolResult(t) => Some(t.clone()),
            _ => None,
        })
        .expect("expected a ToolResult when approved");
    assert_eq!(result.output, payload);
    let _ = mock.await;
    let _ = std::fs::remove_file(&path);
}
