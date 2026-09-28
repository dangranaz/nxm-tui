//! Tool detail overlay for nexum-terminal TUI.
//!
//! Read-only inspector for the most recent tool exchange (kimi
//! `approval-preview` pattern, without gating yet): invocation name, call id,
//! full args and full output. Toggle with Ctrl+O, close with Esc.
//! Kept in its own module so `overlays.rs` stays under the 300-line RULES cap.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Message, Role};
use crate::tool_types::ToolInvocation;

/// Most recent tool invocation plus its result output if present.
///
/// Scans `messages` from the tail for a `ToolInvocation`, then looks up the
/// `role:"tool"` message carrying the same call id.
pub fn last_tool_exchange(messages: &[Message]) -> Option<(ToolInvocation, Option<String>)> {
    let inv = messages.iter().rev().find_map(|m| {
        m.tool_parts.iter().find_map(|p| match p {
            crate::tool_types::ToolPart::ToolInvocation(i) => Some(i.clone()),
            _ => None,
        })
    })?;
    let output = messages.iter().rev().find_map(|m| {
        if m.role == Role::Tool && m.tool_call_id.as_deref() == Some(inv.id.as_str()) {
            Some(m.content.clone())
        } else {
            None
        }
    });
    Some((inv, output))
}

/// Fit one logical line to `max` terminal cells (char-safe).
fn fit(s: &str, max: usize) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let mut out: String = flat.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Render the tool detail overlay dialog.
pub fn render_tool(f: &mut Frame, app: &App) {
    let area = f.area();
    let w = 72.min(area.width);
    let h = 20.min(area.height.saturating_sub(4)).max(8);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let inner_w = w.saturating_sub(4) as usize;
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Tool detail",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    match last_tool_exchange(&app.messages) {
        None => lines.push(Line::from(Span::styled(
            "  No tool calls yet — they appear here after the agent uses a tool.",
            Style::default().fg(Color::DarkGray),
        ))),
        Some((inv, output)) => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(inv.name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("  id:{}", inv.id),
                    Style::default().fg(Color::Rgb(120, 120, 120)),
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Args",
                Style::default().fg(Color::Rgb(150, 150, 150)),
            )));
            if inv.args.trim().is_empty() {
                lines.push(Line::from(Span::styled(
                    "    (none)",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for raw in inv.args.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(fit(raw, inner_w.saturating_sub(4).max(10)), Style::default().fg(Color::White)),
                    ]));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Output",
                Style::default().fg(Color::Rgb(150, 150, 150)),
            )));
            match output {
                None => lines.push(Line::from(Span::styled(
                    "    (pending — result not yet received)",
                    Style::default().fg(Color::DarkGray),
                ))),
                Some(out) => {
                    if out.trim().is_empty() {
                        lines.push(Line::from(Span::styled(
                            "    (empty)",
                            Style::default().fg(Color::DarkGray),
                        )));
                    } else {
                        for raw in out.lines().take(6) {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(
                                    fit(raw, inner_w.saturating_sub(4).max(10)),
                                    Style::default().fg(Color::Rgb(180, 180, 80)),
                                ),
                            ]));
                        }
                        if out.lines().count() > 6 {
                            lines.push(Line::from(Span::styled(
                                "    … (truncated, see history)",
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                }
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Ctrl+O", Style::default().fg(Color::Magenta)),
        Span::raw("  toggle  ·  "),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::raw("  close"),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}

/// Render the modal tool approval prompt (Y/N).
///
/// Shown while `App::pending_approval` is set: the agent task is parked on
/// its oneshot reply until the user decides. Rendered last so it sits above
/// every other overlay.
pub fn render_approval(f: &mut Frame, app: &App) {
    let Some(pending) = &app.pending_approval else {
        return;
    };
    let area = f.area();
    let w = 68.min(area.width);
    let h = 14.min(area.height.saturating_sub(4)).max(8);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 3;

    let overlay = Rect::new(x, y, w, h);
    f.render_widget(Clear, overlay);

    let inner_w = w.saturating_sub(4) as usize;
    let badge = crate::tool_types::risk_badge(&pending.invocation);
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            " Allow tool?",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                pending.invocation.name.clone(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  id:{}", pending.invocation.id),
                Style::default().fg(Color::Rgb(120, 120, 120)),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                fit(&pending.invocation.args, inner_w.saturating_sub(2).max(10)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(badge) = badge {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                badge.to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.extend(vec![
        Line::from(Span::styled(
            "  The agent is waiting — nothing else runs until you decide.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  approve   "),
            Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("  deny   "),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw("  deny"),
        ]),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(block, overlay);

    let inner = Rect::new(x + 1, y + 1, w.saturating_sub(2), h.saturating_sub(2));
    f.render_widget(Paragraph::new(lines), inner);
}
