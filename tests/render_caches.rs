//! Render-path caches in `app.rs` (T4, `wayfinder:task`).
//!
//! - `estimated_tokens` is memoized: the value must track message growth —
//!   a cache with a too-coarse key (e.g. messages.len() only) would serve a
//!   stale value while the last message streams. Output differential, not
//!   internals.
//! - `current_dir_name` is cached: repeated calls stay consistent.

#![allow(dead_code, unused_imports)]

use nxm_tui::app::{App, Message, Role};

#[test]
fn token_estimate_tracks_streaming_growth() {
    let mut app = App::new();
    app.push_token("short");
    // "short" (5 chars) -> 5/4+4 = 5, floored at 8.
    assert_eq!(app.estimated_tokens(), 8);
    // Grow the same message to 36 chars -> 36/4+4 = 13. A stale cache
    // (key not covering last-message growth) would still report 8.
    app.push_token(" aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(app.estimated_tokens(), 13);
}

#[test]
fn token_estimate_recomputes_when_a_message_arrives() {
    let mut app = App::new();
    app.push_token("answer text here"); // 16 chars -> 16/4+4 = 8
    assert_eq!(app.estimated_tokens(), 8);
    // A tool result lands as its own message: (16/4+4) + (6/4+4) = 13.
    app.messages.push(Message::tool("c1", "output".to_string()));
    assert_eq!(app.estimated_tokens(), 13);
}

#[test]
fn token_estimate_matches_a_fresh_app() {
    // Differential oracle: the streamed app and a fresh app built with the
    // same messages must agree (guards any cache corruption).
    let mut streamed = App::new();
    streamed.push_token("hello **world** this is a longer answer");
    let mut fresh = App::new();
    fresh.messages.push(Message::new(
        Role::Assistant,
        "hello **world** this is a longer answer".to_string(),
    ));
    assert_eq!(streamed.estimated_tokens(), fresh.estimated_tokens());
}

#[test]
fn current_dir_name_is_consistent_across_calls() {
    let first = nxm_tui::app::current_dir_name();
    for _ in 0..100 {
        assert_eq!(
            nxm_tui::app::current_dir_name(),
            first,
            "cached cwd must not flip between calls within a session"
        );
    }
}
