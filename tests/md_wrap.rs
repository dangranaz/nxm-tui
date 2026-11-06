//! Behavioural tests for the copied word-wrap (V3/T9b).
//!
//! Derived from xai-grok-pager-render/src/render/wrapping.rs (Apache-2.0).
//! The upstream inline tests rely on `crate::theme::md_style`/`safe_buf`
//! (not ported), so these assert observable behaviour — row widths, counts,
//! style preservation — instead of upstream-exact strings.

use nxm_tui::md::wrap_line::{word_wrap_line, word_wrap_lines};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

#[test]
fn wrap_long_unbreakable_line_respects_width() {
    let line = Line::from("a".repeat(100));
    let wrapped = word_wrap_line(&line, 40);
    assert!(
        wrapped.len() >= 2,
        "expected the line to wrap, got {} rows",
        wrapped.len()
    );
    for row in &wrapped {
        assert!(row.width() <= 40, "row width {} exceeds 40", row.width());
    }
}

#[test]
fn short_line_passes_through_unchanged() {
    let line = Line::from("short");
    let wrapped = word_wrap_line(&line, 40);
    assert_eq!(wrapped.len(), 1, "a fitting line must not wrap");
    assert_eq!(wrapped[0].width(), 5);
    assert_eq!(wrapped[0].spans.len(), 1);
    assert_eq!(wrapped[0].spans[0].content.as_ref(), "short");
}

#[test]
fn word_wrap_lines_wraps_every_input_line() {
    let lines = vec![Line::from("a".repeat(100)), Line::from("b".repeat(100))];
    let wrapped = word_wrap_lines(lines, 40);
    assert!(
        wrapped.len() >= 4,
        "expected >= 4 rows for two 100-cell lines, got {}",
        wrapped.len()
    );
    let mut joined = String::new();
    for row in &wrapped {
        assert!(row.width() <= 40, "row width {} exceeds 40", row.width());
        for span in &row.spans {
            joined.push_str(span.content.as_ref());
        }
    }
    // No whitespace in the input, so nothing is trimmed at the boundaries.
    assert_eq!(joined, format!("{}{}", "a".repeat(100), "b".repeat(100)));
}

#[test]
fn styles_are_preserved_after_wrap() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let line = Line::from(vec![
        Span::styled("B".repeat(30), bold),
        Span::raw("p".repeat(30)),
    ]);
    let wrapped = word_wrap_line(&line, 40);
    assert!(wrapped.len() >= 2, "expected the 60-cell line to wrap");
    let mut bold_spans = 0;
    let mut plain_spans = 0;
    let mut joined = String::new();
    for row in &wrapped {
        assert!(row.width() <= 40, "row width {} exceeds 40", row.width());
        for span in &row.spans {
            joined.push_str(span.content.as_ref());
            if span.style.add_modifier.contains(Modifier::BOLD) {
                bold_spans += 1;
            } else {
                plain_spans += 1;
            }
        }
    }
    assert_eq!(joined, format!("{}{}", "B".repeat(30), "p".repeat(30)));
    assert!(bold_spans >= 1, "bold span lost after wrap");
    assert!(plain_spans >= 1, "plain span lost after wrap");
}

#[test]
fn table_lines_are_never_wrapped() {
    // ASCII borders and box-drawing content rows must stay whole (they would
    // lose column alignment), even when narrower than the content.
    let ascii = Line::from("| col1 | col2 |".to_string());
    assert_eq!(word_wrap_line(&ascii, 5).len(), 1);
    let boxed = Line::from("│ a │ b │".to_string());
    assert_eq!(word_wrap_line(&boxed, 5).len(), 1);
}
