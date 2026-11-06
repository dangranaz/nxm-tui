//! Tests for search_ui pure functions: line_matches, scrollback_for_line,
//! and highlight_line.

use nxm_tui::search::{QueryKind, TextMatcher};
use nxm_tui::search_ui::{highlight_line, line_matches, scrollback_for_line};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

// ── line_matches ────────────────────────────────────────────────────────────

#[test]
fn line_matches_basic() {
    let lines = vec![
        "hello world".to_string(),
        "foo bar".to_string(),
        "hello again".to_string(),
    ];
    let matcher = TextMatcher::new("hello", QueryKind::Substring);
    let matches = line_matches(&lines, &matcher);
    assert_eq!(matches, vec![0, 2]);
}

#[test]
fn line_matches_case_insensitive() {
    let lines = vec![
        "Hello World".to_string(),
        "FOO BAR".to_string(),
        "hello again".to_string(),
    ];
    let matcher = TextMatcher::new("hello", QueryKind::Substring);
    let matches = line_matches(&lines, &matcher);
    assert_eq!(matches, vec![0, 2]);
}

#[test]
fn line_matches_no_matches() {
    let lines = vec!["apple".to_string(), "banana".to_string()];
    let matcher = TextMatcher::new("orange", QueryKind::Substring);
    let matches = line_matches(&lines, &matcher);
    assert!(matches.is_empty());
}

#[test]
fn line_matches_empty_input() {
    let lines: Vec<String> = vec![];
    let matcher = TextMatcher::new("test", QueryKind::Substring);
    let matches = line_matches(&lines, &matcher);
    assert!(matches.is_empty());
}

#[test]
fn line_matches_regex() {
    let lines = vec![
        "error: something failed".to_string(),
        "info: all good".to_string(),
        "error: another failure".to_string(),
    ];
    let matcher = TextMatcher::new(r"error: \w+", QueryKind::Regex);
    let matches = line_matches(&lines, &matcher);
    assert_eq!(matches, vec![0, 2]);
}

#[test]
fn line_matches_bad_regex_no_panic() {
    let lines = vec!["test".to_string()];
    let matcher = TextMatcher::new("[invalid", QueryKind::Regex);
    assert!(matcher.is_error());
    let matches = line_matches(&lines, &matcher);
    assert!(matches.is_empty());
}

// ── scrollback_for_line ─────────────────────────────────────────────────────

#[test]
fn scrollback_for_line_middle() {
    // 100 lines, 20-line viewport, target line 50
    // max_scrollback = 80, so scrollback = 80 - 50 = 30
    assert_eq!(scrollback_for_line(50, 100, 20), 30);
}

#[test]
fn scrollback_for_line_near_end() {
    // Line near the end: scrollback = 0 (already visible)
    assert_eq!(scrollback_for_line(95, 100, 20), 0);
}

#[test]
fn scrollback_for_line_buffer_shorter_than_viewport() {
    // Buffer shorter than viewport: always 0
    assert_eq!(scrollback_for_line(5, 10, 20), 0);
}

#[test]
fn scrollback_for_line_first_line() {
    // First line: max scrollback
    assert_eq!(scrollback_for_line(0, 100, 20), 80);
}

#[test]
fn scrollback_for_line_last_visible() {
    // Last line that requires scrolling: line 79 (max_scrollback = 80)
    assert_eq!(scrollback_for_line(79, 100, 20), 1);
}

#[test]
fn scrollback_for_line_beyond_max() {
    // Line beyond max_scrollback: clamped to 0
    assert_eq!(scrollback_for_line(85, 100, 20), 0);
}

// ── highlight_line ──────────────────────────────────────────────────────────

#[test]
fn highlight_line_basic() {
    let line = Line::from(Span::raw("hello world"));
    let matcher = TextMatcher::new("world", QueryKind::Substring);
    let highlighted = highlight_line(&line, &matcher);

    // Should contain "world" with yellow background
    let has_highlight = highlighted
        .spans
        .iter()
        .any(|s| s.content.as_ref() == "world" && s.style.bg == Some(Color::Yellow));
    assert!(has_highlight, "Expected highlighted span with yellow bg");
}

