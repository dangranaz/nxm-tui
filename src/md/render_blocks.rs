//! Derived from xai-grok-build/crates/codegen/xai-grok-markdown/src/render.rs — see render.rs header (xai-org/grok-build, Apache-2.0). Modified: ratatui path only (render_ansi/Stylize excluded); split for the 300-line cap (the RenderEventKind::Replace/Table/Mermaid arms extracted as flush helpers); crate::md paths; adapt_style via crate::md::style_adapt; StyleInto reused from parse.rs; anstyle-syntect bridge; let-chains→nested ifs (Rust 2021); loop locals grouped into RenderLoopState (mechanical `state.` renames); arm-local `continue`→`return` (the arm bodies are now methods); none of the three helpers were pub upstream, widened to pub(crate) for the cross-file impl split.

use ratatui::text::{Line, Span};

use crate::md::buffers::unicode_display_width;
use crate::md::buffers_table::TableCopyMeta;
use crate::md::output::HyperlinkTarget;
use crate::md::parse::{ParsedMarkdown, StyleInto};
use crate::md::render_events::RenderLoopState;
use crate::md::render_segments::count_newlines_in_range;
use crate::md::style_adapt::adapt_style;

impl<'a, 'b> ParsedMarkdown<'a, 'b> {
    /// Upstream `RenderEventKind::Replace` arm of `render_ratatui`: flush one
    /// syntax-highlighted code-block replacement into `lines` (verbatim
    /// modulo the `RenderLoopState` renames and the mechanical `continue`→
    /// `return`).
    pub(crate) fn flush_code_block(
        &mut self,
        state: &mut RenderLoopState,
        ev_index: usize,
        is_end: bool,
        lines: &mut Vec<Line<'static>>,
        line_source_map: &mut Vec<usize>,
    ) {
        if is_end && state.replace == Some(ev_index) {
            state.replace = None;
        } else if !is_end && state.replace.is_none() && state.table_replace.is_none() {
            let Some(repl) = self.buffers.replaces.get(ev_index) else {
                return;
            };
            state.replace = Some(ev_index);

            // Update source line to code start
            if repl.range.start > state.last_line_count_pos {
                state.current_source_line +=
                    count_newlines_in_range(state.last_line_count_pos, repl.range.start, self.text);
            }
            let code_start_source_line = state.current_source_line;

            for (line_idx, line_spans) in repl.highlighted.iter().enumerate() {
                state.current_source_line = code_start_source_line + line_idx;

                for (syn_style, text) in line_spans {
                    let full_style = anstyle_syntect::to_anstyle(*syn_style);
                    let with_bg = full_style.bg_color(self.ms.code_background.get_bg_color());
                    // This is the only legitimate inline adapt_style call for a dynamically created syntect+background combo
                    let adapted = adapt_style(with_bg);
                    let ratatui_style: ratatui::style::Style = adapted.style_into();

                    for (idx, segment) in text.split('\n').enumerate() {
                        if idx > 0 {
                            line_source_map.push(state.current_source_line);
                            let line = Line::from(std::mem::take(&mut self.buffers.current_spans))
                                .style(state.code_bg_style);
                            lines.push(line);
                            state.current_source_line += 1;
                            state.cur_col_in_line = 0;
                        }
                        if !segment.is_empty() {
                            self.buffers
                                .current_spans
                                .push(Span::styled(segment.to_string(), ratatui_style));
                            state.cur_col_in_line += unicode_display_width(segment);
                        }
                    }
                }

                if !self.buffers.current_spans.is_empty() {
                    line_source_map.push(state.current_source_line);
                    let line = Line::from(std::mem::take(&mut self.buffers.current_spans))
                        .style(state.code_bg_style);
                    lines.push(line);
                    state.cur_col_in_line = 0;
                }
            }

            state.last_pos = repl.range.end;
            let newlines_in_code =
                count_newlines_in_range(repl.range.start, repl.range.end, self.text);
            state.current_source_line = code_start_source_line + newlines_in_code;
            state.last_line_count_pos = repl.range.end;

            if state.checkpoint_output_lines.is_none() {
                if let Some((_, cp_byte)) = state.checkpoint_info {
                    if state.last_pos >= cp_byte {
                        state.checkpoint_output_lines = Some(lines.len());
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    // documented divergence: the upstream Table arm is one inseverable body; grouping it further would force renames deeper into the verbatim copy (see file header)
    /// Upstream `RenderEventKind::Table` arm of `render_ratatui`: flush one
    /// pre-rendered table (styled_lines, hyperlinks, cell copies) into the
    /// output buffers (verbatim modulo the `RenderLoopState` renames and the
    /// mechanical `continue`→`return`).
    pub(crate) fn flush_table(
        &mut self,
        state: &mut RenderLoopState,
        ev_index: usize,
        is_end: bool,
        pretty: bool,
        lines: &mut Vec<Line<'static>>,
        line_source_map: &mut Vec<usize>,
        hyperlinks: &mut Vec<HyperlinkTarget>,
        tables: &mut Vec<TableCopyMeta>,
    ) {
        if is_end && state.table_replace == Some(ev_index) {
            state.table_replace = None;
        } else if !is_end && state.table_replace.is_none() && pretty {
            let Some(trepl) = self.buffers.table_replaces.get(ev_index) else {
                return;
            };
            state.table_replace = Some(ev_index);

            // Tables always start at a line boundary, so for them this is a no-op
            // A display-math block replacement can occur mid-paragraph (`text $$x$$ more`)
            // Without the flush, the pending "text " spans would be emitted AFTER the block lines
            if !self.buffers.current_spans.is_empty() {
                line_source_map.push(state.current_source_line);
                lines.push(Line::from(std::mem::take(&mut self.buffers.current_spans)));
                // cur_col_in_line is reset unconditionally after the block lines are emitted below
            }

            // Update source line to table start
            if trepl.range.start > state.last_line_count_pos {
                state.current_source_line += count_newlines_in_range(
                    state.last_line_count_pos,
                    trepl.range.start,
                    self.text,
                );
            }
            let table_start_source_line = state.current_source_line;
            let table_base_line = lines.len();

            for (line_idx, styled_line) in trepl.styled_lines.iter().enumerate() {
                let offset = trepl
                    .line_source_offsets
                    .get(line_idx)
                    .copied()
                    .unwrap_or(0);
                state.current_source_line = table_start_source_line + offset;
                line_source_map.push(state.current_source_line);
                lines.push(styled_line.clone());
            }
            // Translate table-local hyperlink coordinates into absolute line indices and append to the global list
            for link in &trepl.hyperlinks {
                hyperlinks.push(HyperlinkTarget {
                    line_index: table_base_line + link.line_offset,
                    column_range: link.column_range.clone(),
                    url: link.url.clone(),
                    id: link.id,
                });
            }
            if trepl.n_cols > 0 {
                tables.push(TableCopyMeta {
                    line_index: table_base_line,
                    line_count: trepl.styled_lines.len(),
                    n_cols: trepl.n_cols,
                    cells: trepl.cell_copies.clone(),
                });
            }
            // Table emits whole pre-rendered lines; reset col so any subsequent inline content starts at column 0
            state.cur_col_in_line = 0;

            state.last_pos = trepl.range.end;
            let newlines_in_table =
                count_newlines_in_range(trepl.range.start, trepl.range.end, self.text);
            state.current_source_line = table_start_source_line + newlines_in_table;
            state.last_line_count_pos = trepl.range.end;

            if state.checkpoint_output_lines.is_none() {
                if let Some((_, cp_byte)) = state.checkpoint_info {
                    if state.last_pos >= cp_byte {
                        state.checkpoint_output_lines = Some(lines.len());
                    }
                }
            }
        }
    }

    /// Upstream `RenderEventKind::Mermaid` arm of `render_ratatui`: flush one
    /// rendered Mermaid diagram into `lines` (verbatim modulo the
    /// `RenderLoopState` renames and the mechanical `continue`→`return`).
    pub(crate) fn flush_mermaid(
        &mut self,
        state: &mut RenderLoopState,
        ev_index: usize,
        is_end: bool,
        pretty: bool,
        lines: &mut Vec<Line<'static>>,
        line_source_map: &mut Vec<usize>,
    ) {
        if is_end && state.mermaid_replace == Some(ev_index) {
            state.mermaid_replace = None;
        } else if !is_end && state.mermaid_replace.is_none() && pretty {
            let Some(mrepl) = self.buffers.mermaid_replaces.get(ev_index) else {
                return;
            };
            state.mermaid_replace = Some(ev_index);

            if mrepl.range.start > state.last_line_count_pos {
                state.current_source_line += count_newlines_in_range(
                    state.last_line_count_pos,
                    mrepl.range.start,
                    self.text,
                );
            }
            let start_source_line = state.current_source_line;

            for styled_line in &mrepl.styled_lines {
                line_source_map.push(start_source_line);
                lines.push(styled_line.clone());
            }
            state.cur_col_in_line = 0;

            state.last_pos = mrepl.range.end;
            let newlines = count_newlines_in_range(mrepl.range.start, mrepl.range.end, self.text);
            state.current_source_line = start_source_line + newlines;
            state.last_line_count_pos = mrepl.range.end;

            if state.checkpoint_output_lines.is_none() {
                if let Some((_, cp_byte)) = state.checkpoint_info {
                    if state.last_pos >= cp_byte {
                        state.checkpoint_output_lines = Some(lines.len());
                    }
                }
            }
        }
    }
}
