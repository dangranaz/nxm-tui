//! Chat history rendering for nexum-terminal TUI.
//! Renders user/assistant messages with tool parts support.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Role};
use crate::markdown;

/// Render chat history into the provided area.
pub fn render_history(f: &mut Frame, app: &App, area: Rect) {
    let w = area.width.saturating_sub(4) as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut user_count = 0;

    for msg in &app.messages {
        match msg.role {
            Role::User => {
                user_count += 1;
                for (i, l) in wrap(&msg.content, w).iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled(format!("{user_count}. "), Style::default().fg(Color::Cyan)),
                            Span::styled(l.clone(), Style::default().fg(Color::White)),
                        ]));
                    } else {
                        lines.push(Line::from(Span::raw(format!("   {l}"))));
                    }
                }
                lines.push(Line::from(""));
            }
            Role::Assistant => {
                // Render tool parts if any
                for part in &msg.tool_parts {
                    for line in part.render_lines() {
                        lines.push(line);
                    }
                }
                // Render main content
                for line in markdown::render_markdown(&msg.content, w, "   ") {
                    lines.push(line);
                }
                lines.push(Line::from(""));
                lines.push(Line::from("")); // Extra blank line after response
            }
            Role::System => {
                lines.push(Line::from(Span::styled(
                    format!("   ⚙ {}", msg.content),
                    Style::default().fg(Color::Rgb(180, 180, 100)),
                )));
                lines.push(Line::from(""));
            }
        }
    }

    f.render_widget(Paragraph::new(lines).scroll((app.scroll_offset, 0)), area);
}

/// Wrap text to fit within max width.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if cur.len() + word.len() + 1 > max && !cur.is_empty() {
            result.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        result.push(cur);
    }
    if result.is_empty() {
        result.push(text.to_string());
    }
    result
}