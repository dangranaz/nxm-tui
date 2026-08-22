//! Prompt lines rendering for nexum-terminal TUI.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use super::prompt::PromptState;

/// Generate prompt lines for rendering.
pub fn generate(state: &PromptState) -> Vec<Line<'static>> {
    let (prefix, prefix_style) = if state.text().starts_with('/') {
        ("» ", Style::default().fg(Color::Magenta))
    } else {
        ("> ", Style::default().fg(Color::Cyan))
    };

    if state.text().is_empty() {
        vec![Line::from(vec![
            Span::styled(prefix, prefix_style),
            Span::styled(state.placeholder, Style::default().fg(Color::Rgb(60, 60, 60))),
        ])]
    } else {
        state
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_prefix = if i == 0 { prefix } else { "  " };
                Line::from(vec![
                    Span::styled(line_prefix, prefix_style),
                    Span::raw(line.clone()),
                ])
            })
            .collect()
    }
}