//! Contract tests for `src/md/parse_events.rs` (V3/T5f).
//! Adapted from `xai-grok-markdown/src/parse.rs` (Apache-2.0).

use nxm_tui::md::buffers::MarkdownBuffers;
use nxm_tui::md::parse::MarkdownParser;
use nxm_tui::md::style::MarkdownStyle;
use nxm_tui::md::syntax::Syntect;

/// `parse()` on markdown base popola i buffer con highlight (heading + bold).
#[test]
fn parse_produces_highlights_for_heading_and_bold() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parsed = MarkdownParser::new(
        "# Heading\n\ntesto **bold**",
        style,
        &mut buffers,
        Some(&syntect),
    )
    .parse();

    assert_eq!(parsed.text, "# Heading\n\ntesto **bold**");
    assert!(
        !parsed.buffers.highlights.is_empty(),
        "expected highlights for heading and bold text"
    );
}

/// Inline code `` `code` `` produce uno span con stile inline-code.
#[test]
fn inline_code_produces_span_with_style() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parsed = MarkdownParser::new("`code`", style, &mut buffers, Some(&syntect)).parse();

    assert_eq!(parsed.text, "`code`");
    assert!(
        !parsed.buffers.highlights.is_empty(),
        "expected highlights for inline code"
    );
}

/// Strikethrough `~~x~~` è gestito senza panico.
#[test]
fn strikethrough_is_handled() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parsed = MarkdownParser::new("~~x~~", style, &mut buffers, Some(&syntect)).parse();

    assert_eq!(parsed.text, "~~x~~");
    assert!(
        !parsed.buffers.highlights.is_empty(),
        "expected highlights for strikethrough"
    );
}

/// Ramo math con latex stub: `$x$` non panica e non rompe.
#[test]
fn math_inline_does_not_panic() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parsed = MarkdownParser::new("$x$", style, &mut buffers, Some(&syntect)).parse();

    assert_eq!(parsed.text, "$x$");
    // Non deve panica; output può essere vuoto o contenere il sorgente TeX
    assert!(!parsed.buffers.highlights.is_empty() || parsed.buffers.transforms.is_empty());
}
