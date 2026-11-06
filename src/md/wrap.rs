//! Derived from xai-grok-pager-render/src/render/wrapping.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; crate::md::line_utils paths; inline tests adapted in tests/.
//! Split part 1/3 (prod lines 10-237 + is_table_line/blockquote_prefix_len relocated from 283-328; pub(crate) for the split).
use ratatui::text::Line;
use std::borrow::Cow;
use std::ops::Range;
use textwrap::Options;
use unicode_width::UnicodeWidthChar;

/// Compute byte ranges for wrapped lines without trailing whitespace.
pub(crate) fn wrap_ranges_trim<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    let mut lines: Vec<Range<usize>> = Vec::new();
    for line in textwrap::wrap(text, opts).iter() {
        match line {
            Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                lines.push(start..end);
            }
            Cow::Owned(_) => panic!("wrap_ranges_trim: unexpected owned string"),
        }
    }
    lines
}

/// A segment of a match highlight on a single wrapped row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSegment {
    /// Wrapped row index (0-based).
    pub row: usize,
    /// Start display column within that row (0-based, unicode-width aware).
    pub col_start: usize,
    /// End display column (exclusive).
    pub col_end: usize,
}

/// One segment per wrapped row the match overlaps. Columns are display width, not byte offsets.
pub fn byte_range_to_row_cols(
    text: &str,
    wrap_ranges: &[Range<usize>],
    match_range: Range<usize>,
) -> Vec<HighlightSegment> {
    let mut segments = Vec::new();
    for (row, wr) in wrap_ranges.iter().enumerate() {
        // Intersect match_range with this row's byte range.
        let start = match_range.start.max(wr.start);
        let end = match_range.end.min(wr.end);
        if start < end {
            // Convert byte offsets within this row to display columns.
            let Some(row_text) = text.get(wr.start..wr.end) else {
                continue;
            };
            let col_start = byte_offset_to_display_col(row_text, start - wr.start);
            let col_end = byte_offset_to_display_col(row_text, end - wr.start);
            segments.push(HighlightSegment {
                row,
                col_start,
                col_end,
            });
        }
    }
    segments
}

/// Convert a byte offset within `text` to the corresponding display column.
pub(crate) fn byte_offset_to_display_col(text: &str, byte_offset: usize) -> usize {
    let mut col = 0usize;
    let mut bytes = 0usize;
    for ch in text.chars() {
        if bytes >= byte_offset {
            break;
        }
        bytes += ch.len_utf8();
        col += ch.width().unwrap_or(0);
    }
    col
}

/// Breakpoints must match visual rendering. FirstFit is greedy, so single-pass equals the two-stage wrap; a non-greedy change must mirror it.
#[allow(clippy::single_range_in_vec_init)] // Intentional: the single range is the full text, no wrapping
pub fn wrap_byte_ranges_matching(text: &str, width: usize) -> Vec<Range<usize>> {
    if width == 0 || text.is_empty() {
        return vec![0..text.len()];
    }

    // Must match the Options used by word_wrap_line_with_joiners (via RtOptions).
    // Blockquote lines get a subsequent_indent equal to the prefix, which reduces the effective width for continuation lines
    // Mirror that here so search-highlight breakpoints stay in sync with the visual rendering
    let bq_len = blockquote_prefix_len(text);
    let subsequent_width = if bq_len > 0 && bq_len < text.len() {
        match text.get(..bq_len) {
            Some(prefix) => {
                let prefix_display_width = prefix
                    .chars()
                    .map(|c| c.width().unwrap_or(0))
                    .sum::<usize>();
                width.saturating_sub(prefix_display_width).max(1)
            }
            None => width,
        }
    } else {
        width
    };

    // First line wraps at full width; remainder at reduced width for blockquotes.
    let first_line_opts = textwrap::Options::new(width)
        .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
        .word_splitter(textwrap::WordSplitter::HyphenSplitter)
        .break_words(true);
    let first_ranges = wrap_ranges_trim(text, first_line_opts);
    let Some(first) = first_ranges.first() else {
        return vec![0..text.len()];
    };

    let mut ranges = vec![first.clone()];
    let mut base = first.end;

    // Skip whitespace at the wrap boundary (mirrors word_wrap_line_with_joiners).
    let skip = match text.get(base..) {
        Some(rest) => rest.chars().take_while(|c| *c == ' ').count(),
        None => 0,
    };
    base = base.saturating_add(skip);

    if base < text.len() {
        let Some(remainder) = text.get(base..) else {
            return ranges;
        };
        let rem_opts = textwrap::Options::new(subsequent_width)
            .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
            .word_splitter(textwrap::WordSplitter::HyphenSplitter)
            .break_words(true);
        for r in wrap_ranges_trim(remainder, rem_opts) {
            ranges.push((r.start + base)..(r.end + base));
        }
    }

    ranges
}

