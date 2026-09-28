//! History viewport: follow-tail + scrollback model + UTF-8-safe markdown.
//!
//! - `viewport_offset` pins short buffers to the top, long buffers to the
//!   bottom while following, and clamps deep scrollbacks to the oldest line.
//! - Scroll keys move in the conventional direction (Up = older).
//! - Markdown code fences never panic on multibyte content.

#![allow(dead_code, unused_imports)]


use nxm_tui::app::App;
use nxm_tui::markdown;

#[test]
fn viewport_pins_short_buffers_to_top() {
    assert_eq!(App::viewport_offset(0, 20, 0), 0);
    assert_eq!(App::viewport_offset(10, 20, 0), 0);
    assert_eq!(App::viewport_offset(20, 20, 0), 0);
}

#[test]
fn viewport_follows_tail_when_scrollback_zero() {
    // 100 lines in a 20-line viewport: offset 80 shows the last 20.
    assert_eq!(App::viewport_offset(100, 20, 0), 80);
}

#[test]
fn viewport_hides_scrollback_lines_below() {
    assert_eq!(App::viewport_offset(100, 20, 5), 75);
    assert_eq!(App::viewport_offset(100, 20, 80), 0);
}

#[test]
fn viewport_clamps_deep_scrollback_to_oldest_line() {
    assert_eq!(App::viewport_offset(100, 20, 500), 0);
    assert_eq!(App::viewport_offset(100, 20, u16::MAX), 0);
}

#[test]
fn scroll_keys_move_in_conventional_direction() {
    let mut app = App::new();
    assert!(app.following());
    app.scroll_up();
    assert_eq!(app.scrollback, 1);
    assert!(!app.following());
    app.scroll_up();
    app.page_up(10);
    assert_eq!(app.scrollback, 12);
    app.page_down(10);
    assert_eq!(app.scrollback, 2);
    app.scroll_down();
    app.scroll_down();
    assert!(app.following());
    // Saturates at zero, never wraps.
    app.scroll_down();
    assert_eq!(app.scrollback, 0);
}

#[test]
fn new_content_keeps_pinned_view_at_tail() {
    let mut app = App::new();
    app.push_token("hello");
    assert!(app.following());
    assert_eq!(App::viewport_offset(3, 20, app.scrollback), 0);
}

#[test]
fn markdown_code_fence_with_emoji_does_not_panic() {
    let content = "```\n😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀\n```";
    let lines = markdown::render_markdown(content, 20, "   ");
    assert!(!lines.is_empty());
    let text = format!("{lines:?}");
    assert!(text.contains('…'), "long emoji line must be truncated, got: {text}");
}

#[test]
fn markdown_plain_text_still_renders() {
    let lines = markdown::render_markdown("hello **bold** and `code`", 40, "   ");
    assert!(!lines.is_empty());
    let text = format!("{lines:?}");
    // Inline parser emits per-char spans for plain text ("h", "e", …) but
    // single spans for **bold** and `code`.
    assert!(text.contains("\"h\""));
    assert!(text.contains("bold"));
    assert!(text.contains("code"));
}
