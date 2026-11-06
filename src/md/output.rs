//! Derived from xai-grok-markdown/src/output.rs (xai-org/grok-build, Apache-2.0). Modified: completed in T6b (CodeBlockSpan, MarkdownRenderOutput, MarkdownRenderView, build_code_block_spans); buffers path crate::md::buffers (split: CodeBlockMeta in buffers.rs, TableCopyMeta in buffers_table.rs); build_code_block_spans widened from pub(crate) to pub (visibility-for-testability, tested in tests/md_output.rs).

use std::ops::Range;

use ratatui::text::Line;

use crate::md::buffers::CodeBlockMeta;
use crate::md::buffers_table::TableCopyMeta;

/// A hyperlink target extracted from rendered markdown.
/// The shared `id` enables OSC 8 hover-grouping across wrapped lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperlinkTarget {
    /// Index of the rendered line this target appears on.
    pub line_index: usize,
    /// Column range (in display cells) of the link text on that line.
    pub column_range: Range<usize>,
    /// The destination URL.
    pub url: String,
    /// Stable identifier for grouping link fragments that belong to the same logical link (e.g., a link whose text wraps across lines).
    pub id: u32,
}

/// A fenced code block discovered while rendering markdown.
/// `pulldown-cmark` synthesizes a block end at end-of-input, so closure requires a closing fence after the body rather than the end event alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockSpan {
    /// The fence info string, e.g. `"mermaid"` or `"rust"`.
    /// Empty for a fence opened with no info (just ` ``` `).
    /// Reported verbatim as `pulldown-cmark` yields it (the full info string, not just the first word).
    pub info: String,

    /// The fence body content: the clean, container-stripped code/diagram source.
    /// Prefer this over slicing [`source_byte_range`](Self::source_byte_range) for the logical body.
    pub body: String,

    /// Range of **pre-wrap** rendered body lines for this block, as indices into [`MarkdownRenderOutput::lines`] / [`MarkdownRenderView::lines`].
    /// Covers only the body, with the delimiter ` ``` ` lines excluded, so hiding those delimiters in pretty mode does not affect it.
    /// Empty (`start == end`) for a fence with an empty body.
    pub output_line_range: Range<usize>,

    /// Byte range of the fence body in the **raw** source text.
    /// For a fence nested in a blockquote or list, continuation lines may keep container markers/indentation (and `\r` for CRLF).
    /// Use [`body`](Self::body) for the clean content.
    pub source_byte_range: Range<usize>,
}

/// Output from rendering markdown to ratatui Lines.
///
/// Contains all the information needed to display rendered markdown and support copy operations back to source text.
#[derive(Debug, Clone, Default)]
pub struct MarkdownRenderOutput {
    /// Rendered lines ready for display.
    pub lines: Vec<Line<'static>>,

    /// Maps each rendered line index to its source line number.
    /// `line_source_map[rendered_line_idx]` is the source line number (0-indexed).
    pub line_source_map: Vec<usize>,

    /// Maps a cell range on a rendered line to a URL.
    /// Links that wrap across lines produce multiple entries with the same `id` and `url`.
    pub hyperlinks: Vec<HyperlinkTarget>,

    /// Fenced code blocks discovered during rendering, in document order.
    /// One entry per closed fenced block; see [`CodeBlockSpan`].
    pub code_blocks: Vec<CodeBlockSpan>,
    pub tables: Vec<TableCopyMeta>,
}

impl MarkdownRenderOutput {
    /// Create a new empty output.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all content, keeping allocated capacity.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.line_source_map.clear();
        self.hyperlinks.clear();
        self.code_blocks.clear();
        self.tables.clear();
    }

    /// Get a borrowed view of this output.
    pub fn as_view(&self) -> MarkdownRenderView<'_> {
        MarkdownRenderView {
            lines: &self.lines,
            line_source_map: &self.line_source_map,
            hyperlinks: &self.hyperlinks,
            code_blocks: &self.code_blocks,
            tables: &self.tables,
        }
    }
}

/// Borrowed view of rendered markdown output.
///
/// This is a zero-copy reference to rendered content, used by the streaming renderer to avoid cloning frozen content on every render.
#[derive(Debug, Clone, Copy)]
pub struct MarkdownRenderView<'a> {
    /// Rendered lines ready for display.
    pub lines: &'a [Line<'static>],

    /// Maps each rendered line index to its source line number.
    pub line_source_map: &'a [usize],

    /// Hyperlink targets extracted from the rendered markdown.
    pub hyperlinks: &'a [HyperlinkTarget],

    /// Fenced code blocks discovered during rendering, in document order.
    /// One entry per closed fenced block; see [`CodeBlockSpan`].
    pub code_blocks: &'a [CodeBlockSpan],
    pub tables: &'a [TableCopyMeta],
}

impl<'a> MarkdownRenderView<'a> {
    /// Get the number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Map parse-time code-block metadata onto the rendered output.
/// The renderer emits exactly one output line per body source line and never maps a non-body line into that range.
/// Rescanning from byte 0 for every meta would be O(metas·text_len), quadratic in the number of fences on the streaming hot path.
pub fn build_code_block_spans(
    text: &str,
    line_source_map: &[usize],
    metas: Vec<CodeBlockMeta>,
) -> Vec<CodeBlockSpan> {
    if metas.is_empty() {
        return Vec::new();
    }

    let bytes = text.as_bytes();
    // Monotonic newline cursor. Each query advances from the previous position (metas ascend by body offset), so the whole pass is O(text_len).
    // '\n' is single-byte ASCII, so byte counting is UTF-8-safe at any offset
    let mut cursor_pos = 0usize;
    let mut cursor_newlines = 0usize;
    let mut newlines_before = |pos: usize| -> usize {
        let pos = pos.min(bytes.len());
        debug_assert!(
            pos >= cursor_pos,
            "metas must be processed in ascending body order",
        );
        while cursor_pos < pos {
            if bytes.get(cursor_pos) == Some(&b'\n') {
                cursor_newlines += 1;
            }
            cursor_pos += 1;
        }
        cursor_newlines
    };

    metas
        .into_iter()
        .map(|meta| {
            let range = meta.body_source_range;
            let src_first = newlines_before(range.start);
            let output_line_range = if range.end <= range.start {
                // Empty body: no rendered body lines. Anchor an empty range at the first output line that does not precede the body.
                let start = line_source_map.partition_point(|&src| src < src_first);
                start..start
            } else {
                // `range.end - 1` is the last body byte; its source line is the inclusive last body source line, robust to a trailing newline
                let src_last = newlines_before(range.end - 1);
                let start = line_source_map.partition_point(|&src| src < src_first);
                let end = line_source_map.partition_point(|&src| src <= src_last);
                start..end
            };
            CodeBlockSpan {
                info: meta.info,
                body: meta.body,
                output_line_range,
                source_byte_range: range,
            }
        })
        .collect()
}
