//! Overlay dialogs for nexum-terminal TUI.
//! Help, sessions, metrics, and context overlays.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// Render help overlay dialog.
pub fn render_help(f: &mut Frame, _app: &App) {
    let area = f.area();
    let w = 56.min(area.width);
    let h = 24;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let items = vec![
        Line::from(Span::styled(" Commands", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  /help  /h  /?", Style::default().fg(Color::Magenta)),
            Span::raw("  Show this help"),
        ]),
        Line::from(vec![
            Span::styled("  /clear", Style::default().fg(Color::Magenta)),
            Span::raw("        Clear chat history"),
        ]),
        Line::from(vec![
            Span::styled("  /quit  /q", Style::default().fg(Color::Magenta)),
            Span::raw("        Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Server", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  /server start", Style::default().fg(Color::Magenta)),
            Span::raw("   Start local server"),
        ]),
        Line::from(vec![
            Span::styled("  /server stop", Style::default().fg(Color::Magenta)),
            Span::raw("    Stop local server"),
        ]),
        Line::from(vec![
            Span::styled("  /server status", Style::default().fg(Color::Magenta)),
            Span::raw("  Server status"),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Modes", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  /mode chat|arch|dev|research", Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Sessions", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  /save [name]  /load <name>  /new", Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Keys", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Ctrl+Q", Style::default().fg(Color::DarkGray)),
            Span::raw("  Quit   "),
            Span::styled("Ctrl+S", Style::default().fg(Color::DarkGray)),
            Span::raw("  Save   "),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw("  Cancel"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(items), inner);
}

/// Render sessions overlay dialog.
pub fn render_sessions(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = 60.min(area.width);
    let h = 16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let sessions = crate::app::list_sessions();
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Sessions",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved sessions",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for s in &sessions {
            let is_active = app.session_name.as_ref().map(|n| n == s).unwrap_or(false);
            
            // Get preview from session file
            let preview = get_session_preview(s).unwrap_or_default();
            
            if is_active {
                lines.push(Line::from(vec![
                    Span::styled("  ▶ ", Style::default().fg(Color::Green)),
                    Span::styled(s.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(
                        if preview.len() > 25 { format!("{}...", &preview[..25]) } else { preview },
                        Style::default().fg(Color::Rgb(120, 120, 120)),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::raw(s),
                    Span::raw(" "),
                    Span::styled(
                        if preview.len() > 20 { format!("{}...", &preview[..20]) } else { preview },
                        Style::default().fg(Color::Rgb(80, 80, 80)),
                    ),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  /load <name>", Style::default().fg(Color::Magenta)),
        Span::raw("  or  "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw(" to close"),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}

/// Get preview text from a session file.
fn get_session_preview(name: &str) -> Option<String> {
    let path = crate::app::session_dir().join(format!("{name}.json"));
    let content = std::fs::read_to_string(&path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    
    let messages = data["messages"].as_array()?;
    for m in messages {
        let role = m["role"].as_str()?;
        if role == "user" {
            return m["content"].as_str().map(|s| {
                s.chars().take(30).collect::<String>()
            });
        }
    }
    None
}