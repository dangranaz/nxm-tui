//! Derived from xai-grok-markdown/src/render.rs (xai-org/grok-build, Apache-2.0). Modified: documented variations below.
//!
//! Markdown renderer, ratatui path (V3/T6d): transforms parsed markdown
//! buffers into styled ratatui output. `render_ratatui` + its support helpers
//! `apply_transforms`, `build_render_events`, `build_render_events_into`.
//!
//! The ANSI path (`render_ansi`, `render_replace_ansi`, `Stylize`/`StyledStr`)
//! is NOT copied: `render_ratatui` does not use it (0 calls, verified).
//!
//! Split for the 300-line cap (all `impl ParsedMarkdown`):
//! - `render.rs`        — `render_ratatui` skeleton + trailing-text helper + checkpoint tail;
//! - `render_events.rs` — `RenderLoopState`, `apply_transforms`, `build_render_events(_into)`;
//! - `render_segments.rs` — main text-chunk emission (`emit_text_ranges`) + the upstream
//!   stateless closures `in_untagged_code`/`count_newlines_in_range` promoted to fns;
//! - `render_blocks.rs` — the `Replace`/`Table`/`Mermaid` arm flush helpers.
//!
//! Upstream `crate::X` → `crate::md::X`; `crate::colors::adapt_style` → the T1
//! `crate::md::style_adapt::adapt_style` stub; `StyleInto`/`anstyle_to_ratatui_color`
//! reused from parse.rs (the latter widened private→pub(crate)); let-chains →
//! nested ifs (Rust 2021); loop locals grouped into `RenderLoopState`
//! (mechanical `state.` renames with names preserved).

use ratatui::text::{Line, Span};

use crate::md::buffers::{unicode_display_width, RenderEventKind};
use crate::md::buffers_table::TableCopyMeta;
use crate::md::checkpoint::Checkpoint;
use crate::md::hyperlinks::{chunk_link_offsets, emit_segment_hyperlinks, ChunkLinkRange};
use crate::md::output::{HyperlinkTarget, MarkdownRenderOutput};
use crate::md::parse::{ParsedMarkdown, StyleInto};
use crate::md::render_events::RenderLoopState;
use crate::md::render_segments::{count_newlines_in_range, in_untagged_code};

impl<'a, 'b> ParsedMarkdown<'a, 'b> {
    /// Render the parsed markdown to ratatui lines. If `pretty` is true,
    /// syntax markers are hidden.
    /// Returns the styled output and an optional checkpoint for streaming.
    pub fn render_ratatui(&mut self, pretty: bool) -> (MarkdownRenderOutput, Option<Checkpoint>) {
        let render_events = self.build_render_events();

        self.buffers.current_spans.clear();
        self.buffers.active_highlights.clear();

        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut line_source_map: Vec<usize> = Vec::new();
        let mut hyperlinks: Vec<HyperlinkTarget> = Vec::new();
        let mut tables: Vec<TableCopyMeta> = Vec::new();

        // Style already adapted, no need to call adapt_style again
        let mut state = RenderLoopState {
            last_pos: 0,
            replace: None,
            table_replace: None,
            mermaid_replace: None,
            skip_leading_newline: false,
            in_hidden_code_block: false,
            next_link_idx: 0,
            // Running display-column tracker for the in-progress line.
            cur_col_in_line: 0,
            current_source_line: 0,
            last_line_count_pos: 0,
            pending_line_is_code: false,
            code_bg_style: self.ms.code_background.style_into(),
            checkpoint_info: self.last_checkpoint,
            checkpoint_output_lines: None,
        };

        for ev in &render_events {
            if state.replace.is_none()
                && state.table_replace.is_none()
                && state.mermaid_replace.is_none()
                && ev.pos > state.last_pos
            {
                self.emit_text_ranges(
                    &mut state,
                    ev.pos,
                    pretty,
                    &mut lines,
                    &mut line_source_map,
                    &mut hyperlinks,
                );
            }

            match ev.kind {
                RenderEventKind::Replace => {
                    self.flush_code_block(
                        &mut state,
                        ev.index,
                        ev.is_end,
                        &mut lines,
                        &mut line_source_map,
                    );
                }
                RenderEventKind::Table => {
                    self.flush_table(
                        &mut state,
                        ev.index,
                        ev.is_end,
                        pretty,
                        &mut lines,
                        &mut line_source_map,
                        &mut hyperlinks,
                        &mut tables,
                    );
                }
                RenderEventKind::Mermaid => {
                    self.flush_mermaid(
                        &mut state,
                        ev.index,
                        ev.is_end,
                        pretty,
                        &mut lines,
                        &mut line_source_map,
                    );
                }
                RenderEventKind::Highlight => {
                    if ev.is_end {
                        self.buffers.active_highlights.retain(|&x| x != ev.index);
                    } else {
                        self.buffers.active_highlights.push(ev.index);
                    }
                }
            }
        }

        self.emit_trailing_text(
            &mut state,
            pretty,
            &mut lines,
            &mut line_source_map,
            &mut hyperlinks,
        );

        // Emit final line. Use the code-block membership of the chunk that produced these spans, not a recomputed one.
        // An unterminated bare fence ends its range exactly at last_pos (EOF); the end-exclusive range check there would drop the code bg
        if !self.buffers.current_spans.is_empty() {
            line_source_map.push(state.current_source_line);
            let final_is_code = state.pending_line_is_code;
            let line = Line::from(std::mem::take(&mut self.buffers.current_spans));
            lines.push(if final_is_code {
                line.style(state.code_bg_style)
            } else {
                line
            });
        }

        let checkpoint = self.finalize_checkpoint(&mut state, &lines, &line_source_map);

        // Now that `line_source_map` is final, map each parsed code block's body onto its rendered (pre-wrap) line range
        let text = self.text;
        let code_blocks = crate::md::output::build_code_block_spans(
            text,
            &line_source_map,
            std::mem::take(&mut self.buffers.code_blocks),
        );

        (
            MarkdownRenderOutput {
                lines,
                line_source_map,
                hyperlinks,
                code_blocks,
                tables,
            },
            checkpoint,
        )
    }