#[test]
fn highlight_line_no_match() {
    let line = Line::from(Span::raw("hello world"));
    let matcher = TextMatcher::new("xyz", QueryKind::Substring);
    let highlighted = highlight_line(&line, &matcher);

    // No highlight: should be unchanged
    assert_eq!(highlighted.spans.len(), 1);
    assert_eq!(highlighted.spans[0].content.as_ref(), "hello world");
}

#[test]
fn highlight_line_multiple_spans() {
    let line = Line::from(vec![
        Span::styled("hello ", Style::default().fg(Color::Cyan)),
        Span::styled("world", Style::default().fg(Color::White)),
    ]);
    let matcher = TextMatcher::new("world", QueryKind::Substring);
    let highlighted = highlight_line(&line, &matcher);

    // Should have highlighted "world"
    let has_highlight = highlighted
        .spans
        .iter()
        .any(|s| s.content.as_ref() == "world" && s.style.bg == Some(Color::Yellow));
    assert!(has_highlight);
}

#[test]
fn highlight_line_bad_regex_no_panic() {
    let line = Line::from(Span::raw("test"));
    let matcher = TextMatcher::new("[invalid", QueryKind::Regex);
    assert!(matcher.is_error());
    let highlighted = highlight_line(&line, &matcher);
    // Should return unchanged line
    assert_eq!(highlighted.spans.len(), 1);
    assert_eq!(highlighted.spans[0].content.as_ref(), "test");
}

#[test]
fn highlight_line_empty_line() {
    let line = Line::from("");
    let matcher = TextMatcher::new("test", QueryKind::Substring);
    let highlighted = highlight_line(&line, &matcher);
    // Empty line has no content to highlight
    assert!(highlighted.spans.is_empty() || highlighted.spans.iter().all(|s| s.content.is_empty()));
}

// ── Navigation (n/N) ────────────────────────────────────────────────────────

#[test]
fn navigation_forward_wraps() {
    // matches = [10, 50, 100], current = 2 (last)
    // forward: (2 + 1) % 3 = 0
    let matches = [10usize, 50, 100];
    let len = matches.len();
    let mut current = 2usize;
    current = (current + 1) % len;
    assert_eq!(current, 0);
    assert_eq!(matches[current], 10);
}

#[test]
fn navigation_backward_wraps() {
    // matches = [10, 50, 100], current = 0 (first)
    // backward: (0 + 3 - 1) % 3 = 2
    let matches = [10usize, 50, 100];
    let len = matches.len();
    let mut current = 0usize;
    current = (current + len - 1) % len;
    assert_eq!(current, 2);
    assert_eq!(matches[current], 100);
}

#[test]
fn navigation_forward_middle() {
    // matches = [10, 50, 100], current = 0
    // forward: (0 + 1) % 3 = 1
    let matches = [10usize, 50, 100];
    let len = matches.len();
    let mut current = 0usize;
    current = (current + 1) % len;
    assert_eq!(current, 1);
    assert_eq!(matches[current], 50);
}

#[test]
fn navigation_scrollback_jumps_to_match() {
    // 200 rendered lines, 20-line viewport, match at line 150
    // max_scrollback = 200 - 20 = 180
    // scrollback = 180 - 150 = 30
    let matches = [10usize, 50, 150];
    let current = 2usize;
    let line_idx = matches[current];
    let scrollback = scrollback_for_line(line_idx, 200, 20);
    assert_eq!(scrollback, 30);
    // Verify: with scrollback=30, viewport_offset = max(180-30, 0) = 150
    // So line 150 is at the top of the viewport (visible)
    let offset = 180u16.saturating_sub(scrollback.min(180));
    assert_eq!(offset, 150);
}
