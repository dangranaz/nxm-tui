//! Contract tests for `src/md/render.rs` (V3/T6d).
//! Adapted from upstream `render.rs` inline tests (Apache-2.0), renderer-free:
//! they build a `MarkdownParser`, `.parse()`, then call
//! `ParsedMarkdown::render_ratatui(true)` directly and assert on the produced
//! `MarkdownRenderOutput` (no `StreamingMarkdownRenderer` dependency).

use ratatui::text::Line;

use nxm_tui::md::buffers::MarkdownBuffers;
use nxm_tui::md::output::MarkdownRenderOutput;
use nxm_tui::md::parse::MarkdownParser;
use nxm_tui::md::syntax::Syntect;
use nxm_tui::md::theme::fixed_style;

fn render(src: &str) -> MarkdownRenderOutput {
    let mut buffers = MarkdownBuffers::new();
    let syntect = Syntect::new().expect("syntect defaults");
    let mut parsed = MarkdownParser::new(src, fixed_style(), &mut buffers, Some(&syntect)).parse();
    let (output, _checkpoint) = parsed.render_ratatui(true);
    output
}

/// Concatenate the text of every span in the highlighted lines.
fn lines_text(lines: &[Line<'static>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// MILESTONE: `render_ratatui` produces a `MarkdownRenderOutput` with
/// renderable `Line`s; heading/bold/inline-code text survives while the
/// pretty markers are consumed into spans.
#[test]
fn heading_bold_code_rendered() {
    let output = render("# Titolo\n\ntesto **bold** e `code`");
    assert!(!output.lines.is_empty());
    let text = lines_text(&output.lines);
    assert!(text.contains("Titolo"));
    assert!(text.contains("bold"));
    assert!(text.contains("code"));
}

/// A fenced code block renders its body lines (styled, code background) and
/// yields a `CodeBlockSpan`.
#[test]
fn code_fence_renders_block() {
    let output: MarkdownRenderOutput = render("```rust\nlet x = 1;\n```\n");
    assert!(!output.lines.is_empty());
    let text = lines_text(&output.lines);
    assert!(text.contains("let x = 1;"));
    assert_eq!(output.code_blocks.len(), 1);
}

/// A GFM table renders its rows as pre-styled lines.
#[test]
fn gfm_table_renders_rows() {
    let output = render("| a | b |\n|---|---|\n| 1 | 2 |\n");
    assert!(!output.lines.is_empty());
    let text = lines_text(&output.lines);
    assert!(text.contains('a'));
    assert!(text.contains('b'));
    assert!(text.contains('1'));
    assert!(text.contains('2'));
}

/// A `[text](url)` link produces a `HyperlinkTarget` in the output.
#[test]
fn link_produces_hyperlink_target() {
    let output = render("leggi [la guida](https://example.com/guide) ora");
    assert!(!output.lines.is_empty());
    assert_eq!(output.hyperlinks.len(), 1);
    let link = &output.hyperlinks[0];
    assert_eq!(link.url, "https://example.com/guide");
    assert!(link.column_range.start < link.column_range.end);
    let text = lines_text(&output.lines);
    assert!(text.contains("la guida"));
}
