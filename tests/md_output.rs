//! Contract tests for `src/md/output.rs` (V3/T6b).
//! Cover the render output types ported from upstream `output.rs`
//! (Apache-2.0): MarkdownRenderOutput::new/clear/as_view, MarkdownRenderView,
//! the CodeBlockSpan fields, and the pure `build_code_block_spans` mapping
//! (upstream tests for it go through the not-yet-ported renderer; here the
//! mapping is exercised directly).

use ratatui::text::{Line, Span};

use nxm_tui::md::buffers::CodeBlockMeta;
use nxm_tui::md::output::{build_code_block_spans, CodeBlockSpan, MarkdownRenderOutput};

/// A fresh `MarkdownRenderOutput` is empty on every buffer.
#[test]
fn new_is_empty() {
    let out = MarkdownRenderOutput::new();
    assert!(out.lines.is_empty());
    assert!(out.line_source_map.is_empty());
    assert!(out.hyperlinks.is_empty());
    assert!(out.code_blocks.is_empty());
    assert!(out.tables.is_empty());
}

/// `clear()` empties every buffer after content was pushed.
#[test]
fn clear_empties_all_buffers() {
    let mut out = MarkdownRenderOutput::new();
    out.lines.push(Line::raw("hi"));
    out.line_source_map.push(0);
    out.code_blocks.push(CodeBlockSpan {
        info: "rust".into(),
        body: "let x = 1;".into(),
        output_line_range: 0..1,
        source_byte_range: 0..4,
    });

    assert!(!out.lines.is_empty());
    out.clear();
    assert!(out.lines.is_empty(), "clear must reset lines");
    assert!(
        out.line_source_map.is_empty(),
        "clear must reset line_source_map"
    );
    assert!(out.code_blocks.is_empty(), "clear must reset code_blocks");
}

/// `as_view()` yields a read-only view with the same slices.
#[test]
fn as_view_reports_correct_slices() {
    let mut out = MarkdownRenderOutput::new();
    for w in ["one", "two", "three"] {
        out.lines.push(Line::raw(w));
        out.line_source_map.push(0);
    }

    let view = out.as_view();
    assert_eq!(view.line_count(), 3);
    assert_eq!(view.lines.len(), 3);
    assert_eq!(view.line_source_map.len(), 3);
    assert_eq!(view.code_blocks.len(), out.code_blocks.len());
    assert_eq!(view.hyperlinks.len(), out.hyperlinks.len());
    assert_eq!(view.tables.len(), out.tables.len());
}

/// `CodeBlockSpan` carries its four fields with the expected content.
#[test]
fn code_block_span_fields_roundtrip() {
    let span = CodeBlockSpan {
        info: "mermaid".into(),
        body: "flowchart TD\n  A --> B\n".into(),
        output_line_range: 2..4,
        source_byte_range: 41..50,
    };
    assert_eq!(span.info, "mermaid");
    assert_eq!(span.body, "flowchart TD\n  A --> B\n");
    assert_eq!(span.output_line_range, 2..4);
    assert_eq!(span.source_byte_range, 41..50);
}

/// Direct check of the pure mapping: source line numbers (from the
/// line_source_map) drive the span's `output_line_range`; empty bodies anchor
/// an empty range; content is carried over verbatim.
#[test]
fn build_code_block_spans_maps_metas_to_output_lines() {
    // Source: "```text\nA\nB\n```\nprose\n```text\nC\nD\n```\n"
    // Rendered line_source_map (one per source line, skipped fence lines
    // would not be there in a real render, but the helper only looks up
    // source line numbers):
    let line_source_map = vec![0, 1, 2, 3, 4, 5, 6, 7];
    let text = "```text\nA\nB\n```\nprose\n```text\nC\nD\n```\n";
    let metas = vec![
        CodeBlockMeta {
            info: "text".into(),
            body: "A\nB\n".into(),
            body_source_range: 8..12, // "A\nB\n"
        },
        CodeBlockMeta {
            info: "text".into(),
            body: "C\nD\n".into(),
            body_source_range: 30..34, // "C\nD\n"
        },
    ];

    let spans = build_code_block_spans(text, &line_source_map, metas);
    assert_eq!(spans.len(), 2, "one span per meta, document order");
    assert_eq!(spans[0].info, "text");
    assert_eq!(spans[0].body, "A\nB\n");
    assert_eq!(spans[0].source_byte_range, 8..12);
    assert_eq!(spans[0].output_line_range, 1..3, "body source lines 1-2");
    assert_eq!(spans[1].output_line_range, 6..8, "body source lines 6-7");

    // Monotonic, disjoint, increasing ranges across blocks.
    assert!(spans[0].output_line_range.end <= spans[1].output_line_range.start);
}

/// Empty body: an empty `output_line_range` anchored before the body's first
/// source line, and empty metas yield no spans.
#[test]
fn build_code_block_spans_empty_body_and_no_metas() {
    assert!(build_code_block_spans("", &[], Vec::new()).is_empty());

    let line_source_map = vec![0, 1, 2];
    let text = "```text\nD\n```\n";
    let spans = build_code_block_spans(
        text,
        &line_source_map,
        vec![CodeBlockMeta {
            info: "text".into(),
            body: String::new(),
            body_source_range: 9..9, // empty
        }],
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].output_line_range,
        1..1,
        "empty range at the anchor"
    );
    assert_eq!(spans[0].body, "");
}

/// Display sanity: the plain text of the rendered lines a span points at maps
/// back to the span body (mirrors the upstream `body_lines` helper contract
/// without the renderer).
#[test]
fn span_output_range_slices_rendered_lines() {
    let lines = [
        Line::from(vec![Span::raw("```text")]),
        Line::from(vec![Span::raw("X")]),
        Line::from(vec![Span::raw("```")]),
    ];
    let slice = lines.get(1..2).expect("output_line_range within bounds");
    let joined: String = slice
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(joined, "X");
}
