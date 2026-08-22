//! Tool call parts rendering for nexum-terminal TUI.
//! Based on OpenCode tool output rendering.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Tool call part types matching OpenAI API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPart {
    /// Text content part (streaming).
    Text(String),
    /// Reasoning token part.
    Reasoning(String),
    /// Tool call invocation.
    ToolInvocation(ToolInvocation),
    /// Tool call result.
    ToolResult(ToolResult),
    /// Error message.
    Error(String),
}

/// Tool invocation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    pub id: String,
    pub name: String,
    pub args: String,
}

/// Tool result data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
}

impl ToolPart {
    /// Creates a text part.
    pub fn text(content: String) -> Self {
        ToolPart::Text(content)
    }

    /// Creates a reasoning part.
    pub fn reasoning(content: String) -> Self {
        ToolPart::Reasoning(content)
    }

    /// Renders the tool part as lines.
    pub fn render_lines(&self) -> Vec<Line<'static>> {
        match self {
            ToolPart::Text(s) => {
                if s.is_empty() {
                    vec![]
                } else {
                    vec![Line::from(Span::raw(s.clone()))]
                }
            }
            ToolPart::Reasoning(s) => {
                vec![Line::from(vec![
                    Span::styled(" ◆ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(s.clone(), Style::default().fg(Color::DarkGray)),
                ])]
            }
            ToolPart::ToolInvocation(inv) => {
                vec![Line::from(vec![
                    Span::styled(" ▶ ", Style::default().fg(Color::Cyan)),
                    Span::styled(inv.name.clone(), Style::default().fg(Color::Yellow)),
                ])]
            }
            ToolPart::ToolResult(res) => {
                let truncated = if res.output.len() > 100 {
                    format!("{}...", &res.output[..100])
                } else {
                    res.output.clone()
                };
                vec![Line::from(vec![
                    Span::styled(" ◀ ", Style::default().fg(Color::Cyan)),
                    Span::styled(truncated, Style::default().fg(Color::Rgb(180, 180, 80))),
                ])]
            }
            ToolPart::Error(e) => {
                vec![Line::from(Span::styled(
                    format!("Error: {}", e),
                    Style::default().fg(Color::Red),
                ))]
            }
        }
    }
}