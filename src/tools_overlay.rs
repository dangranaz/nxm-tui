//! Tools selection overlay for nexum-terminal TUI (T1).
//!
//! Flat checklist of the registry tools with `[x]`/`[ ]` markers (active/
//! inactive) and a cursor on the selected row. No grouping by source yet —
//! that arrives with MCP in T2. Kept in its own module so `overlays.rs`
//! stays under the 300-line RULES cap (same reason as `tool_overlay.rs`).

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// Route one keypress while the Tools overlay is open (T1).
///
/// Modal contract (same as search mode): every key is consumed here, so a
/// keystroke can never leak into the prompt while the overlay is up. Esc
/// closes the overlay and is handled by `handler.rs`'s Esc block before this
/// function runs; Ctrl chords also keep their global meaning upstream.
///
/// ```
/// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use nxm_tui::app::App;
/// use nxm_tui::tools_overlay::handle_tools_key;
///
/// let mut app = App::new();
/// handle_tools_key(&mut app, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
/// assert!(app.tool_registry.tool_names().iter().all(|n| !app.tool_registry.is_enabled(n)));
/// ```
pub fn handle_tools_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => {
            let len = app.tool_registry.tool_names().len();
            if len > 0 {
                app.tools_cursor = if app.tools_cursor == 0 {
                    len - 1
                } else {
                    app.tools_cursor - 1
                };
            }
        }
        KeyCode::Down => {
            let len = app.tool_registry.tool_names().len();
            if len > 0 {
                app.tools_cursor = (app.tools_cursor + 1) % len;
            }
        }
        KeyCode::Char(' ') => {
            if let Some(name) = app
                .tool_registry
                .tool_names()
                .get(app.tools_cursor)
                .map(|s| s.to_string())
            {
                let on = !app.tool_registry.is_enabled(&name);
                app.tool_registry.set_enabled(&name, on);
                app.set_status(format!(
                    "Tool `{name}` {}",
                    if on { "enabled" } else { "disabled" }
                ));
            }
        }
        KeyCode::Char('a') => {
            app.tool_registry.enable_all();
            app.set_status("All tools enabled".into());
        }
        KeyCode::Char('n') => {
            app.tool_registry.disable_all();
            app.set_status("All tools disabled".into());
        }
        _ => {} // swallow: nothing else reaches the prompt while modal
    }
}

/// Render the Tools selection overlay dialog (T1).
///
/// ```
/// use nxm_tui::app::App;
/// use nxm_tui::tools_overlay::render_tools;
/// use ratatui::backend::TestBackend;
/// use ratatui::Terminal;
///
/// let app = App::new();
/// let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
/// terminal.draw(|f| render_tools(f, &app)).unwrap();
/// ```
pub fn render_tools(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = 56.min(area.width);

    let names = app.tool_registry.tool_names();
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Tools",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (i, name) in names.iter().enumerate() {
        let enabled = app.tool_registry.is_enabled(name);
        let cursor = if i == app.tools_cursor { "> " } else { "  " };
        let mark = if enabled { "[x]" } else { "[ ]" };
        lines.push(Line::from(vec![
            Span::styled(cursor, Style::default().fg(Color::Yellow)),
            Span::styled(
                mark,
                Style::default().fg(if enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw(" "),
            Span::raw(*name),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Space", Style::default().fg(Color::Magenta)),
        Span::raw(" toggle  "),
        Span::styled("a", Style::default().fg(Color::Magenta)),
        Span::raw(" all  "),
        Span::styled("n", Style::default().fg(Color::Magenta)),
        Span::raw(" none  "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(" close"),
    ]));

    // Size to content (+2 borders); at least 8 rows when the terminal has
    // room, never taller than the frame (order matters: cap last, or a
    // sub-8-row terminal would get a rect bigger than the frame).
    let h = ((lines.len() + 2) as u16).max(8).min(area.height);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;
    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}
