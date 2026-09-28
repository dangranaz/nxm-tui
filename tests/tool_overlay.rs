//! Tool detail overlay + char-safe truncation (kimi approval-preview pattern).
//!
//! - `ToolPart::truncate_chars` never splits UTF-8 codepoints (the old byte
//!   slice panicked on multibyte boundaries).
//! - `ToolInvocation` rows render an args preview.
//! - `last_tool_exchange` finds the most recent invocation + its result.

#![allow(dead_code, unused_imports)]


use nxm_tui::app::{App, Message, Role};
use nxm_tui::tool_overlay;
use nxm_tui::tool_types::{ToolInvocation, ToolPart, ToolResult};

fn invocation(id: &str, name: &str, args: &str) -> Message {
    Message::with_tool(
        Role::Assistant,
        vec![ToolPart::ToolInvocation(ToolInvocation {
            id: id.to_string(),
            name: name.to_string(),
            args: args.to_string(),
        })],
    )
}

#[test]
fn truncate_chars_is_multibyte_safe() {
    // 150 emoji = 600 bytes; a byte slice at 100 would panic or split a codepoint.
    let s = "😀".repeat(150);
    let out = ToolPart::truncate_chars(&s, 100);
    assert_eq!(out.chars().count(), 101); // 100 + "…"
    assert!(out.ends_with('…'));
    let short = "hello";
    assert_eq!(ToolPart::truncate_chars(short, 100), "hello");
}

#[test]
fn invocation_row_renders_args_preview() {
    let part = ToolPart::ToolInvocation(ToolInvocation {
        id: "c1".to_string(),
        name: "read_file".to_string(),
        args: "{\"path\": \"/tmp/x.txt\"}".to_string(),
    });
    let lines = part.render_lines();
    assert_eq!(lines.len(), 1);
    let text: String = format!("{:?}", lines[0]);
    assert!(text.contains("read_file"));
    assert!(text.contains("/tmp/x.txt"));
}

#[test]
fn invocation_row_without_args_renders_name_only() {
    let part = ToolPart::ToolInvocation(ToolInvocation {
        id: "c1".to_string(),
        name: "list_resources".to_string(),
        args: String::new(),
    });
    let text = format!("{:?}", part.render_lines()[0]);
    assert!(text.contains("list_resources"));
}

#[test]
fn last_tool_exchange_finds_invocation_and_result() {
    let mut app = App::new();
    assert!(!app.show_tool);
    assert!(tool_overlay::last_tool_exchange(&app.messages).is_none());
    app.messages.push(Message::new(Role::User, "read it".to_string()));
    app.messages.push(invocation("call_1", "read_file", "{\"path\":\"/tmp/a\"}"));
    // Pending: invocation without result yet.
    let (inv, out) = tool_overlay::last_tool_exchange(&app.messages).expect("pending inv");
    assert_eq!(inv.name, "read_file");
    assert!(out.is_none());
    // Result arrives.
    app.messages.push(Message::tool("call_1", "file contents".to_string()));
    let (inv, out) = tool_overlay::last_tool_exchange(&app.messages).expect("resolved inv");
    assert_eq!(inv.id, "call_1");
    assert_eq!(out.as_deref(), Some("file contents"));
}

#[test]
fn last_tool_exchange_prefers_most_recent() {
    let mut app = App::new();
    app.messages.push(invocation("call_1", "read_file", "{}"));
    app.messages.push(Message::tool("call_1", "first".to_string()));
    app.messages.push(invocation("call_2", "web_search", "{\"q\":\"x\"}"));
    app.messages.push(Message::tool("call_2", "second".to_string()));
    let (inv, out) = tool_overlay::last_tool_exchange(&app.messages).expect("latest inv");
    assert_eq!(inv.name, "web_search");
    assert_eq!(out.as_deref(), Some("second"));
}
