//! Derived from xai-grok-markdown/src/style.rs (xai-org/grok-build, Apache-2.0).
//! Modified: moved from the upstream `#[cfg(test)] test_style` module into a
//! standalone integration test (RULES.md: no inline tests).
//!
//! The `STYLE` constant is the faithful port of the upstream `test_style`
//! module (behind `#[cfg(any(test, fuzzing))]` in `xai-grok-markdown/src/style.rs`,
//! Apache-2.0), kept here as an external integration test per RULES.md.
//!
//! Per T1-review Resolution 1, `all_hidden`, `merge_styles`, and
//! `adapt_style` are `pub(crate)` (matching upstream) and therefore not
//! reachable from external integration tests. Their behaviour is exercised
//! only through the public API — `MarkdownStyle::adapt()` (which delegates to
//! `adapt_style`) — and the public `MarkdownStyle` fields and `TableBorders`
//! constants.

use anstyle::{Color, Effects, RgbColor, Style};

use nxm_tui::md::style::{MarkdownStyle, TableBorders};

/// A minimal style for testing with no colors (ex-upstream `test_style::STYLE`).
pub const STYLE: MarkdownStyle = MarkdownStyle {
    heading_inner: [Style::new().bold(); 6],
    heading_outer: [Style::new().dimmed().hidden(); 6],
    strong_inner: Style::new().bold(),
    strong_outer: Style::new().dimmed().hidden(),
    emphasis_inner: Style::new().italic(),
    emphasis_outer: Style::new().dimmed().hidden(),
    strikethrough_inner: Style::new().strikethrough(),
    strikethrough_outer: Style::new().dimmed().hidden(),
    inline_code_inner: Style::new().bold(),
    inline_code_outer: Style::new().dimmed().hidden(),
    blockquote_outer: Style::new().dimmed(),
    task_checked: Style::new(),
    task_unchecked: Style::new().dimmed(),
    list_item: Style::new().dimmed(),
    rule: Style::new(),
    link_outer: Style::new(),
    link_text: Style::new().bold(),
    link_url: Style::new().dimmed(),
    link_title: Style::new(),
    code_outer: Style::new().dimmed().hidden(),
    code_language: Style::new().hidden(),
    code_untagged: Style::new(),
    code_background: Style::new(),
    table_outer: Style::new().bold(),
    text: Style::new(),
    math: Style::new().italic(),
};

// ---------------------------------------------------------------------------
// MarkdownStyle field contract
// ---------------------------------------------------------------------------

#[test]
fn markdown_style_has_all_upstream_fields() {
    // Constructing via the const proves every field exists with the expected type.
    let _ = STYLE;

    // Spot-check the effects the ticket requires.
    assert!(STYLE.strong_inner.get_effects().contains(Effects::BOLD));
    assert!(STYLE.emphasis_inner.get_effects().contains(Effects::ITALIC));
    assert!(STYLE
        .strikethrough_inner
        .get_effects()
        .contains(Effects::STRIKETHROUGH));
}

#[test]
fn markdown_style_default_is_plain() {
    let def = MarkdownStyle::default();
    assert!(def.text.get_effects().is_plain());
    assert!(def.strong_inner.get_effects().is_plain());
}

// ---------------------------------------------------------------------------
// TableBorders
// ---------------------------------------------------------------------------

#[test]
fn table_borders_box_chars() {
    assert_eq!(TableBorders::BOX.horizontal(), '─');
    assert_eq!(TableBorders::BOX.vertical(), '│');
    assert_eq!(TableBorders::BOX.cross(), '┼');
}

#[test]
fn table_borders_ascii_chars() {
    assert_eq!(TableBorders::ASCII.horizontal(), '-');
    assert_eq!(TableBorders::ASCII.vertical(), '|');
    assert_eq!(TableBorders::ASCII.cross(), '+');
}

#[test]
fn table_borders_double_chars() {
    assert_eq!(TableBorders::DOUBLE.horizontal(), '═');
    assert_eq!(TableBorders::DOUBLE.vertical(), '║');
    assert_eq!(TableBorders::DOUBLE.cross(), '╬');
}

#[test]
fn table_borders_default_is_box() {
    assert_eq!(TableBorders::default(), TableBorders::BOX);
}

#[test]
fn table_borders_new_preserves_chars() {
    let t = TableBorders::new(['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k']);
    assert_eq!(t.horizontal(), 'a');
    assert_eq!(t.vertical(), 'b');
}

// ---------------------------------------------------------------------------
// adapt_style (identity placeholder — tested indirectly via MarkdownStyle::adapt)
// ---------------------------------------------------------------------------

#[test]
fn adapt_identity_through_adapt() {
    // MarkdownStyle::adapt() delegates to adapt_style on every field.
    // Since adapt_style is the identity (pub(crate) stub), adapt() must return
    // a style identical for every field — including one with a truecolor (RGB)
    // foreground, which is the value adapt_style will eventually downgrade.
    let md = MarkdownStyle {
        text: Style::new().fg_color(Some(Color::Rgb(RgbColor(255, 100, 50)))),
        ..MarkdownStyle::default()
    };
    let adapted = md.adapt();
    assert_eq!(adapted.text, md.text);
    assert_eq!(adapted.strong_inner, md.strong_inner);
}
