//! Pure search-UI helpers: line matching, scrollback conversion, highlighting.
//!
//! These functions are UI-agnostic and testable; the handler and renderer
//! call them to wire search into the TUI.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::search::TextMatcher;

/// Find indices of lines that match the query.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::search::{QueryKind, TextMatcher};
/// use nxm_tui::search_ui::line_matches;
///
/// let lines = vec![
///     "hello world".to_string(),
///     "foo bar".to_string(),
///     "hello again".to_string(),
/// ];
/// let matcher = TextMatcher::new("hello", QueryKind::Substring);
/// let matches = line_matches(&lines, &matcher);
/// assert_eq!(matches, vec![0, 2]);
/// ```
pub fn line_matches(lines_text: &[String], matcher: &TextMatcher) -> Vec<usize> {
    lines_text
        .iter()
        .enumerate()
        .filter(|(_, text)| matcher.is_match(text))
        .map(|(idx, _)| idx)
        .collect()
}

/// Convert a line index to a scrollback value so the line is visible.
///
/// The viewport shows `height` lines; `scrollback` hides lines below the
/// viewport (0 = tail). To make `line_idx` visible, we need
/// `scrollback = max_scrollback - line_idx` clamped to 0.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::search_ui::scrollback_for_line;
///
/// // 100 lines, 20-line viewport, target line 50
/// // max_scrollback = 80, so scrollback = 80 - 50 = 30
/// assert_eq!(scrollback_for_line(50, 100, 20), 30);
///
/// // Line near the end: scrollback = 0 (already visible)
/// assert_eq!(scrollback_for_line(95, 100, 20), 0);
///
/// // Buffer shorter than viewport: always 0
/// assert_eq!(scrollback_for_line(5, 10, 20), 0);
/// ```
pub fn scrollback_for_line(line_idx: usize, total: usize, height: usize) -> u16 {
    let max_scrollback = total.saturating_sub(height);
    max_scrollback
        .saturating_sub(line_idx)
        .min(u16::MAX as usize) as u16
}

/// Highlight all matches in a line by restyling matching spans.
///
/// Uses the compiled regex to find match ranges and applies a yellow
/// background to matching text. Non-matching spans are kept as-is.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::search::{QueryKind, TextMatcher};
/// use nxm_tui::search_ui::highlight_line;
/// use ratatui::text::{Line, Span};
///
/// let line = Line::from(Span::raw("hello world"));
/// let matcher = TextMatcher::new("world", QueryKind::Substring);
/// let highlighted = highlight_line(&line, &matcher);
/// // The highlighted line should contain "world" with a different style
/// let text: String = highlighted.spans.iter().map(|s| s.content.as_ref()).collect();
/// assert!(text.contains("world"));
/// ```
pub fn highlight_line(line: &Line<'_>, matcher: &TextMatcher) -> Line<'static> {
    let Some(regex) = matcher.compiled_regex() else {
        // Rebuild as owned 'static line
        return Line::from(
            line.spans
                .iter()
                .map(|s| Span::styled(s.content.to_string(), s.style))
                .collect::<Vec<_>>(),
        );
    };

    // Extract full text and span boundaries
    let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    if full_text.is_empty() {
        return Line::from(
            line.spans
                .iter()
                .map(|s| Span::styled(s.content.to_string(), s.style))
                .collect::<Vec<_>>(),
        );
    }

    // Find all match ranges in the full text
    let match_ranges: Vec<(usize, usize)> = regex
        .find_iter(&full_text)
        .map(|m| (m.start(), m.end()))
        .collect();

    if match_ranges.is_empty() {
        return Line::from(
            line.spans
                .iter()
                .map(|s| Span::styled(s.content.to_string(), s.style))
                .collect::<Vec<_>>(),
        );
    }

    // Build new spans with highlighting
    let mut new_spans: Vec<Span> = Vec::new();
    let mut char_pos = 0usize;

    for span in &line.spans {
        let span_text = span.content.as_ref();
        let span_start = char_pos;
        let span_end = char_pos + span_text.len();
        char_pos = span_end;

        // Check if this span overlaps with any match
        let mut last_end = span_start;
        let mut sub_spans: Vec<Span> = Vec::new();

        for &(mstart, mend) in &match_ranges {
            // Match is entirely outside this span
            if mend <= span_start || mstart >= span_end {
                continue;
            }

            // Non-matching part before the match
            if mstart > last_end {
                let before = &full_text[last_end..mstart];
                sub_spans.push(Span::styled(before.to_string(), span.style));
            }

            // Matching part
            let match_start = mstart.max(span_start);
            let match_end = mend.min(span_end);
            let matched = &full_text[match_start..match_end];
            sub_spans.push(Span::styled(
                matched.to_string(),
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ));

            last_end = match_end;
        }

        // Non-matching part after the last match
        if last_end < span_end {
            let after = &full_text[last_end..span_end];
            sub_spans.push(Span::styled(after.to_string(), span.style));
        }

        if sub_spans.is_empty() {
            new_spans.push(Span::styled(span.content.to_string(), span.style));
        } else {
            new_spans.extend(sub_spans);
        }
    }

    Line::from(new_spans)
}
