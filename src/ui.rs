use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Role, RunState};
use crate::sidebar::SidebarInfo;

/// Sidebar threshold width - only show if terminal is wider than this.
const SIDEBAR_THRESHOLD: u16 = 120;

pub fn render(f: &mut Frame, app: &App) {
    match app.state {
        RunState::NoServer => render_no_server(f),
        _ => {
            render_chat(f, app);
            if app.show_help {
                crate::overlays::render_help(f, app);
            }
            if app.show_sessions {
                crate::overlays::render_sessions(f, app);
            }
            // Metrics and context overlays remain in ui.rs (shorter)
            if app.show_metrics {
                render_metrics_overlay(f, app);
            }
            if app.show_context {
                render_context_overlay(f, app);
            }
        }
    }
}

fn render_chat(f: &mut Frame, app: &App) {
    let area = f.area();
    let width = area.width;
    
    // Sidebar visible only if width > threshold
    let show_sidebar = app.sidebar_open && width > SIDEBAR_THRESHOLD;
    
    let chat_area = if show_sidebar {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(crate::sidebar::SIDEBAR_WIDTH), // Sidebar
                Constraint::Min(20), // Chat area
            ])
            .split(area);
        
        // Render sidebar if visible
        let sidebar_area = chunks[0];
        let info = SidebarInfo::from_app(app);
        crate::sidebar::render_sidebar(&info, sidebar_area, f.buffer_mut());
        
        chunks[1] // Chat area
    } else {
        area
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // mode bar (compact)
            Constraint::Min(3),      // chat history
            Constraint::Length(1),   // working indicator
            Constraint::Length(1),   // separator
            Constraint::Length(1),   // input
            Constraint::Length(1),   // separator
            Constraint::Length(3),   // bottom 3 lines: Indexer / Router / Worker
        ])
        .split(chat_area);

    crate::mode_bar::render(f, app, chunks[0]);
    crate::history::render_history(f, app, chunks[1]);
    render_working(f, app, chunks[2]);
    render_separator(f, chunks[3]);
    crate::prompt::render_prompt(&app.prompt_state, chunks[4], f.buffer_mut());
    render_separator(f, chunks[5]);
    crate::bottom::render_bottom_3(f, app, chunks[6]);
}

