//! T9a contract tests (`src/md/line_utils.rs`).
//!
//! Adapted from xai-grok-pager-render/src/render/line_utils.rs inline tests
//! (xai-org/grok-build, Apache-2.0) — pure `Line`/`str` utilities, renderer
//! free, so they port directly. Expectations for `truncate_str`/
//! `cascade_truncate` reflect the ellipsis-free backing
//! (`crate::markdown_layout::truncate_to_width`), documented in the source
//! header's `Modified:` clause (upstream truncation appended `…`); the
//! upstream test for the un-ported `tool_paths` re-exports was dropped.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use nxm_tui::md::line_utils::{
    cascade_truncate, fit_line_to_width, line_to_static, push_owned_lines, truncate_line,
    truncate_str,
};

/// Concatenate the text of every span in the line.
fn line_text(line: &Line<'static>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

// ── truncate_str ─────────────────────────────────────────────

/// Upstream contract: when it does not fit, `truncate_str` cuts to `max_width-1`
/// cells and appends `…` (so the result is `max_width` cells). `日本語テスト`
/// (12 cells) at width 4 → `日` (2) + `…` (1) = `日…`.
#[test]
fn truncate_str_cjk_appends_ellipsis() {
    assert_eq!(truncate_str("日本語テスト", 4), "日…");
}

/// Fits unchanged; over-wide cuts to `max_width-1` + `…`.
#[test]
fn truncate_str_ascii_budget() {
    assert_eq!(truncate_str("hello", 10), "hello");
    assert_eq!(truncate_str("hello", 3), "he…");
}

// ── truncate_line ────────────────────────────────────────────

#[test]
fn truncate_line_fits() {
    let line = Line::from(vec![Span::raw("Hello "), Span::raw("world")]);
    let result = truncate_line(line, 20);
    assert_eq!(result.spans.len(), 2);
    let [a, b] = result.spans.as_slice() else {
        panic!("expected two spans: {:?}", result.spans);
    };
    assert_eq!(a.content.as_ref(), "Hello ");
    assert_eq!(b.content.as_ref(), "world");
}

#[test]
fn truncate_line_cuts_mid_span() {
    let line = Line::from(vec![
        Span::raw("Edit "),
        Span::raw("very/long/path/to/file.rs"),
    ]);
    // Truncating to 15 cuts inside the second span and yields "Edit very/long…"
    let result = truncate_line(line, 15);
    let text: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.ends_with('\u{2026}'));
    assert!(text.width() <= 15);
}

#[test]
fn truncate_line_drops_later_spans() {
    let line = Line::from(vec![
        Span::raw("Search "),
        Span::raw("pattern"),
        Span::raw(" in "),
        Span::raw("path"),
        Span::raw(" (5 matches)"),
    ]);
    let result = truncate_line(line, 18);
    let text: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.ends_with('\u{2026}'));
    assert!(text.width() <= 18);
}

#[test]
fn truncate_line_zero_width() {
    let line = Line::from(vec![Span::raw("hello")]);
    let result = truncate_line(line, 0);
    assert!(result.spans.is_empty());
}

/// Styles survive on every span kept up to the cut; the ellipsis inherits the
/// last kept span's style.
#[test]
fn truncate_line_preserves_styles_up_to_the_cut() {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let line = Line::from(vec![
        Span::raw("head "),
        Span::styled("verylongstyledtoken", bold),
        Span::styled(" tail", Style::new().fg(ratatui::style::Color::Green)),
    ]);
    let result = truncate_line(line, 12);
    let text = line_text(&result);
    assert!(text.ends_with('\u{2026}'));
    assert!(text.width() <= 12);
    // Part of the styled token survived AND kept its style.
    let kept_bold = result
        .spans
        .iter()
        .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
    assert!(kept_bold, "styled span up to the cut was dropped");
    let ellipsis = result.spans.last().expect("ellipsis span");
    assert_eq!(ellipsis.content.as_ref(), "\u{2026}");
    assert!(ellipsis.style.add_modifier.contains(Modifier::BOLD));
}

// ── fit_line_to_width ────────────────────────────────────────

