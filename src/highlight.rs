//! Per-token syntax highlighting for code fences via `syntect`.
//!
//! Extracted from `src/markdown.rs` (V2): `highlight_code` turns a snippet +
//! language token into per-span `ratatui::style::Style` runs. The default
//! `SyntaxSet`/`ThemeSet` are loaded once (`OnceLock`), never per call. Empty
//! or unknown language → plain-text fallback (one span per line, default
//! style): never panics, never breaks the render.

use std::sync::OnceLock;

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Highlight `code` for language `lang`, returning one `Vec` of spans per
/// line. Each span is a `(Style, text)` pair ready for ratatui.
///
/// Uses the default `SyntaxSet`/`ThemeSet` loaded exactly once (`OnceLock`).
/// Falls back to plain text (single span per line, default style) when `lang`
/// is empty or unknown, or when no theme is available — never panics.
///
/// # Examples
///
/// ```text
/// use nxm_tui::highlight::highlight_code;
///
/// let lines = highlight_code("fn main() {}", "rust");
/// assert!(!lines.is_empty());
/// let has_multi = lines.iter().any(|line| {
///     line.len() >= 2 && line.windows(2).any(|w| w[0].0 != w[1].0)
/// });
/// assert!(has_multi, "keyword vs identifier should yield distinct styles");
/// ```
pub fn highlight_code(code: &str, lang: &str) -> Vec<Vec<(Style, String)>> {
    let syntax_set = syntax_set();
    let theme_set = theme_set();

    let syntax = if lang.is_empty() {
        None
    } else {
        syntax_set.find_syntax_by_token(lang)
    };

    let Some(syntax) = syntax else {
        tracing::debug!(target: "nexum::highlight", "unknown lang {lang:?}: plain-text fallback");
        return plain_text(code);
    };

    // Pick a dark theme with a light foreground for good contrast on a dark
    // TUI; fall back to any available theme, then to plain text.
    let theme = theme_set
        .themes
        .get("base16-eighties.dark")
        .or_else(|| theme_set.themes.values().next());
    let Some(theme) = theme else {
        tracing::debug!(target: "nexum::highlight", "no theme available: plain-text fallback");
        return plain_text(code);
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut out: Vec<Vec<(Style, String)>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let mut spans: Vec<(Style, String)> = Vec::new();
        if let Ok(regions) = highlighter.highlight_line(line, syntax_set) {
            for (style, text) in regions {
                // `LinesWithEndings` keeps the trailing `\n` on each line; strip
                // it from the span text so ratatui does not render a spurious
                // extra blank row per code line.
                let text = text.strip_suffix('\n').unwrap_or(text);
                if text.is_empty() {
                    continue;
                }
                let color = style.foreground;
                let style = Style::default().fg(Color::Rgb(color.r, color.g, color.b));
                spans.push((style, text.to_string()));
            }
        }
        if spans.is_empty() {
            spans.push((Style::default(), String::new()));
        }
        out.push(spans);
    }

    out
}

/// Plain-text fallback: one span per line, default style, content preserved.
///
/// A single trailing newline is dropped (it would otherwise yield a spurious
/// empty last row, matching the highlighted path); interior blank lines are
/// kept.
fn plain_text(code: &str) -> Vec<Vec<(Style, String)>> {
    let code = code.strip_suffix('\n').unwrap_or(code);
    code.split('\n')
        .map(|line| vec![(Style::default(), line.to_string())])
        .collect()
}

/// Default `SyntaxSet`, loaded exactly once.
pub fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Default `ThemeSet`, loaded exactly once.
pub fn theme_set() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

/// Join the text of all spans into a single string.
pub fn join_spans(spans: &[(Style, String)]) -> String {
    spans.iter().map(|(_, t)| t.as_str()).collect()
}

/// Truncate a list of `(Style, text)` spans to at most `max` display cells,
/// never splitting a char. Returns the kept spans and whether truncation
/// occurred (caller appends `…`).
pub fn truncate_spans(spans: Vec<(Style, String)>, max: usize) -> (Vec<Span<'static>>, bool) {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut width = 0usize;
    let mut truncated = false;
    for (style, text) in spans {
        if width >= max {
            truncated = true;
            break;
        }
        let remaining = max - width;
        let twidth = text.width();
        if twidth <= remaining {
            out.push(Span::styled(text, style));
            width += twidth;
        } else {
            let mut taken = String::new();
            let mut taken_width = 0;
            for ch in text.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if taken_width + cw > remaining {
                    break;
                }
                taken.push(ch);
                taken_width += cw;
            }
            out.push(Span::styled(taken, style));
            truncated = true;
            break;
        }
    }
    (out, truncated)
}
