//! Legacy V1 markdown renderer — the `USE_NEW_MD` **fallback** (V3/T8).
//!
//! Moved verbatim from `src/markdown.rs` when `render_markdown` became the
//! `USE_NEW_MD` shim (`RULES.md` 300-line cap). Behaviour is unchanged: prose
//! is word-wrapped to `max_width` display columns, code fences are boxed and
//! truncated, `prefix` is prepended to every line. The V3 path lives behind
//! the flag in `src/markdown.rs` + `crate::md`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::highlight::highlight_code;
use crate::markdown_layout::{truncate_to_width, wrap};

/// Legacy renderer: word-wrap prose to `max_width` display columns (CJK/emoji
/// count as two, malformed width as zero), box code fences inside the same
/// budget, and prepend `prefix` to every line.
pub(crate) fn render_markdown_legacy(
    content: &str,
    max_width: usize,
    prefix: &str,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let segments = split_blocks(content);

    for seg in segments {
        match seg {
            Segment::Plain(text) => {
                for wrapped in wrap(&text, max_width.saturating_sub(prefix.width())) {
                    let mut styled = parse_inline(&wrapped);
                    styled.insert(0, Span::raw(String::from(prefix)));
                    lines.push(Line::from(styled));
                }
            }
            Segment::Code(text, lang) => {
                let header = if lang.is_empty() {
                    format!("{prefix}┌─ code ")
                } else {
                    format!("{prefix}┌─ {lang} ")
                };
                lines.push(Line::from(Span::styled(header, code_border_style())));
                let inner_w = max_width.saturating_sub(prefix.width() + 3);
                let known_lang = !lang.is_empty()
                    && crate::highlight::syntax_set()
                        .find_syntax_by_token(&lang)
                        .is_some();
                let single_style = highlight_style(&lang, &text);
                let highlighted = highlight_code(&text, &lang);
                for line_spans in highlighted {
                    let spans: Vec<Span<'static>> = if known_lang {
                        let (mut spans, truncated) =
                            crate::highlight::truncate_spans(line_spans, inner_w);
                        if truncated {
                            spans.push(Span::raw("…"));
                        }
                        spans
                    } else {
                        // Fallback: single color for the whole line (pre-V2 behavior).
                        let joined = crate::highlight::join_spans(&line_spans);
                        let rendered = if joined.width() > inner_w {
                            let head = truncate_to_width(&joined, inner_w.saturating_sub(1));
                            format!("{head}…")
                        } else {
                            joined
                        };
                        vec![Span::styled(rendered, single_style)]
                    };
                    let mut line: Vec<Span<'static>> = vec![Span::raw(format!("{prefix}│ "))];
                    line.extend(spans);
                    lines.push(Line::from(line));
                }
                lines.push(Line::from(Span::styled(
                    format!("{prefix}└─"),
                    code_border_style(),
                )));
            }
        }
    }

    lines
}

enum Segment {
    Plain(String),
    Code(String, String),
}

fn split_blocks(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut remaining = content;

    while let Some(start) = remaining.find("```") {
        let before = &remaining[..start];
        if !before.is_empty() {
            segments.push(Segment::Plain(before.to_string()));
        }
        let after_fence = &remaining[start + 3..];
        let end = after_fence.find("```").unwrap_or(after_fence.len());
        let code_block = &after_fence[..end];
        let (lang, code) = if let Some(nl) = code_block.find('\n') {
            (
                code_block[..nl].trim().to_string(),
                code_block[nl + 1..].to_string(),
            )
        } else {
            (String::new(), code_block.to_string())
        };
        segments.push(Segment::Code(code, lang));
        let next_start = (end + 3).min(after_fence.len());
        remaining = &after_fence[next_start..];
    }

    if !remaining.is_empty() {
        segments.push(Segment::Plain(remaining.to_string()));
    }

    segments
}

fn parse_inline(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some(&(i, c)) = chars.peek() {
        if text[i..].starts_with("**") {
            if let Some(end) = text[i + 2..].find("**") {
                let inner = &text[i + 2..i + 2 + end];
                spans.push(Span::styled(inner.to_string(), bold_style()));
                let advance = i + 2 + end + 2;
                while let Some(&(pos, _)) = chars.peek() {
                    if pos >= advance {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
        }

        if c == '`' {
            if let Some(end) = text[i + 1..].find('`') {
                let inner = &text[i + 1..i + 1 + end];
                spans.push(Span::styled(inner.to_string(), code_style()));
                let advance = i + 1 + end + 1;
                while let Some(&(pos, _)) = chars.peek() {
                    if pos >= advance {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
        }

        if c == '*' {
            if let Some(end) = text[i + 1..].find('*') {
                let inner = &text[i + 1..i + 1 + end];
                spans.push(Span::styled(inner.to_string(), italic_style()));
                let advance = i + 1 + end + 1;
                while let Some(&(pos, _)) = chars.peek() {
                    if pos >= advance {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
        }

        spans.push(Span::raw(c.to_string()));
        chars.next();
    }

    if spans.is_empty() {
        spans.push(Span::raw(text.to_string()));
    }

    spans
}

fn bold_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn italic_style() -> Style {
    Style::default()
        .fg(Color::Rgb(180, 180, 220))
        .add_modifier(Modifier::ITALIC)
}

fn code_style() -> Style {
    Style::default().fg(Color::Rgb(180, 220, 140))
}

/// Get syntax highlighting style for a language.
fn highlight_style(lang: &str, _content: &str) -> Style {
    match lang.to_lowercase().as_str() {
        "rust" => Style::default().fg(Color::Rgb(220, 120, 120)),
        "python" => Style::default().fg(Color::Rgb(220, 180, 120)),
        "javascript" | "js" | "typescript" | "ts" => Style::default().fg(Color::Rgb(120, 200, 220)),
        "go" => Style::default().fg(Color::Rgb(120, 180, 220)),
        "c" | "cpp" => Style::default().fg(Color::Rgb(140, 180, 220)),
        "bash" | "sh" | "shell" => Style::default().fg(Color::Rgb(120, 220, 120)),
        "json" => Style::default().fg(Color::Rgb(220, 220, 120)),
        _ => code_style(),
    }
}

fn code_border_style() -> Style {
    Style::default().fg(Color::Rgb(80, 100, 80))
}
