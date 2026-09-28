//! Chat history rendering for nexum-terminal TUI.
//! Renders user/assistant messages with tool parts support.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Message, Role};
use crate::markdown;

/// Cached render of one message: a fingerprint key + the lines it contributes
/// to the chat (wrapping, markdown, tool parts, separators included).
pub struct MsgEntry {
    /// `(content.len(), tool_parts.len(), width)` — a mismatch re-renders.
    pub key: (usize, usize, usize),
    pub lines: Vec<Line<'static>>,
}

/// Per-message render cache. Messages are append-only, so the index
/// identifies the message; during streaming only the last message re-renders
/// instead of the whole O(N) history. The key carries the width, so a resize
/// invalidates everything. Regression-tested in `tests/history_cache.rs`.
#[derive(Default)]
pub struct HistoryCache {
    pub entries: Vec<MsgEntry>,
}

impl HistoryCache {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Render chat history into the provided area. Per-message renders are
/// cached in `app.history_cache` (see [`HistoryCache`]) so streaming a token
/// re-renders one message instead of reparsing the full transcript.
pub fn render_history(f: &mut Frame, app: &mut App, area: Rect) {
    let w = area.width.saturating_sub(4) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut user_count = 0usize;

    // Drop cached entries for cleared messages (new session / /clear).
    app.history_cache.entries.truncate(app.messages.len());

    for (idx, msg) in app.messages.iter().enumerate() {
        if msg.role == Role::User {
            user_count += 1;
        }
        let key = (msg.content.len(), msg.tool_parts.len(), w);
        if let Some(entry) = app.history_cache.entries.get(idx) {
            if entry.key == key {
                lines.extend(entry.lines.iter().cloned());
                continue;
            }
        }
        let rendered = render_message(msg, w, user_count);
        let entry = MsgEntry { key, lines: rendered.clone() };
        if idx == app.history_cache.entries.len() {
            app.history_cache.entries.push(entry);
        } else {
            app.history_cache.entries[idx] = entry;
        }
        lines.extend(rendered);
    }

    let offset = App::viewport_offset(lines.len(), area.height as usize, app.scrollback);
    f.render_widget(Paragraph::new(lines).scroll((offset, 0)), area);
}

/// Render one message's contribution to the chat lines (tool parts, wrapping,
/// markdown, separators). `user_count` = number of User messages before this
/// one (drives the "N. " prefix).
fn render_message(msg: &Message, w: usize, user_count: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    match msg.role {
        Role::User => {
            for (i, l) in wrap(&msg.content, w).into_iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(format!("{user_count}. "), Style::default().fg(Color::Cyan)),
                        Span::styled(l, Style::default().fg(Color::White)),
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
                lines.extend(part.render_lines());
            }
            // Render main content
            lines.extend(markdown::render_markdown(&msg.content, w, "   "));
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
        Role::Tool => {
            lines.push(Line::from(vec![
                Span::styled(" ◀ tool ", Style::default().fg(Color::Cyan)),
                Span::styled(msg.content.clone(), Style::default().fg(Color::Rgb(180, 180, 80))),
            ]));
            lines.push(Line::from(""));
        }
    }
    lines
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
