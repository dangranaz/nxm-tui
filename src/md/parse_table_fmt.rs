//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap (row/border formatting extracted here); visibility-for-testability: `wrap_cell_text_joins` (from `pub(crate)`) and `format_border_line` (from private) widened to `pub` — tested directly in tests/md_parse_table.rs.

use unicode_segmentation::UnicodeSegmentation;

use crate::md::buffers::{floor_char_boundary, unicode_display_width, CellJoin, StyledCell};
use crate::md::buffers_table::{TableCellCopy, TableHyperlink};
use crate::md::parse::MarkdownParser;
use crate::md::parse::StyleInto;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};

impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Word-wrap a cell's plain text into lines of at most `width` display columns.
    /// Returns visual lines plus the exact omitted gap at each wrap (never empty).
    /// It never breaks mid-word.
    pub fn wrap_cell_text_joins(text: &str, width: usize) -> (Vec<String>, Vec<CellJoin>) {
        let lines = if width == 0 {
            vec![String::new()]
        } else {
            let opts = textwrap::Options::new(width)
                .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
                .word_separator(textwrap::WordSeparator::Custom(
                    crate::md::parse_table_word::cell_word_separator,
                ))
                .break_words(false);
            let wrapped = textwrap::wrap(text, opts);
            let mut lines: Vec<String> = Vec::with_capacity(wrapped.len());
            for cow in wrapped {
                let line = cow.into_owned();
                if unicode_display_width(&line) <= width {
                    lines.push(line);
                    continue;
                }
                // An unbreakable word survived textwrap wider than the column
                // break_words is off, and textwrap's char-based emergency split can tear grapheme clusters
                // Hard-split on grapheme boundaries using the same display-width model as the table formatter
                let mut piece = String::new();
                let mut piece_width = 0usize;
                for grapheme in line.graphemes(true) {
                    let grapheme_width = unicode_display_width(grapheme);
                    if piece_width > 0 && piece_width.saturating_add(grapheme_width) > width {
                        lines.push(std::mem::take(&mut piece));
                        piece_width = 0;
                    }
                    piece.push_str(grapheme);
                    piece_width = piece_width.saturating_add(grapheme_width);
                }
                if !piece.is_empty() {
                    lines.push(piece);
                }
            }
            if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            }
        };
        let joins = crate::md::parse_table_word::cell_wrap_joins(text, &lines);
        (lines, joins)
    }

    /// Format a table row that may span multiple visual lines (when cells wrap).
    /// There is one plain + styled entry per visual line, plus any hyperlinks discovered in cell spans.
    /// Hyperlink `line_offset`s are relative to the first visual line of this row (caller adds the absolute base to embed in the table).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn format_styled_content_lines(
        &self,
        cells: &[StyledCell],
        col_widths: &[usize],
        alignments: &[pulldown_cmark::Alignment],
        padding: usize,
        v: char,
        border_style: ratatui::style::Style,
        is_header: bool,
    ) -> (
        Vec<String>,
        Vec<Line<'static>>,
        Vec<TableHyperlink>,
        Vec<TableCellCopy>,
    ) {
        // 1. Wrap each cell's text into lines constrained to col_widths[i].
        let mut wrapped_cells = Vec::with_capacity(col_widths.len());
        let mut cell_copies = Vec::with_capacity(col_widths.len());
        for (i, &width) in col_widths.iter().enumerate() {
            let text = cells.get(i).map(|c| c.plain_text()).unwrap_or_default();
            let (lines, joins) = Self::wrap_cell_text_joins(&text, width);
            cell_copies.push(TableCellCopy { text, joins });
            wrapped_cells.push(lines);
        }

        // 2. Determine the number of visual lines (max wrapped lines across cells).
        let num_visual_lines = wrapped_cells.iter().map(|c| c.len()).max().unwrap_or(1);

        // 3. Build each visual line.
        let mut all_plains = Vec::with_capacity(num_visual_lines);
        let mut all_styled = Vec::with_capacity(num_visual_lines);
        let mut all_links: Vec<TableHyperlink> = Vec::new();

        // Monotonic per-column source cursors: each fragment is searched for strictly after the previous fragment's match end
        // Repeated substrings (a linked "aa" followed by a plain "aa") can never re-match earlier bytes once textwrap has eaten boundary whitespace
        let mut source_cursors: Vec<usize> = vec![0; col_widths.len()];

        for vis_line in 0..num_visual_lines {
            let mut plain = String::new();
            let mut spans: Vec<Span<'static>> = Vec::new();
            // Running display column on this visual line; used to record hyperlink column ranges in the table-local coordinate system
            let mut display_col: usize = 0;

            plain.push(v);
            spans.push(Span::styled(v.to_string(), border_style));
            display_col += unicode_display_width(&v.to_string());

            for (i, width) in col_widths.iter().enumerate() {
                let cell_line_text = wrapped_cells
                    .get(i)
                    .and_then(|c| c.get(vis_line))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let cell_line_width = unicode_display_width(cell_line_text);
                let total_padding = width.saturating_sub(cell_line_width);

                let alignment = alignments
                    .get(i)
                    .copied()
                    .unwrap_or(pulldown_cmark::Alignment::None);
                let (left_pad, right_pad) = match alignment {
                    pulldown_cmark::Alignment::Left | pulldown_cmark::Alignment::None => {
                        (0, total_padding)
                    }
                    pulldown_cmark::Alignment::Right => (total_padding, 0),
                    pulldown_cmark::Alignment::Center => {
                        let left = total_padding / 2;
                        (left, total_padding - left)
                    }
                };

                // Left padding
                let left_space = " ".repeat(padding + left_pad);
                let left_space_width = unicode_display_width(&left_space);
                plain.push_str(&left_space);
                spans.push(Span::raw(left_space));
                display_col += left_space_width;

                // Cell text: slice the original styled spans to match this visual line's character range
                // This preserves per-span formatting (bold, italic, strike, code, link) across wrap boundaries
                if !cell_line_text.is_empty() {
                    if let Some(cell) = cells.get(i) {
                        // Find the byte offset of this visual line within the full cell plain text, then emit styled spans covering that range
                        let full_text = cell.plain_text();
                        // Search from the previous fragment's match end
                        // Whitespace textwrap ate could otherwise let `.find` re-match an earlier overlapping occurrence of this fragment
                        let cursor = floor_char_boundary(
                            &full_text,
                            source_cursors.get(i).copied().unwrap_or(0),
                        );
                        let line_start = full_text
                            .get(cursor..)
                            .and_then(|rest| rest.find(cell_line_text))
                            .map(|off| cursor + off)
                            .unwrap_or(cursor);
                        let line_end = (line_start + cell_line_text.len()).min(full_text.len());
                        if let Some(next_cursor) = source_cursors.get_mut(i) {
                            *next_cursor = line_end;
                        }

                        // Walk the cell's spans, emitting the slice that overlaps [line_start..line_end]
                        let mut offset = 0usize;
                        for cell_span in &cell.spans {
                            let span_start = offset;
                            let span_end = offset + cell_span.text.len();
                            offset = span_end;

                            // Intersect [span_start..span_end] with [line_start..line_end]
                            let start = span_start.max(line_start);
                            let end = span_end.min(line_end);
                            if start >= end {
                                continue;
                            }

                            let Some(slice) = full_text.get(start..end) else {
                                continue;
                            };
                            if slice.is_empty() {
                                continue;
                            }

                            let mut style: ratatui::style::Style = self.ms.text.style_into();
                            if is_header || cell_span.bold {
                                style = style.bold();
                            }
                            if cell_span.italic {
                                style = style.italic();
                            }
                            if cell_span.code {
                                // Full replace; apply strike after this so ~~`code`~~ stays crossed out.
                                style = self.ms.inline_code_inner.style_into();
                            }
                            if cell_span.strike {
                                style = style.crossed_out();
                            }
                            if let Some((url, id)) = &cell_span.link {
                                // Apply link styling additively (preserves bold/italic if combined)
                                // link_text style typically adds underline + accent color so the cell visually matches paragraph link rendering
                                let link_style: ratatui::style::Style =
                                    self.ms.link_text.style_into();
                                style = style.patch(link_style);

                                let slice_width = unicode_display_width(slice);
                                all_links.push(TableHyperlink {
                                    line_offset: vis_line,
                                    column_range: display_col..(display_col + slice_width),
                                    url: url.clone(),
                                    id: *id,
                                });
                            }
                            let slice_width = unicode_display_width(slice);
                            plain.push_str(slice);
                            spans.push(Span::styled(slice.to_string(), style));
                            display_col += slice_width;
                        }
                    } else {
                        plain.push_str(cell_line_text);
                        spans.push(Span::raw(cell_line_text.to_string()));
                        display_col += cell_line_width;
                    }
                }

                // Right padding
                let right_space = " ".repeat(right_pad + padding);
                let right_space_width = unicode_display_width(&right_space);
                plain.push_str(&right_space);
                spans.push(Span::raw(right_space));
                display_col += right_space_width;

                // Column separator
                plain.push(v);
                spans.push(Span::styled(v.to_string(), border_style));
                display_col += unicode_display_width(&v.to_string());
            }

            all_plains.push(plain);
            all_styled.push(Line::from(spans));
        }

        (all_plains, all_styled, all_links, cell_copies)
    }

    pub fn format_border_line(
        &self,
        col_widths: &[usize],
        padding: usize,
        left: char,
        mid: char,
        right: char,
        h: char,
    ) -> String {
        let mut line = String::new();
        line.push(left);
        for (i, &width) in col_widths.iter().enumerate() {
            let total_width = width + padding * 2;
            for _ in 0..total_width {
                line.push(h);
            }
            if i < col_widths.len() - 1 {
                line.push(mid);
            }
        }
        line.push(right);
        line
    }
}