#[test]
fn fit_line_pads_short_line() {
    let line = Line::from(vec![Span::raw("│ a │")]);
    let out = fit_line_to_width(line, 10);
    assert_eq!(line_text(&out).width(), 10);
    assert_eq!(line_text(&out), "│ a │     ");
}

#[test]
fn fit_line_exact_width_unchanged() {
    let line = Line::from(vec![Span::raw("hello")]);
    let out = fit_line_to_width(line, 5);
    assert_eq!(out.spans.len(), 1);
    assert_eq!(line_text(&out), "hello");
}

#[test]
fn fit_line_clips_long_line_no_ellipsis() {
    let line = Line::from(vec![Span::raw("│ Column A │ Column B │")]);
    let out = fit_line_to_width(line, 8);
    assert_eq!(line_text(&out).width(), 8);
    assert_eq!(line_text(&out), "│ Column");
}

#[test]
fn fit_line_does_not_split_emoji_grapheme() {
    // The line is 4 columns wide: a and b, then the width-2 ⚠️ grapheme
    // Clipping to 3 drops that grapheme whole and pads with one space
    let line = Line::from(vec![Span::raw("ab\u{26A0}\u{FE0F}")]);
    let out = fit_line_to_width(line, 3);
    assert_eq!(line_text(&out).width(), 3);
    assert_eq!(line_text(&out), "ab ");
}

#[test]
fn fit_line_clips_grapheme_straddle_in_later_span() {
    // The straddle happens in a later span: after "ab" one column is left, so
    // the width-2 ⚠️ is dropped whole and the gap padded
    let line = Line::from(vec![Span::raw("ab"), Span::raw("\u{26A0}\u{FE0F}cd")]);
    let out = fit_line_to_width(line, 3);
    assert_eq!(line_text(&out).width(), 3);
    assert_eq!(line_text(&out), "ab ");
}

#[test]
fn fit_line_drops_subsequent_spans_after_clip() {
    let line = Line::from(vec![
        Span::raw("hello"),
        Span::raw(" world"),
        Span::raw("!!!"),
    ]);
    let out = fit_line_to_width(line, 5);
    assert_eq!(line_text(&out), "hello");
    assert_eq!(out.spans.len(), 1);
}

#[test]
fn fit_line_takes_partial_of_later_span() {
    let line = Line::from(vec![Span::raw("ab"), Span::raw("cdef")]);
    let out = fit_line_to_width(line, 4);
    assert_eq!(line_text(&out), "abcd");
    assert_eq!(line_text(&out).width(), 4);
}

#[test]
fn fit_line_zero_width_returns_empty() {
    let line = Line::from(vec![Span::raw("│ a │")]);
    let out = fit_line_to_width(line, 0);
    assert_eq!(line_text(&out), "");
    assert_eq!(line_text(&out).width(), 0);
}

#[test]
fn fit_line_preserves_span_styles_when_padding() {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let line = Line::from(vec![Span::styled("hi", bold)]);
    let out = fit_line_to_width(line, 5);
    assert_eq!(line_text(&out).width(), 5);
    let Some(span) = out.spans.first() else {
        panic!("expected a span: {:?}", out.spans);
    };
    assert!(span.style.add_modifier.contains(Modifier::BOLD));
}

// ── push_owned_lines / line_to_static ────────────────────────

/// `push_owned_lines` turns borrowed lines into independent `'static` copies:
/// the source can be dropped (proving the borrow is gone) without touching
/// the output.
#[test]
fn push_owned_lines_makes_static_copies() {
    let source = String::from("owned text");
    let src = vec![Line::from(vec![
        Span::raw(source.as_str()),
        Span::raw(" two"),
    ])];
    let mut out = Vec::new();
    push_owned_lines(&src, &mut out);
    assert_eq!(out.len(), 1);
    drop(src);
    assert_eq!(line_text(&out[0]), "owned text two");
}

#[test]
fn line_to_static_returns_owned_content() {
    let source = "borrowed".to_string();
    let line = Line::from(vec![Span::raw(source.as_str()), Span::raw(" tail")]);
    let owned = line_to_static(&line);
    assert_eq!(line_text(&owned), "borrowed tail");
    for span in &owned.spans {
        assert!(
            matches!(span.content, std::borrow::Cow::Owned(_)),
            "span content must be owned after line_to_static"
        );
    }
}

