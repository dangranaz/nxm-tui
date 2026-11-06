//! Markdown renderer: cell-width aware wrapping and truncation (V1).
//!
//! The renderer must lay text out in **display columns** — a CJK ideograph or
//! an emoji occupies two cells, not one, and not three bytes. These tests drive
//! the public `render_markdown` entry point (the interface `history.rs` uses)
//! and assert on the produced lines' rendered width, including the prefix.

use nxm_tui::markdown::render_markdown;
use unicode_width::UnicodeWidthStr;

/// Render `content` and return each line's full text (prefix included).
fn rendered(content: &str, width: usize) -> Vec<String> {
    render_markdown(content, width, PREFIX)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

const PREFIX: &str = "   ";

fn assert_fits(lines: &[String], width: usize) {
    for line in lines {
        assert!(
            line.width() <= width,
            "line {:?} is {} columns wide, pane is {width}",
            line,
            line.width()
        );
    }
}

#[test]
fn wide_glyphs_wrap_by_cell_width_not_bytes() {
    // Two CJK words: 6 + 1 + 6 = 13 display columns, but 19 bytes. A byte-based
    // wrap would split them onto two lines; the pane (0..=15) fits both.
    let lines = rendered("日本語 テスト", 18);
    assert_eq!(lines.len(), 1, "wide words must share a row: {lines:?}");
    assert_fits(&lines, 18);
    assert_eq!(lines[0].width(), 16);
}

#[test]
fn long_unbroken_word_is_split_to_fit_the_pane() {
    let word = "https://example.com/very/long/path/segment?q=1";
    let lines = rendered(word, 16);
    assert!(lines.len() > 1, "long word must be split: {lines:?}");
    assert_fits(&lines, 16);

    // Nothing is dropped: chunks reassemble into the original word.
    let mut reassembled = String::new();
    for line in &lines {
        reassembled.push_str(&line[PREFIX.len()..]);
    }
    assert_eq!(reassembled, word);
}

#[test]
fn code_fence_truncation_counts_display_width() {
    let emoji = "😀".repeat(30);
    let content = format!("```\n{emoji}\n```");
    let lines = rendered(&content, 20);
    assert_fits(&lines, 20);
    let body = lines
        .iter()
        .find(|line| line.contains('│'))
        .expect("code fence must render a body line");
    assert!(body.contains('…'), "truncation marker missing: {body:?}");
}

#[test]
fn multibyte_fence_never_panics_and_stays_inside_the_pane() {
    // Mixed widths: a byte slice at `inner_w` would land mid-codepoint.
    let content = "```rust\nlet s = \"héllo wörld 日本語 😀😀\";\n```";
    let lines = rendered(content, 12);
    assert_fits(&lines, 12);
    assert!(lines.iter().any(|line| line.contains('│')));
}

#[test]
fn plain_ascii_prose_and_inline_spans_are_preserved() {
    // Inline markup is consumed into styled spans, so the assembled text is
    // the plain rendering of it — one unwrapped row for a short sentence.
    let lines = rendered("hello **bold** and `code`", 40);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].strip_prefix(PREFIX), Some("hello bold and code"));
}

#[test]
fn zero_width_budget_does_not_panic() {
    // Pane narrower than the prefix: no prose line can fit, but rendering the
    // (already degenerate) fence border must not panic either.
    let lines = rendered("hi 日本語", 3);
    assert!(lines.is_empty(), "nothing fits the pane: {lines:?}");
    let _ = rendered("```\ncode\n```", 3);
}

#[test]
fn code_fence_line_wider_than_pane_is_truncated_to_cell_width() {
    // A fence body line wider than the pane must be cell-width truncated and
    // end with the ellipsis marker; the whole rendered line (prefix included)
    // must stay inside the pane budget.
    let long = "x".repeat(50);
    let content = format!("```\n{long}\n```");
    let lines = rendered(&content, 20);
    assert_fits(&lines, 20);
    let body = lines
        .iter()
        .find(|line| line.contains('│'))
        .expect("code fence must render a body line");
    assert!(body.contains('…'), "truncation marker missing: {body:?}");
    assert!(
        body.width() <= 20,
        "body line {body:?} exceeds pane width 20"
    );
}
