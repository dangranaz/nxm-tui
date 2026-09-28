//! Overlay render regression net (pi-tui VirtualTerminal pattern).
//!
//! Uses ratatui's `TestBackend` (80x24, no TTY) to prove every overlay
//! renders without panicking and actually shows its content: help, thinking
//! (full + empty), tool detail (full + empty) and approval (with risk badge).

#![allow(dead_code, unused_imports)]


use nxm_tui::app::{App, Message, PendingApproval, Role};
use nxm_tui::overlays;
use nxm_tui::tool_overlay;
use nxm_tui::tool_types::{ToolInvocation, ToolPart};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Render `draw` on an 80x24 headless terminal and return the whole screen
/// as one string (all cell symbols concatenated row by row).
fn screen_text(
    draw: impl FnOnce(&mut ratatui::Frame),
) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal.draw(draw).expect("draw must not panic");
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

#[test]
fn help_overlay_lists_commands_and_overlay_hints() {
    let app = App::new();
    let text = screen_text(|f| overlays::render_help(f, &app));
    assert!(text.contains("Commands"));
    assert!(text.contains("/clear"));
    assert!(text.contains("Ctrl+R"));
    assert!(text.contains("Ctrl+O"));
}

#[test]
fn thinking_overlay_shows_live_reasoning() {
    let mut app = App::new();
    app.append_thinking_delta("considering edge cases; ");
    app.append_thinking_delta("done");
    let text = screen_text(|f| overlays::render_thinking(f, &app));
    assert!(text.contains("Reasoning"));
    assert!(text.contains("considering edge cases"));
}

#[test]
fn thinking_overlay_empty_state_does_not_panic() {
    let app = App::new();
    let text = screen_text(|f| overlays::render_thinking(f, &app));
    assert!(text.contains("No reasoning yet"));
}

#[test]
fn tool_overlay_shows_invocation_and_output() {
    let mut app = App::new();
    app.messages.push(Message::with_tool(
        Role::Assistant,
        vec![ToolPart::ToolInvocation(ToolInvocation {
            id: "call_9".to_string(),
            name: "read_file".to_string(),
            args: "{\"path\": \"/tmp/a.txt\"}".to_string(),
        })],
    ));
    app.messages
        .push(Message::tool("call_9", "file body here".to_string()));
    let text = screen_text(|f| tool_overlay::render_tool(f, &app));
    assert!(text.contains("Tool detail"));
    assert!(text.contains("read_file"));
    assert!(text.contains("file body here"));
}

#[test]
fn tool_overlay_empty_state_does_not_panic() {
    let app = App::new();
    let text = screen_text(|f| tool_overlay::render_tool(f, &app));
    assert!(text.contains("No tool calls yet"));
}

#[test]
fn approval_overlay_shows_prompt_and_risk_badge() {
    let mut app = App::new();
    let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel();
    app.pending_approval = Some(PendingApproval {
        invocation: ToolInvocation {
            id: "call_env".to_string(),
            name: "read_file".to_string(),
            args: "{\"path\": \"/home/u/.env\"}".to_string(),
        },
        reply: reply_tx,
    });
    let text = screen_text(|f| tool_overlay::render_approval(f, &app));
    assert!(text.contains("Allow tool?"));
    assert!(text.contains("read_file"));
    assert!(text.contains("sensitive path"));
}

#[test]
fn approval_overlay_without_pending_renders_nothing() {
    let app = App::new();
    let text = screen_text(|f| tool_overlay::render_approval(f, &app));
    assert!(!text.contains("Allow tool?"));
}
