//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap (table formatting extracted here); Rust-2021: let-chains rewritten as nested `if`s; `sort_by` → `sort_by_key(Reverse)` for clippy parity (same ordering).
//!
//! [`format_table`] renders a buffered [`TableState`] into box-drawing lines.
//! `FormattedTable` stays defined in `parse_end.rs` (where the T5f split put
//! it); the parser destructures it there in `on_end` and pushes a
//! `TableReplace` into the buffers.

use ratatui::style::Stylize;
use ratatui::text::Line;
use unicode_segmentation::UnicodeSegmentation;

use crate::md::buffers::unicode_display_width;
use crate::md::buffers::StyledCell;
use crate::md::buffers_table::{TableCellCopy, TableHyperlink, TableState};
use crate::md::parse::MarkdownParser;
use crate::md::parse::StyleInto;
use crate::md::parse_end::FormattedTable;
use crate::md::parse_table_word::cell_word_separator;
use crate::md::style::TableBorders;

impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Format a buffered table into lines with box-drawing borders.
    pub(crate) fn format_table(&self, state: &TableState) -> FormattedTable {
        let borders = TableBorders::BOX;
        let padding = 1;

        // Style already adapted, no need to call adapt_style again
        let border_style: ratatui::style::Style = self.ms.rule.style_into().dim();

        let all_rows: Vec<&Vec<StyledCell>> = std::iter::once(&state.header)
            .chain(state.rows.iter())
            .filter(|r| !r.is_empty())
            .collect();

        if all_rows.is_empty() {
            return FormattedTable::default();
        }

        let num_cols = all_rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if num_cols == 0 {
            return FormattedTable::default();
        }

        let mut col_widths: Vec<usize> = vec![0; num_cols];
        for row in &all_rows {
            for (col_idx, cell) in row.iter().enumerate() {
                let text = cell.plain_text();
                let cell_width = text
                    .split('\n')
                    .map(unicode_display_width)
                    .max()
                    .unwrap_or(0);
                if let Some(width) = col_widths.get_mut(col_idx) {
                    *width = (*width).max(cell_width);
                }
            }
        }

        // Constrain column widths to fit within max_table_width if set.
        // = num_cols * (2 * padding + 1) + sum(col_width) + 2 - 1
        if let Some(max_width) = self.max_table_width {
            let overhead = num_cols * (2 * padding + 1) + 1; // borders + padding
            let content_budget = max_width.saturating_sub(overhead);
            let total_content: usize = col_widths.iter().sum();

            if total_content > content_budget && total_content > 0 {
                // Compute per-column minimum widths: the longest unbreakable word across all cells in each column
                // The word separator determines what counts as unbreakable (e.g. "LongalphaToken", "$145,000", "ID-AA1001").
                let mut min_col_widths: Vec<usize> = vec![1; num_cols];
                // Per-column hard floors: the widest single grapheme (0 for empty columns)
                // That is the narrowest width at which cell text can still reflow without losing content
                let mut hard_floors: Vec<usize> = vec![0; num_cols];
                for row in &all_rows {
                    for (col, cell) in row.iter().enumerate() {
                        if col >= num_cols {
                            break;
                        }
                        let text = cell.plain_text();
                        for word in cell_word_separator(&text) {
                            let w = unicode_display_width(word.word);
                            if let Some(min_w) = min_col_widths.get_mut(col) {
                                *min_w = (*min_w).max(w);
                            }
                        }
                        if !text.is_empty() {
                            if let Some(floor) = hard_floors.get_mut(col) {
                                let widest_grapheme = text
                                    .graphemes(true)
                                    .map(unicode_display_width)
                                    .max()
                                    .unwrap_or(0);
                                *floor = (*floor).max(widest_grapheme.max(1));
                            }
                        }
                    }
                }

                let min_total: usize = min_col_widths.iter().sum();
                let hard_total: usize = hard_floors.iter().sum();

                // Grow from the word minimums toward natural widths when the word minimums fit the budget
                // Otherwise restart from the grapheme floors
                // When even the grapheme floors cannot fit, keep the word minimums: downstream clipping remains the safety net
                let (base_widths, target_widths) =
                    if min_total > content_budget && hard_total <= content_budget {
                        (hard_floors, min_col_widths)
                    } else {
                        (min_col_widths, col_widths.clone())
                    };
                let base_total: usize = base_widths.iter().sum();
                let extra_budget = content_budget.saturating_sub(base_total);

                // How much each column *wants* above its base.
                let extra_wants: Vec<usize> = target_widths
                    .iter()
                    .zip(&base_widths)
                    .map(|(&target, &base)| target.saturating_sub(base))
                    .collect();
                let total_extra_want: usize = extra_wants.iter().sum();

                let mut new_widths = base_widths.clone();
                if total_extra_want > 0 && extra_budget > 0 {
                    // Distribute proportionally.
                    for (width, &want) in new_widths.iter_mut().zip(&extra_wants) {
                        let share = (want as f64 * extra_budget as f64 / total_extra_want as f64)
                            .floor() as usize;
                        *width += share;
                    }

                    // Hand out any remaining columns (from floor rounding) to columns with the most unmet want, one at a time
                    let used: usize = new_widths.iter().sum();
                    let mut remaining = content_budget.saturating_sub(used);
                    if remaining > 0 {
                        let mut indices: Vec<usize> = (0..num_cols).collect();
                        // Sort by unmet want descending.
                        let unmet = |i: usize| {
                            target_widths
                                .get(i)
                                .copied()
                                .unwrap_or(0)
                                .saturating_sub(new_widths.get(i).copied().unwrap_or(0))
                        };
                        indices.sort_by_key(|&i| std::cmp::Reverse(unmet(i)));
                        for &idx in &indices {
                            if remaining == 0 {
                                break;
                            }
                            // Don't grow beyond this pass's target width.
                            let target = target_widths.get(idx).copied().unwrap_or(0);
                            if let Some(width) = new_widths.get_mut(idx) {
                                if *width < target {
                                    *width += 1;
                                    remaining -= 1;
                                }
                            }
                        }
                    }
                }

                col_widths = new_widths;
            }
        }

        let alignments: Vec<_> = (0..num_cols)
            .map(|i| {
                state
                    .alignments
                    .get(i)
                    .copied()
                    .unwrap_or(pulldown_cmark::Alignment::None)
            })
            .collect();

        let mut lines = Vec::new();
        let mut styled_lines = Vec::new();
        let mut line_source_offsets: Vec<usize> = Vec::new();
        let mut hyperlinks: Vec<TableHyperlink> = Vec::new();
        let mut cell_copies: Vec<TableCellCopy> = Vec::new();

        // Source line layout within a table: offset 0: header row (| Col A | Col B |) offset 1: separator (|-------|-------|) offset 2+: body rows (| val1 | val2 |)
        let header_offset = 0usize;
        let separator_offset = 1usize;

        // Top border, belongs to the header line
        let top_border = self.format_border_line(
            &col_widths,
            padding,
            borders.c_tl(),
            borders.t_t(),
            borders.c_tr(),
            borders.h(),
        );
        styled_lines.push(Line::styled(top_border.clone(), border_style));
        lines.push(top_border);
        line_source_offsets.push(header_offset);

        // Header row
        if !state.header.is_empty() {
            let (row_plains, row_styleds, row_links, row_copies) = self
                .format_styled_content_lines(
                    &state.header,
                    &col_widths,
                    &alignments,
                    padding,
                    borders.v(),
                    border_style,
                    true,
                );
            cell_copies.extend(row_copies);
            let base_line = styled_lines.len();
            for (p, s) in row_plains.into_iter().zip(row_styleds) {
                lines.push(p);
                styled_lines.push(s);
                line_source_offsets.push(header_offset);
            }
            for mut link in row_links {
                link.line_offset += base_line;
                hyperlinks.push(link);
            }

            // Header separator
            let sep = self.format_border_line(
                &col_widths,
                padding,
                borders.t_l(),
                borders.x(),
                borders.t_r(),
                borders.h(),
            );
            styled_lines.push(Line::styled(sep.clone(), border_style));
            lines.push(sep);
            line_source_offsets.push(separator_offset);
        }

        // Body rows
        for (i, row) in state.rows.iter().enumerate() {
            let row_offset = separator_offset + 1 + i; // offset 2, 3, ...

            let (row_plains, row_styleds, row_links, row_copies) = self
                .format_styled_content_lines(
                    row,
                    &col_widths,
                    &alignments,
                    padding,
                    borders.v(),
                    border_style,
                    false,
                );
            cell_copies.extend(row_copies);
            let base_line = styled_lines.len();
            for (p, s) in row_plains.into_iter().zip(row_styleds) {
                lines.push(p);
                styled_lines.push(s);
                line_source_offsets.push(row_offset);
            }
            for mut link in row_links {
                link.line_offset += base_line;
                hyperlinks.push(link);
            }

            // Row divider between body rows (not after last row)
            if i < state.rows.len().saturating_sub(1) {
                let row_sep = self.format_border_line(
                    &col_widths,
                    padding,
                    borders.t_l(),
                    borders.x(),
                    borders.t_r(),
                    borders.h(),
                );
                styled_lines.push(Line::styled(row_sep.clone(), border_style));
                lines.push(row_sep);
                line_source_offsets.push(row_offset);
            }
        }

        // Bottom border, belongs to the last body row
        let last_row_offset = separator_offset + state.rows.len();
        let bottom_border = self.format_border_line(
            &col_widths,
            padding,
            borders.c_bl(),
            borders.t_b(),
            borders.c_br(),
            borders.h(),
        );
        styled_lines.push(Line::styled(bottom_border.clone(), border_style));
        lines.push(bottom_border);
        line_source_offsets.push(last_row_offset);

        FormattedTable {
            lines,
            styled_lines,
            line_source_offsets,
            hyperlinks,
            cell_copies,
            n_cols: num_cols,
        }
    }
}
