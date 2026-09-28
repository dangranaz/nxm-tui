//! History render cache (T3, `wayfinder:task`).
//!
//! The cache is behavior-preserving: cached rendering must be
//! indistinguishable from fresh rendering — across repeated frames,
//! streaming tokens and resize. A stale cache keeps showing old content;
//! these tests catch exactly that (output differential, not internals).

#![allow(dead_code, unused_imports)]

use nxm_tui::app::{App, Message, Role};
use nxm_tui::history::render_history;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Render the chat history into a headless terminal (no TTY) and return the
/// whole screen as one string (all cell symbols concatenated row by row).
/// Each call uses a fresh `TestBackend`; the render cache lives in `app` and
/// persists across calls.
fn screen_text(app: &mut App, width: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render_history(f, app, Rect::new(0, 0, width, 24)))
        .expect("draw must not panic");
    let buf = terminal.backend().buffer().clone();
    let (w, h) = (buf.area.width as usize, buf.area.height as usize);
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            if let Some(cell) = buf.cell((x as u16, y as u16)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

/// A conversation exercising every role, markdown inline styling, code fences
/// and tool output.
fn sample_app() -> App {
    let mut app = App::new();
    app.messages
        .push(Message::new(Role::User, "hello **world** this is a question".into()));
    app.messages.push(Message::new(
        Role::Assistant,
        "Answer with `code` inline.\n\n```rust\nfn main() {}\n```".into(),
    ));
    app.messages.push(Message::new(Role::System, "server ready".into()));
    app.messages
        .push(Message::tool("call-1", "tool output line".into()));
    app
}

#[test]
fn repeated_renders_are_identical() {
    let mut app = sample_app();
    let first = screen_text(&mut app, 60);
    let second = screen_text(&mut app, 60);
    assert_eq!(
        first, second,
        "warm cache must not change the rendered output"
    );
}

#[test]
fn streaming_token_invalidates_cache() {
    let mut app = App::new();
    app.push_token("hello");
    let before = screen_text(&mut app, 60);
    assert!(before.contains("hello"));
    app.push_token(" world");
    let after = screen_text(&mut app, 60);
    assert!(
        after.contains("hello world"),
        "a new token must appear on screen — a stale cache would keep \
         showing only 'hello'"
    );
}

#[test]
fn resize_rerenders_at_new_width() {
    let long =
        "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll".to_string();

    // Warm the cache at width 60, then "resize" down to 20.
    let mut warm = App::new();
    warm.messages.push(Message::new(Role::User, long.clone()));
    let _wide = screen_text(&mut warm, 60);
    let resized = screen_text(&mut warm, 20);

    // Independent oracle: a fresh app (empty cache) rendered at width 20.
    let mut oracle = App::new();
    oracle.messages.push(Message::new(Role::User, long));
    let expected = screen_text(&mut oracle, 20);

    assert_eq!(
        resized, expected,
        "after a resize the cache must re-render at the new width — \
         stale width-60 lines would differ from the fresh render"
    );
}
