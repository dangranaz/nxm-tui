//! Mode bar rendering for nexum-terminal TUI.
//! Compact status line with mode, message count, and context usage.

use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AgentMode, App};

/// Render the mode bar status line.
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let mode_style = match app.mode {
        AgentMode::Chat => Style::default().fg(Color::Cyan),
        AgentMode::Architect => Style::default().fg(Color::Magenta),
        AgentMode::Developer => Style::default().fg(Color::Green),
        AgentMode::Researcher => Style::default().fg(Color::Yellow),
    };

    let n = app.messages.len();
    let used = app.estimated_tokens();
    let max_k = app.max_context / 1000;
    let pct = app.context_pct();
    let ctx_color = if pct < 60.0 {
        Color::Rgb(80, 180, 80)
    } else if pct < 85.0 {
        Color::Rgb(200, 180, 60)
    } else {
        Color::Rgb(220, 80, 80)
    };
    let bar_w = 8usize;
    let filled = ((pct / 100.0) * bar_w as f64).round() as usize;
    let bar = "▓".repeat(filled) + &"░".repeat(bar_w.saturating_sub(filled));

    let model_display = app.model_name.as_deref().unwrap_or("—");
    let mut spans = vec![
        Span::styled(" M", mode_style.bold()),
        Span::styled(format!(" {}", app.mode.label()), mode_style),
        Span::styled(
            format!(" {n} msgs"),
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{bar} {:.1}K/{max_k}K]", used as f64 / 1000.0),
            Style::default().fg(ctx_color),
        ),
        Span::raw(" "),
        Span::styled(
            format!("model: {model_display}"),
            Style::default().fg(Color::Rgb(120, 120, 120)),
        ),
    ];

    // Goal corrente (impostato via `update_goal`): inline nella mode_bar,
    // niente overlay/tasto. Solo se Some e non vuoto; troncato a ~40 col.
    if let Some(goal) = app
        .goal_handle
        .lock()
        .ok()
        .and_then(|g| g.as_ref().filter(|s| !s.is_empty()).cloned())
    {
        let goal = if goal.len() > 40 {
            format!("{}…", goal.chars().take(40).collect::<String>())
        } else {
            goal
        };
        spans.push(Span::raw(format!(" 🎯 {goal}")));
    }
    // Indicatore plan mode: mostrato nella mode_bar quando il flag è ON.
    if app
        .plan_mode_handle
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        spans.push(Span::raw(" 🅿️ PLAN"));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
