//! Todo overlay (MT-3): read-only display of the agent's shared todo list.
//!
//! Renders the todo items from `App.todo_handle` (the same
//! `Arc<Mutex<TodoList>>` the `todo` tool mutates in MT-2) as a checklist
//! with `[x]`/`[ ]` markers. The overlay never mutates the list — only the
//! `todo` tool does. Kept in its own module so `overlays.rs` stays under
//! the 300-line RULES cap (same reason as `tool_overlay.rs`).

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tracing::debug;

use crate::app::App;

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
const TARGET: &str = "nexum::todo::overlay";

/// Route one keypress while the Todo overlay is open (MT-3, read-only).
///
/// Modal contract (same as search mode): every key is consumed here, so a
/// keystroke can never leak into the prompt while the overlay is up. Esc
/// closes the overlay and is handled by `handler.rs`'s Esc block before this
/// function runs.
pub fn handle_todo_key(_app: &mut App, _key: KeyEvent) {
    debug!(target: TARGET, "todo overlay key swallowed (modal)");
    // Swallow every key while the overlay is open; Esc is handled by
    // `handler.rs`'s Esc block before this function runs.
}

/// Render the Todo overlay dialog (MT-3, read-only).
///
/// Shows the shared todo list as `[x]`/`[ ]` checklist rows with a "Todo"
/// title. An empty list shows a "(no todos)" placeholder. The Mutex lock is
/// handled without panic — if poisoned, an error placeholder is shown
/// instead.
///
/// ```
/// use nxm_tui::app::App;
/// use nxm_tui::todo_overlay::render_todo;
/// use ratatui::backend::TestBackend;
/// use ratatui::Terminal;
///
/// let app = App::new();
/// let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
/// terminal.draw(|f| render_todo(f, &app)).unwrap();
/// ```
pub fn render_todo(f: &mut Frame, app: &App) {
    debug!(target: TARGET, "todo overlay render");
    let area = f.area();
    let w = 56.min(area.width);

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Todo",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let list = match app.todo_handle.lock() {
        Ok(g) => g,
        Err(_) => {
            lines.push(Line::from(Span::styled(
                "  (todo list unavailable: mutex poisoned)",
                Style::default().fg(Color::Red),
            )));
            render_lines(f, area, w, lines);
            return;
        }
    };

    let items = list.list();
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no todos)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for item in items {
            let mark = if item.done { "[x]" } else { "[ ]" };
            lines.push(Line::from(vec![
                Span::styled(
                    mark,
                    Style::default().fg(if item.done {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(" "),
                Span::raw(&item.text),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(" close"),
    ]));

    render_lines(f, area, w, lines);
}

/// Common overlay sizing/rendering: draw `lines` centered in `area` with
/// width `w`, capped to the frame height.
fn render_lines(f: &mut Frame, area: Rect, w: u16, lines: Vec<Line>) {
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