fn render_no_server(f: &mut Frame) {
    let area = f.area();
    let y = area.height / 3;
    let cfg = crate::config::TuiConfig::load();
    let providers = crate::provider::all_providers(&cfg.providers);

    let mut lines = vec![
        Line::from(Span::styled(
            "Server non attivo",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Nessun server LLM trovato. Scegli un endpoint:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];
    for (i, p) in providers.iter().enumerate() {
        let key = i + 1; // menu keys are 1-based; only 1-9 are selectable
        let mut label = format!("{}   ({})", p.name, p.base_url);
        if p.requires_api_key {
            label.push_str("  [key]");
        }
        lines.push(Line::from(vec![
            Span::styled(format!("  [{key}] "), Style::default().fg(Color::Cyan)),
            Span::raw(label),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  /provider add <nome> <url>  per aggiungere un provider",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "  /server start per avviare il server locale",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "  Ctrl+Q per uscire",
        Style::default().fg(Color::DarkGray),
    )));

    let width = 64u16;
    let block_area = Rect::new(
        area.width.saturating_sub(width) / 2,
        y,
        width.min(area.width),
        lines.len() as u16,
    );
    f.render_widget(Paragraph::new(lines), block_area);
}

fn render_working(f: &mut Frame, app: &App, area: Rect) {
    if !app.is_working() {
        return;
    }
    let braille = ['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];
    let dot = braille[(app.tick as usize / 4) % braille.len()];
    let elapsed = app.operation_elapsed();
    let time = if elapsed > 0 {
        format!(" ({elapsed}s)")
    } else {
        String::new()
    };
    let line = Line::from(vec![
        Span::styled(format!(" {dot} "), Style::default().fg(Color::DarkGray)),
        Span::styled("Thinking...", Style::default().fg(Color::DarkGray)),
        Span::styled(time, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_separator(f: &mut Frame, area: Rect) {
    let s = "─".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            s,
            Style::default().fg(Color::Rgb(80, 80, 80)),
        ))),
        area,
    );
}

fn render_metrics_overlay(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = 58.min(area.width);
    let h = 24;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let m = &app.metrics;
    let session_dur = m.session_start.elapsed();
    let session_secs = session_dur.as_secs();
    let session_min = session_secs / 60;
    let session_sec = session_secs % 60;

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Metrics",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Session",
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )),
        Line::from(vec![
            Span::raw("  Duration:      "),
            Span::styled(format!("{session_min}m {session_sec}s"), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  User messages: "),
            Span::styled(format!("{}", m.session_user_msgs), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Responses:     "),
            Span::styled(format!("{}", m.response_times_ms.len()), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Performance",
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )),
        Line::from(vec![
            Span::raw("  Avg response:  "),
            Span::styled(format!("{}ms", m.avg_response_time_ms()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Avg tokens:    "),
            Span::styled(format!("{}", m.avg_tokens_per_response()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Tokens/sec:    "),
            Span::styled(format!("{:.1}", m.tokens_per_second()), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Cumulative (all sessions)",
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )),
        Line::from(vec![
            Span::raw("  Total tokens:  "),
            Span::styled(format!("{}", m.total_tokens), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Total chars:   "),
            Span::styled(format!("{}", m.total_chars), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Total calls:   "),
            Span::styled(format!("{}", m.total_responses), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Ctrl+M  /m", Style::default().fg(Color::Magenta)),
            Span::raw("  toggle  ·  "),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw("  close"),
        ]),
    ];

    // Add last response times as sparkline-style dots
    if !m.response_times_ms.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Last response times (ms):",
            Style::default().fg(Color::Rgb(120, 120, 120)),
        )));
        let max_t = m.response_times_ms.iter().max().copied().unwrap_or(1).max(1);
        let bar_w = w.saturating_sub(6) as usize;
        for t in m.response_times_ms.iter().rev().take(10).rev() {
            let frac = (*t as f64 / max_t as f64).min(1.0);
            let fill = (frac * bar_w as f64) as usize;
            let bar = "▬".repeat(fill.min(bar_w));
            let color = if *t < 2000 {
                Color::Green
            } else if *t < 8000 {
                Color::Yellow
            } else {
                Color::Red
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{t:>5}"), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(bar, Style::default().fg(color)),
            ]));
        }
    }

    let block = Block::default()
        .title(" Observability Dashboard ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_context_overlay(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = 54.min(area.width);
    let h = if app.messages.len() > 20 { 22 } else { (app.messages.len() + 12).max(14) as u16 };
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let pct = app.context_pct();
    let used = app.estimated_tokens();
    let max_k = app.max_context as f64 / 1000.0;

    let ctx_color = if pct < 60.0 { Color::Green }
    else if pct < 85.0 { Color::Yellow }
    else { Color::Red };

    let bar_w = (w.saturating_sub(6)) as usize;
    let filled = ((pct / 100.0) * bar_w as f64).round() as usize;
    let bar = "█".repeat(filled) + &"░".repeat(bar_w.saturating_sub(filled));

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Context Budget",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(&bar, Style::default().fg(ctx_color)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{:.1}K / {max_k}K tokens  ({:.0}%)", used as f64 / 1000.0, pct),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    // Warning if over 80%
    if pct >= 80.0 {
        lines.push(Line::from(Span::styled(
            format!("  ⚠  High context usage ({pct:.0}%): consider /new or /save"),
            Style::default().fg(Color::Red),
        )));
        lines.push(Line::from(""));
    }

    // Breakdown by message
    lines.push(Line::from(Span::styled(
        "  Per-message estimate:",
        Style::default().fg(Color::Rgb(120, 120, 120)),
    )));
    for (i, msg) in app.messages.iter().enumerate() {
        let t = msg.content.len() / 4 + 4;
        let role_label = match msg.role {
            Role::User => "usr",
            Role::Assistant => "asst",
            Role::System => "sys",
            Role::Tool => "tool",
        };
        let first_line = msg.content.lines().next().unwrap_or("");
        let preview: String = first_line.chars().take(30).collect();
        let preview = if first_line.len() > 30 { format!("{preview}…") } else { preview };
        lines.push(Line::from(vec![
            Span::styled(format!("  {i:>2} "), Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled(role_label, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled(format!("{t:>4}t"), Style::default().fg(Color::Rgb(200, 200, 200))),
            Span::raw("  "),
            Span::styled(
                if preview.is_empty() { "(empty)".into() } else { preview.to_string() },
                Style::default().fg(Color::Rgb(140, 140, 140)),
            ),
        ]));
    }

    // Warning about estimates
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  * estimate: chars/4 + overhead. Actual tokens vary by tokenizer.",
        Style::default().fg(Color::Rgb(80, 80, 80)),
    )));

    // Set context command hint
    lines.push(Line::from(vec![
        Span::styled("  /context <N>", Style::default().fg(Color::Magenta)),
        Span::raw("  set max  ·  "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw("  close"),
    ]));

    let block = Block::default()
        .title(" Context Budget ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}
