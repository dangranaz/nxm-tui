//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; latex/format_table live in their own modules (latex stub/T-latex, table formatting `parse_table.rs`/T5g); mermaid/math branches stubbed.

use std::ops::Range;

use anstyle::Style;
use pulldown_cmark::TagEnd;
use ratatui::text::Line;

use crate::md::buffers::{CodeBlockMeta, TableReplace};
use crate::md::buffers_table::{TableCellCopy, TableHyperlink};
use crate::md::checkpoint::CheckpointKind;
use crate::md::parse::{has_blank_line_after, MarkdownParser};

/// Output of [`MarkdownParser::format_table`]: the rendered lines of a single table.
/// Rendered by `parse_table.rs::format_table` (T5g); kept here where the T5f
/// split defined it so `on_end` can destructure it next to the buffer push.
#[derive(Default)]
pub(crate) struct FormattedTable {
    /// Plain-text lines (for ANSI rendering).
    pub lines: Vec<String>,
    /// Styled lines (for ratatui rendering).
    pub styled_lines: Vec<Line<'static>>,
    /// Per-line source offset within the table (0 = header, 1 = separator, 2+ = body rows).
    pub line_source_offsets: Vec<usize>,
    /// Hyperlinks (in table-local line coordinates) for links inside cells.
    pub hyperlinks: Vec<TableHyperlink>,
    pub cell_copies: Vec<TableCellCopy>,
    pub n_cols: usize,
}

impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Handle tag end events.
    pub(crate) fn on_end(&mut self, tag_end: TagEnd, range: Range<usize>) {
        self.tag_stack.pop();

        let style = self.on_end_style(&tag_end, range.clone());

        if let Some(style) = style {
            self.push_highlight(Some(style), &range);
        }

        self.on_end_depth_checkpoint(&tag_end, range);
    }

    /// Compute style for tag end event.
    fn on_end_style(&mut self, tag_end: &TagEnd, range: Range<usize>) -> Option<Style> {
        match tag_end {
            TagEnd::Emphasis => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_italic = false;
                }
                None
            }
            TagEnd::Strong => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_bold = false;
                }
                None
            }
            TagEnd::Strikethrough => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_strike = false;
                }
                None
            }
            TagEnd::CodeBlock => {
                if let Some(pending) = self.pending_code_block.take() {
                    if pending.body_range.end < range.end {
                        self.buffers.code_blocks.push(CodeBlockMeta {
                            info: pending.info,
                            body: pending.body_text,
                            body_source_range: pending.body_range,
                        });
                    }
                }
                None
            }
            TagEnd::Link | TagEnd::Image => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_link = None;
                }
                None
            }
            TagEnd::TableCell => {
                if let Some(ref mut state) = self.table_state {
                    state
                        .current_row
                        .push(std::mem::take(&mut state.current_cell));
                    state.cell_bold = false;
                    state.cell_italic = false;
                    state.cell_code = false;
                    state.cell_strike = false;
                    state.cell_link = None;
                }
                None
            }
            TagEnd::TableRow => {
                if let Some(ref mut state) = self.table_state {
                    if !state.in_header {
                        let row = std::mem::take(&mut state.current_row);
                        state.rows.push(row);
                    }
                }
                None
            }
            TagEnd::TableHead => {
                if let Some(ref mut state) = self.table_state {
                    state.header = std::mem::take(&mut state.current_row);
                    state.in_header = false;
                }
                None
            }
            TagEnd::Table => {
                if let Some(mut state) = self.table_state.take() {
                    state.range.end = range.end;
                    let FormattedTable {
                        lines,
                        styled_lines,
                        line_source_offsets,
                        hyperlinks,
                        cell_copies,
                        n_cols,
                    } = self.format_table(&state);
                    self.buffers.table_replaces.push(TableReplace {
                        lines,
                        styled_lines,
                        range: state.range,
                        line_source_offsets,
                        hyperlinks,
                        cell_copies,
                        n_cols,
                    });
                }
                None
            }
            _ => None,
        }
    }

    /// Handle depth tracking and checkpoint recording for tag end events.
    fn on_end_depth_checkpoint(&mut self, tag_end: &TagEnd, range: Range<usize>) {
        match tag_end {
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::Table => {
                self.depth = self.depth.saturating_sub(1);
            }
            _ => {}
        }
        if matches!(tag_end, TagEnd::BlockQuote(_)) {
            self.bq_depth = self.bq_depth.saturating_sub(1);
        }

        if self.depth == 0 {
            let kind = match tag_end {
                TagEnd::Paragraph => Some(CheckpointKind::Paragraph),
                TagEnd::Heading(_) => Some(CheckpointKind::Heading),
                TagEnd::CodeBlock => Some(CheckpointKind::CodeBlock),
                TagEnd::BlockQuote(_) => Some(CheckpointKind::BlockQuote),
                TagEnd::List(_) => Some(CheckpointKind::List),
                TagEnd::Table => Some(CheckpointKind::Table),
                TagEnd::HtmlBlock => Some(CheckpointKind::HtmlBlock),
                _ => None,
            };

            if let Some(kind) = kind {
                let has_blank = has_blank_line_after(self.text, range.end);
                let is_code_block = matches!(kind, CheckpointKind::CodeBlock);
                let at_eof = range.end >= self.text.len();
                let code_block_properly_closed = is_code_block && !at_eof;

                if has_blank || code_block_properly_closed {
                    let checkpoint_pos = if is_code_block && has_blank {
                        range.end + 1
                    } else {
                        range.end
                    };
                    self.last_checkpoint = Some((kind, checkpoint_pos));
                }
            }
        }
    }

    /// Render a mermaid code block into a [`MermaidReplace`]; `true` if drawn.
    /// Stub for T5g — returns false until the mermaid module is implemented.
    pub(crate) fn try_push_mermaid(&mut self, _text: &str, _range: &Range<usize>) -> bool {
        // T5g: mermaid rendering not yet implemented.
        // Returns false so the caller falls through to plain code-block rendering.
        false
    }

    /// Push a pretty-mode block replacement rendering `latex_src` as display math over `range`.
    /// Stub for T-latex — returns false until the latex module is implemented.
    pub(crate) fn push_display_math_block(
        &mut self,
        _range: Range<usize>,
        _latex_src: &str,
    ) -> bool {
        // T-latex: display math rendering not yet implemented.
        // Returns false so the caller falls through to highlighting the TeX source as code.
        false
    }
}