    /// Emit the source text remaining after the last render event (upstream
    /// `render_ratatui`'s trailing-text path, verbatim modulo the
    /// `RenderLoopState` renames). `pretty` passthrough keeps the
    /// whitespace-suppression condition identical to the upstream one.
    fn emit_trailing_text(
        &mut self,
        state: &mut RenderLoopState,
        pretty: bool,
        lines: &mut Vec<Line<'static>>,
        line_source_map: &mut Vec<usize>,
        hyperlinks: &mut Vec<HyperlinkTarget>,
    ) {
        // Handle remaining text
        let len = self.text.len();
        if state.last_pos < len {
            // Apply force transforms only; non-force transforms have never been applied in this trailing path
            // Force transforms preserve byte length, so source offsets below stay valid
            let raw = self.text.get(state.last_pos..len).unwrap_or("");
            let transformed = self.apply_transforms(raw, state.last_pos, false);
            debug_assert_eq!(transformed.len(), raw.len());
            let text: &str = &transformed;
            let is_only_whitespace = text.as_bytes().iter().all(u8::is_ascii_whitespace);

            if !(pretty && is_only_whitespace) {
                if state.last_pos > state.last_line_count_pos {
                    state.current_source_line += count_newlines_in_range(
                        state.last_line_count_pos,
                        state.last_pos,
                        self.text,
                    );
                    state.last_line_count_pos = state.last_pos;
                }
                let chunk_src_start = state.last_pos;
                let chunk_src_end = state.last_pos + text.len();

                // Same cursor-skip pattern as the main path: a no-link chunk skips all hyperlink bookkeeping
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

                // Trailing text bypasses apply_transforms (it's emitted raw), so transformed offsets equal source offsets within the chunk
                let chunk_links: Vec<ChunkLinkRange> = if chunk_has_links {
                    chunk_link_offsets(
                        &self.buffers.link_targets,
                        state.next_link_idx,
                        chunk_src_start,
                        chunk_src_end,
                        false,
                        &[],
                    )
                } else {
                    Vec::new()
                };

                let mut byte_offset = state.last_pos;
                let mut seg_x_offset: usize = 0;
                let is_in_code = in_untagged_code(state.last_pos, self.buffers);
                state.pending_line_is_code = is_in_code;

                for (idx, segment) in text.split('\n').enumerate() {
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
                            .push(Span::raw(segment.to_string()));
                        state.cur_col_in_line += unicode_display_width(segment);
                    }
                    byte_offset += segment.len() + 1;
                    seg_x_offset += segment.len() + 1;
                }
            }
        }
    }
}
