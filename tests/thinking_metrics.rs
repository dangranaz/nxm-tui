//! Reasoning overlay + per-token metrics (pi.dev/codex pattern port).
//!
//! Verifies the two wiring fixes:
//! - `App::push_token` streams text into the assistant message while
//!   `Metrics::record_chars` accumulates per-token counts (was dead code).
//! - Thinking deltas accumulate in `thinking_content` and the
//!   `show_thinking` overlay flag defaults off (toggled by Ctrl+R).

#![allow(dead_code, unused_imports)]


use nxm_tui::app::{App, Role};

#[test]
fn push_token_streams_text_into_assistant_message() {
    let mut app = App::new();
    app.push_token("Hello ");
    app.push_token("world");
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Assistant);
    assert_eq!(app.messages[0].content, "Hello world");
}

#[test]
fn record_chars_accumulates_per_token_counts() {
    let mut app = App::new();
    app.metrics.start_response();
    let t1 = "Hello ";
    let t2 = "world";
    app.metrics.record_chars(t1.chars().count());
    app.push_token(t1);
    app.metrics.record_chars(t2.chars().count());
    app.push_token(t2);
    assert_eq!(app.metrics.response_char_count, 11);
    assert_eq!(app.metrics.total_chars, 11);
    app.metrics.finish_response();
    assert_eq!(app.metrics.total_responses, 1);
    assert!(app.metrics.total_tokens >= 1);
}

#[test]
fn thinking_delta_accumulates_and_overlay_flag_defaults_off() {
    let mut app = App::new();
    assert!(!app.show_thinking);
    assert!(app.thinking_content.is_empty());
    app.append_thinking_delta("step 1; ");
    app.append_thinking_delta("step 2");
    assert!(app.thinking_content.contains("step 1"));
    assert!(app.thinking_content.contains("step 2"));
    // Simulates Ctrl+R toggle in handler.rs
    app.show_thinking = !app.show_thinking;
    assert!(app.show_thinking);
}

#[test]
fn apply_token_filters_thinking_tags() {
    let mut app = App::new();
    app.push_token("<thinking>");
    app.push_token("hidden ");
    app.push_token(">");
    app.push_token("visible");
    assert!(app.thinking_content.contains("hidden "));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "visible");
}

#[test]
fn apply_token_keeps_text_after_closing_bracket() {
    let mut app = App::new();
    app.push_token("<thinking>");
    app.push_token("hidden ");
    app.push_token(">visible");
    assert!(app.thinking_content.contains("hidden "));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "visible");
}

#[test]
fn apply_token_handles_single_token_open_and_close() {
    let mut app = App::new();
    app.push_token("before<thinking>hidden</thinking>after");
    assert!(app.thinking_content.contains("hidden"));
    let combined: String = app
        .messages
        .iter()
        .map(|m| m.content.clone())
        .collect();
    assert!(combined.contains("before"));
    assert!(combined.contains("after"));
    assert!(!combined.contains("hidden"));
}
