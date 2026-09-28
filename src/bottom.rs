//! Bottom status bar for nexum-terminal TUI.
//! Renders Indexer/Router/Worker status lines.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, ModelRoleInfo};

/// Render the bottom 3 status lines.
pub fn render_bottom_3(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Line 1: Indexer + workspace/git ──
    let cwd = crate::app::current_dir_name();
    let branch = crate::app::git_branch().unwrap_or_default();
    let dir_info = if branch.is_empty() {
        format!("~/{cwd}")
    } else {
        format!("~/{cwd} [{branch}]")
    };

    let indexer_line = role_line(&app.model_indexer, &dir_info, area.width);
    f.render_widget(Paragraph::new(indexer_line), rows[0]);

    // ── Line 2: Router + context ──
    let used = app.estimated_tokens();
    let max_k = app.max_context / 1000;
    let ctx_info = format!("Context: [{:.1}K/{}K]", used as f64 / 1000.0, max_k);
    let router_line = role_line(&app.model_router, &ctx_info, area.width);
    f.render_widget(Paragraph::new(router_line), rows[1]);

    // ── Line 3: Worker + version ──
    let version = format!("Nexum v{}", env!("CARGO_PKG_VERSION"));
    let worker_line = role_line(&app.model_worker, &version, area.width);
    f.render_widget(Paragraph::new(worker_line), rows[2]);
}

/// Create a role status line with left and right aligned text.
pub fn role_line<'a>(info: &'a ModelRoleInfo, right_text: &str, width: u16) -> Line<'a> {
    let status_display = match info.status.as_str() {
        "loaded" | "ready" => "Active",
        "loading" => "Loading",
        "error" => "Error",
        "not loaded" | "unavailable" => "Unavailable",
        _ => "Not Active",
    };
    let status_color = match status_display {
        "Active" => Color::Green,
        "Loading" => Color::Yellow,
        "Error" => Color::Red,
        _ => Color::Rgb(80, 80, 80),
    };

    let left = format!(" Model {}: {} Status: {}", info.role, info.name, status_display);
    let right = format!("{} ", right_text);
    let pad = (width as usize).saturating_sub(left.len() + right.len());

    let spans = vec![
        Span::styled(
            format!(" Model {}: ", info.role),
            Style::default().fg(Color::Rgb(100, 100, 100)),
        ),
        Span::styled(info.name.clone(), Style::default().fg(Color::White)),
        Span::styled(" Status: ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled(status_display.to_string(), Style::default().fg(status_color)),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, Style::default().fg(Color::Rgb(120, 120, 120))),
    ];
    Line::from(spans)
}