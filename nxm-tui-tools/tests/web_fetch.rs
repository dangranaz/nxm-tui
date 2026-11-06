//! Tests for the `web_fetch` tool (R1-R5).
//!
//! R3/R4 use a raw TCP test server speaking minimal HTTP — zero new
//! dependencies, per `prompts/27-tools-web-fetch_ITA.md`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use nxm_tui_tools::{default_registry, Tool};
use serde_json::json;

fn spawn_test_server(body: &str, status: &str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let port = listener.local_addr().expect("local addr").port();
    let body = body.to_string();
    let status = status.to_string();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut req = [0u8; 1024];
            let _ = stream.read(&mut req);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

// Helper to run web_fetch with allow-local for tests that need loopback access.
fn reg_with_allow_local() -> nxm_tui_tools::Registry {
    std::env::set_var("NEXUM_WEBFETCH_ALLOW_LOCAL", "1");
    default_registry()
}

#[test]
fn web_fetch_schema() {
    let reg = default_registry();
    let tool = reg.get("web_fetch").expect("web_fetch registered");
    assert_eq!(tool.name(), "web_fetch");
    assert!(!tool.description().is_empty());
    let schema = tool.schema();
    let required = schema["required"].as_array().expect("required array");
    assert!(required.iter().any(|v| v.as_str() == Some("url")));
    assert_eq!(schema["properties"]["url"]["type"].as_str(), Some("string"));
}

#[test]
fn web_fetch_missing_url() {
    let reg = default_registry();
    let err = reg.call("web_fetch", &json!({})).unwrap_err();
    assert!(err.to_string().contains("url"));
}

#[test]
fn web_fetch_ok() {
    let port = spawn_test_server("hello-fetch", "200 OK");
    let reg = reg_with_allow_local();
    let args = json!({ "url": format!("http://127.0.0.1:{port}") });
    let res = reg.call("web_fetch", &args).expect("run web_fetch");
    assert!(res.ok);
    assert_eq!(res.content, "hello-fetch");
}

#[test]
fn web_fetch_http_error() {
    let port = spawn_test_server("not found", "404 Not Found");
    let reg = reg_with_allow_local();
    let args = json!({ "url": format!("http://127.0.0.1:{port}") });
    let res = reg.call("web_fetch", &args).expect("run web_fetch");
    assert!(!res.ok);
    assert!(res.content.contains("404"));
}

#[test]
fn web_fetch_in_default_registry() {
    let reg = default_registry();
    // MT-3: registration no longer auto-enables; verify registration only.
    assert!(reg.tool_names().contains(&"web_fetch"));
}
