use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn render_markdown(content: &str, max_width: usize, prefix: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let segments = split_blocks(content);

    for seg in segments {
        match seg {
            Segment::Plain(text) => {
                for wrapped in wrap(&text, max_width.saturating_sub(prefix.len())) {
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
                lines.push(Line::from(Span::styled(
                    header,
                    code_border_style(),
                )));
                let inner_w = max_width.saturating_sub(prefix.len() + 3);
                let style = highlight_style(&lang, &text);
                for line in text.lines() {
                    // Char-safe truncation: a byte slice would panic when
                    // `inner_w` lands inside a multibyte codepoint.
                    let truncated = if line.chars().count() > inner_w {
                        let head: String = line.chars().take(inner_w.saturating_sub(1)).collect();
                        format!("{prefix}│ {head}…")
                    } else {
                        format!("{prefix}│ {line}")
                    };
                    lines.push(Line::from(Span::styled(truncated, style)));
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
            (code_block[..nl].trim().to_string(), code_block[nl + 1..].to_string())
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
                    if pos >= advance { break; }
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
                    if pos >= advance { break; }
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
                    if pos >= advance { break; }
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
    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
}

fn italic_style() -> Style {
    Style::default().fg(Color::Rgb(180, 180, 220)).add_modifier(Modifier::ITALIC)
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

fn wrap(text: &str, max: usize) -> Vec<String> {
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