/// Wrapping options with initial/subsequent indent support.
#[derive(Debug, Clone)]
pub struct RtOptions<'a> {
    /// The width in columns at which the text will be wrapped.
    pub width: usize,
    pub line_ending: textwrap::LineEnding,
    pub initial_indent: Line<'a>,
    pub subsequent_indent: Line<'a>,
    /// Allow long words to be broken if they cannot fit on a line.
    pub break_words: bool,
    pub wrap_algorithm: textwrap::WrapAlgorithm,
    /// The line breaking algorithm to use.
    pub word_separator: textwrap::WordSeparator,
    pub word_splitter: textwrap::WordSplitter,
}

impl From<usize> for RtOptions<'_> {
    fn from(width: usize) -> Self {
        RtOptions::new(width)
    }
}

#[allow(dead_code)]
impl<'a> RtOptions<'a> {
    pub fn new(width: usize) -> Self {
        RtOptions {
            width,
            line_ending: textwrap::LineEnding::LF,
            initial_indent: Line::default(),
            subsequent_indent: Line::default(),
            break_words: true,
            word_separator: textwrap::WordSeparator::new(),
            wrap_algorithm: textwrap::WrapAlgorithm::FirstFit,
            word_splitter: textwrap::WordSplitter::HyphenSplitter,
        }
    }

    pub fn line_ending(self, line_ending: textwrap::LineEnding) -> Self {
        RtOptions {
            line_ending,
            ..self
        }
    }

    pub fn width(self, width: usize) -> Self {
        RtOptions { width, ..self }
    }

    pub fn initial_indent(self, initial_indent: Line<'a>) -> Self {
        RtOptions {
            initial_indent,
            ..self
        }
    }

    pub fn subsequent_indent(self, subsequent_indent: Line<'a>) -> Self {
        RtOptions {
            subsequent_indent,
            ..self
        }
    }

    pub fn break_words(self, break_words: bool) -> Self {
        RtOptions {
            break_words,
            ..self
        }
    }

    pub fn word_separator(self, word_separator: textwrap::WordSeparator) -> RtOptions<'a> {
        RtOptions {
            word_separator,
            ..self
        }
    }

    pub fn wrap_algorithm(self, wrap_algorithm: textwrap::WrapAlgorithm) -> RtOptions<'a> {
        RtOptions {
            wrap_algorithm,
            ..self
        }
    }

    pub fn word_splitter(self, word_splitter: textwrap::WordSplitter) -> RtOptions<'a> {
        RtOptions {
            word_splitter,
            ..self
        }
    }
}

/// Never wrap tables: wrapping destroys column alignment. A leading `│` is a blockquote unless an interior `│` separates cells.
pub(crate) fn is_table_line(line: &Line<'_>) -> bool {
    let mut chars = line.spans.iter().flat_map(|s| s.content.chars());
    match chars.next() {
        // Box-drawing border characters (horizontal lines, corners, junctions): these are unambiguously table borders, never blockquote prefixes
        Some(ch)
            if ('\u{2500}'..='\u{257F}').contains(&ch) && ch != '\u{2502}' && ch != '\u{2503}' =>
        {
            true
        }
        // │ (U+2502) at line start is a table content row only if │ appears after the leading prefix region
        // Blockquote prefixes are only │ and spaces
        Some('\u{2502}') => {
            let mut in_prefix = true;
            for ch in chars {
                if in_prefix && (ch == '\u{2502}' || ch == ' ') {
                    continue;
                }
                in_prefix = false;
                if ch == '\u{2502}' {
                    return true;
                }
            }
            false
        }
        // ASCII table borders (TableBorders::ASCII uses '+' and '|')
        Some('|') => true,
        _ => false,
    }
}

/// Leading `│ ` sequences, else 0. Must agree with the selection layer's style-aware twin, which depends on re-injected prefix spans.
pub(crate) fn blockquote_prefix_len(flat: &str) -> usize {
    const BAR_BYTES: usize = '\u{2502}'.len_utf8(); // 3
    let mut len = 0;
    let mut chars = flat.chars();
    while let Some('\u{2502}') = chars.next() {
        if chars.next() == Some(' ') {
            len += BAR_BYTES + 1;
        } else {
            break;
        }
    }
    len
}
