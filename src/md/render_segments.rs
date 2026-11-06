//! Derived from xai-grok-markdown/src/render.rs (xai-org/grok-build, Apache-2.0). Modified: ratatui path only (render_ansi/Stylize excluded); split for the 300-line cap (render_ratatui's main text-chunk path extracted as emit_text_ranges); crate::md paths; StyleInto reused from parse.rs; let-chains→nested ifs (Rust 2021); loop locals grouped into RenderLoopState (mechanical `state.` renames); preamble closures `in_untagged_code`/`count_newlines_in_range` promoted to pub(crate) fns for cross-file helper reuse.

use ratatui::text::{Line, Span};

use crate::md::buffers::{unicode_display_width, MarkdownBuffers};
use crate::md::hyperlinks::{chunk_link_offsets, emit_segment_hyperlinks, ChunkLinkRange};
use crate::md::output::HyperlinkTarget;
use crate::md::parse::{ParsedMarkdown, StyleInto};
use crate::md::render_events::RenderLoopState;
use crate::md::style::{all_hidden, merge_styles};

/// Upstream `in_untagged_code` closure of `render_ratatui` (stateless — two
/// parameters), promoted to a free fn for the T6d cross-file split.
pub(crate) fn in_untagged_code(pos: usize, buffers: &MarkdownBuffers) -> bool {
    buffers
        .untagged_code_ranges
        .iter()
        .any(|range| pos >= range.start && pos < range.end)
}

/// Upstream `count_newlines_in_range` closure of `render_ratatui` (stateless —
/// three parameters), promoted to a free fn for the T6d cross-file split.
pub(crate) fn count_newlines_in_range(from: usize, to: usize, text: &str) -> usize {
    if to <= from {
        return 0;
    }
    let to = to.min(text.len());
    let from = from.min(to);
    // Use as_bytes() to avoid panicking on non-char-boundary offsets.
    // This is safe because '\n' (0x0A) is a single-byte ASCII value that can never appear as a UTF-8 continuation byte (0x80..0xBF)
    text.as_bytes()
        .get(from..to)
        .map(|s| s.iter().filter(|&&b| b == b'\n').count())
        .unwrap_or(0)
}