// ── cascade_truncate ────────────────────────────────────────

#[test]
fn cascade_truncate_all_fit() {
    let (t, d, a, m) = cascade_truncate(50, "type  ", "description", " \u{2014} running", "  meta");
    assert_eq!(t, "type  ");
    assert_eq!(d, "description");
    assert_eq!(a, " \u{2014} running");
    assert_eq!(m, "  meta");
}

/// With the ellipsis-free backing the truncated pieces are plain prefixes
/// (upstream appended `…`; documented in the source header).
#[test]
fn cascade_truncate_desc_truncated() {
    let (t, d, a, m) = cascade_truncate(
        25,
        "type  ",
        "long description here",
        " \u{2014} running",
        "  meta",
    );
    assert_eq!(t, "type  ");
    assert_eq!(d, "lo\u{2026}");
    assert_eq!(a, " \u{2014} running");
    assert_eq!(m, "  meta");
}

#[test]
fn cascade_truncate_desc_gone_meta_truncated() {
    // Overhead 22 exceeds avail 20, so the description goes and meta is
    // truncated to the 4 columns left (upstream: "  m…"; ours: plain prefix).
    let (t, d, a, m) = cascade_truncate(20, "type  ", "desc", " \u{2014} running", "  meta");
    assert_eq!(t, "type  ");
    assert_eq!(d, "");
    assert_eq!(a, " \u{2014} running");
    assert_eq!(m, "  m\u{2026}");
}

#[test]
fn cascade_truncate_meta_and_activity_gone() {
    // With avail 8, type takes 6, activity is truncated to the 2 columns left
    // (" —" fits exactly; upstream appended the ellipsis: " …"), and meta goes.
    let (t, d, a, m) = cascade_truncate(8, "type  ", "desc", " \u{2014} running", "  meta");
    assert_eq!(t, "type  ");
    assert_eq!(d, "");
    assert_eq!(a, " \u{2026}");
    assert_eq!(m, "");
}

#[test]
fn cascade_truncate_type_truncated() {
    let (t, d, a, m) = cascade_truncate(3, "type  ", "desc", " \u{2014} running", "  meta");
    assert_eq!(t, "ty\u{2026}");
    assert_eq!(d, "");
    assert_eq!(a, "");
    assert_eq!(m, "");
}

#[test]
fn cascade_truncate_zero_avail() {
    let (t, d, a, m) = cascade_truncate(0, "type  ", "desc", " \u{2014} running", "  meta");
    assert_eq!(t, "");
    assert_eq!(d, "");
    assert_eq!(a, "");
    assert_eq!(m, "");
}

#[test]
fn cascade_truncate_unicode() {
    // `✗` and `—` are each 1 display column wide; desc_max = 1 keeps one glyph.
    let (t, d, a, m) = cascade_truncate(10, "\u{2717}  ", "description", " \u{2014} run", "");
    assert_eq!(t, "\u{2717}  ");
    assert_eq!(d, "\u{2026}");
    assert_eq!(a, " \u{2014} run");
    assert_eq!(m, "");
}

#[test]
fn cascade_truncate_overhead_equals_avail() {
    // Overhead exactly equals avail, so the description is empty and everything else fits
    let (t, d, a, m) = cascade_truncate(22, "type  ", "desc", " \u{2014} running", "  meta");
    assert_eq!(t, "type  ");
    assert_eq!(d, "");
    assert_eq!(a, " \u{2014} running");
    assert_eq!(m, "  meta");
}

#[test]
fn cascade_truncate_avail_one() {
    let (t, d, a, m) = cascade_truncate(1, "type", "desc", "act", "meta");
    assert_eq!(t, "\u{2026}");
    assert_eq!((d.as_str(), a.as_str(), m.as_str()), ("", "", ""));
}

#[test]
fn cascade_truncate_all_empty() {
    let (t, d, a, m) = cascade_truncate(10, "", "", "", "");
    assert_eq!(
        (t.as_str(), d.as_str(), a.as_str(), m.as_str()),
        ("", "", "", "")
    );
}