impl<'a, 'b> ParsedMarkdown<'a, 'b> {
    /// Emit the plain-text ranges between render events (upstream
    /// `render_ratatui`'s main text-chunk path, verbatim modulo the
    /// `RenderLoopState` renames and the promoted closures).
    pub(crate) fn emit_text_ranges(
        &mut self,
        state: &mut RenderLoopState,
        pos: usize,
        pretty: bool,
        lines: &mut Vec<Line<'static>>,
        line_source_map: &mut Vec<usize>,
        hyperlinks: &mut Vec<HyperlinkTarget>,
    ) {
        // Split text processing at the checkpoint boundary when last_pos < cp_byte <= ev.pos:
        // process [last_pos..cp_byte], capture lines.len(), then process [cp_byte..ev.pos]
        let split_at_checkpoint = state.checkpoint_output_lines.is_none()
            && state
                .checkpoint_info
                .map(|(_, cp_byte)| state.last_pos < cp_byte && cp_byte <= pos)
                .unwrap_or(false);

        let cp_byte = state.checkpoint_info.map(|(_, cp)| cp).unwrap_or(0);

        // Snap cp_byte to the nearest char boundary
        // Checkpoint byte offsets come from pulldown-cmark event ranges, which should always be char-aligned
        // Snapping forward is safe: it only affects where we split the text for line counting
        let cp_byte = {
            let mut b = cp_byte;
            while b < self.text.len() && !self.text.is_char_boundary(b) {
                b += 1;
            }
            b
        };

        let ranges: &[(usize, usize)] = if split_at_checkpoint {
            // Process in two parts, capturing checkpoint between them
            &[(state.last_pos, cp_byte), (cp_byte, pos)]
        } else {
            // Process as single range
            &[(state.last_pos, pos)]
        };

        for (range_idx, &(range_start, range_end)) in ranges.iter().enumerate() {
            // After processing the first range when splitting, capture checkpoint.
            // Without this flush, the checkpoint's `output_lines` count would be too low, causing the line to vanish on re-render
            if split_at_checkpoint && range_idx == 1 {
                if !self.buffers.current_spans.is_empty() {
                    line_source_map.push(state.current_source_line);
                    let line = Line::from(std::mem::take(&mut self.buffers.current_spans));
                    lines.push(line);
                    state.cur_col_in_line = 0;
                }
                state.checkpoint_output_lines = Some(lines.len());
            }

            if range_end <= range_start {
                continue;
            }

            // Update source line counter
            if range_start > state.last_line_count_pos {
                state.current_source_line +=
                    count_newlines_in_range(state.last_line_count_pos, range_start, self.text);
                state.last_line_count_pos = range_start;
            }

            let is_hidden = pretty
                && all_hidden(
                    self.buffers
                        .active_highlights
                        .iter()
                        .filter_map(|&i| self.buffers.highlights.get(i).map(|h| h.style)),
                );

            if is_hidden {
                let at_line_start =
                    range_start == 0 || self.text.as_bytes().get(range_start - 1) == Some(&b'\n');
                if at_line_start {
                    // Check if this hidden block is a code fence (``` or ~~~).
                    // Only code fences need separator handling; heading markers (#) are also hidden at line start but are unpaired
                    let hidden_text = self
                        .text
                        .get(range_start..range_end)
                        .unwrap_or("")
                        .trim_start();
                    let is_code_fence =
                        hidden_text.starts_with("```") || hidden_text.starts_with("~~~");

                    if is_code_fence {
                        // Emit a blank separator before an OPENING fence (not closing)
                        // Otherwise adjacent blocks (a list then a code block) collapse together once pretty mode hides the fence markers
                        if !state.in_hidden_code_block
                            && lines.last().is_some_and(|l| l.width() > 0)
                        {
                            line_source_map.push(state.current_source_line);
                            lines.push(Line::default());
                            state.cur_col_in_line = 0;
                        }
                        state.in_hidden_code_block = !state.in_hidden_code_block;
                    }
                    state.skip_leading_newline = true;
                }
            } else {
                let Some(mut text) = self.text.get(range_start..range_end) else {
                    continue;
                };
                let mut text_start = range_start;

                if state.skip_leading_newline {
                    if let Some(rest) = text.strip_prefix('\n') {
                        text = rest;
                        text_start += 1;
                    }
                }
                state.skip_leading_newline = false;

                if !text.is_empty() {
                    let style = merge_styles(
                        self.buffers
                            .active_highlights
                            .iter()
                            .filter_map(|&i| self.buffers.highlights.get(i).map(|h| h.style)),
                    );

                    let transformed = self.apply_transforms(text, range_start, pretty);
                    let ratatui_style: ratatui::style::Style = style.style_into();

                    let chunk_src_start = text_start;
                    let chunk_src_end = text_start + text.len();

                    // Advance the cursor past links that ended before this chunk starts, then check if any remaining link overlaps it
                    // When none does, skip all hyperlink bookkeeping so the common no-link chunk does no extra work
                    while self
                        .buffers
                        .link_targets
                        .get(state.next_link_idx)
                        .is_some_and(|t| t.source_range.end <= chunk_src_start)
                    {
                        state.next_link_idx += 1;
                    }
                    let chunk_has_links = self
                        .buffers
                        .link_targets
                        .get(state.next_link_idx)
                        .is_some_and(|t| t.source_range.start < chunk_src_end);

                    let chunk_links: Vec<ChunkLinkRange> = if chunk_has_links {
                        chunk_link_offsets(
                            &self.buffers.link_targets,
                            state.next_link_idx,
                            chunk_src_start,
                            chunk_src_end,
                            pretty,
                            &self.buffers.transforms,
                        )
                    } else {
                        Vec::new()
                    };

                    let mut byte_offset = text_start;
                    let mut seg_x_offset: usize = 0;
                    let is_in_code = in_untagged_code(text_start, self.buffers);
                    state.pending_line_is_code = is_in_code;
                    for (idx, segment) in transformed.split('\n').enumerate() {
                        if idx > 0 {
                            line_source_map.push(state.current_source_line);
                            let line = Line::from(std::mem::take(&mut self.buffers.current_spans));
                            lines.push(if is_in_code {
                                line.style(state.code_bg_style)
                            } else {
                                line
                            });
                            if byte_offset > state.last_line_count_pos {
                                state.current_source_line += count_newlines_in_range(
                                    state.last_line_count_pos,
                                    byte_offset,
                                    self.text,
                                );
                                state.last_line_count_pos = byte_offset;
                            }
                            state.cur_col_in_line = 0;
                        }

                        if !chunk_links.is_empty() {
                            emit_segment_hyperlinks(
                                &chunk_links,
                                &self.buffers.link_targets,
                                segment,
                                seg_x_offset,
                                state.cur_col_in_line,
                                lines.len(),
                                hyperlinks,
                            );
                        }

                        if !segment.is_empty() {
                            self.buffers
                                .current_spans
                                .push(Span::styled(segment.to_string(), ratatui_style));
                            state.cur_col_in_line += unicode_display_width(segment);
                        }
                        byte_offset += segment.len() + 1;
                        seg_x_offset += segment.len() + 1;
                    }
                }
            }
        }
        state.last_pos = pos;
    }
}
