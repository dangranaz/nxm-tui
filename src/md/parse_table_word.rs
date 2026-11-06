//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap (cell wrap helpers extracted here); visibility-for-testability: `cell_wrap_joins` widened from private to `pub` (tested directly in tests/md_parse_table.rs).

use std::ops::Range;

use crate::md::buffers::{unicode_display_width, CellJoin};

/// URLs (validated via `url::Url::parse`) are treated as unbreakable words.
/// No break points are placed within a URL, so terminal hyperlink detection (Cmd+Click) keeps working when cells wrap.
pub(crate) fn cell_word_separator<'a>(
    line: &'a str,
) -> Box<dyn Iterator<Item = textwrap::core::Word<'a>> + 'a> {
    // Pass 1: find break-point byte positions.
    // A break point sits between a punctuation/symbol char and the alphabetic char that follows it
    // We record (break_byte_idx, punct_byte_start) break_byte_idx is where the next word would start if the punct char attaches to the left punct_byte_start is where the punct char begins, for attaching it to the right instead
    let mut breaks: Vec<(usize, usize)> = Vec::new();
    {
        let mut in_whitespace = false;
        let mut after_break_char = false;
        let mut prev_is_digit = false; // was the *previous* char a digit?
        let mut digit_before_break = false; // was the char before the break char a digit?
        let mut last_break_ch: char = '\0';
        let mut break_char_start: usize = 0;
        for (idx, ch) in line.char_indices() {
            let is_space = ch == ' ';
            let is_break_char = !is_space && !ch.is_alphanumeric();

            // After a break char, decide if we should split here.
            // Two cases allow a break: a) Followed by a letter: always break (new word boundary) b) digit-punct-digit: break, unless the punct is `,` or `.` (number formatting like `$145,000` or `3.14`)
            // `foo/bar` breaks (letter after punct); `555-0101` breaks (digit-hyphen-digit); `$145,000` stays (digit-comma-digit); `$145` stays (no digit before `$`); `EMP-1001` breaks at hyphen (letter before it).
            let should_break = if in_whitespace && !is_space {
                true
            } else if after_break_char {
                if ch.is_alphabetic() {
                    true
                } else if ch.is_ascii_digit() && digit_before_break {
                    // digit-punct-digit: only break for non-formatting punct
                    last_break_ch != ',' && last_break_ch != '.'
                } else {
                    false
                }
            } else {
                false
            };

            if should_break {
                if in_whitespace {
                    breaks.push((idx, idx));
                } else {
                    breaks.push((idx, break_char_start));
                }
            }

            if is_break_char {
                break_char_start = idx;
                last_break_ch = ch;
                digit_before_break = prev_is_digit;
            }
            prev_is_digit = ch.is_ascii_digit();
            in_whitespace = is_space;
            after_break_char = is_break_char;
        }
    }

    // Filter out break points that fall inside a URL.
    // Each whitespace-delimited token is tested with `url::Url::parse`; tokens that parse as valid URLs are protected from splitting
    let url_ranges: Vec<Range<usize>> = {
        let mut ranges = Vec::new();
        let mut pos = 0;
        for token in line.split_whitespace() {
            let Some(rel) = line.get(pos..).and_then(|rest| rest.find(token)) else {
                break;
            };
            let start = rel + pos;
            let end = start + token.len();
            if url::Url::parse(token).is_ok() {
                ranges.push(start..end);
            }
            pos = end;
        }
        ranges
    };
    breaks.retain(|&(break_pos, _)| {
        !url_ranges
            .iter()
            .any(|r| break_pos > r.start && break_pos < r.end)
    });

    // Pass 2: decide attachment for each break point.
    // For punct breaks, choose the side that minimizes max(left_len, right_len).
    let mut split_positions: Vec<usize> = Vec::with_capacity(breaks.len());
    {
        let len = line.len();
        for (i, &(attach_left, attach_right)) in breaks.iter().enumerate() {
            if attach_left == attach_right {
                // Whitespace break, no choice
                split_positions.push(attach_left);
            } else {
                // Determine segment boundaries for this break.
                let seg_start = if i == 0 {
                    0
                } else {
                    match i
                        .checked_sub(1)
                        .and_then(|j| split_positions.get(j))
                        .copied()
                    {
                        Some(s) => s,
                        None => continue,
                    }
                };
                let seg_end = if i < breaks.len() - 1 {
                    match breaks.get(i + 1).map(|&(end, _)| end) {
                        Some(e) => e,
                        None => len,
                    }
                } else {
                    len
                };

                let Some(left_if_attach_left) =
                    line.get(seg_start..attach_left).map(unicode_display_width)
                else {
                    continue;
                };
                let Some(right_if_attach_left) =
                    line.get(attach_left..seg_end).map(unicode_display_width)
                else {
                    continue;
                };
                let max_attach_left = left_if_attach_left.max(right_if_attach_left);

                let Some(left_if_attach_right) =
                    line.get(seg_start..attach_right).map(unicode_display_width)
                else {
                    continue;
                };
                let Some(right_if_attach_right) =
                    line.get(attach_right..seg_end).map(unicode_display_width)
                else {
                    continue;
                };
                let max_attach_right = left_if_attach_right.max(right_if_attach_right);

                if max_attach_right < max_attach_left {
                    split_positions.push(attach_right);
                } else {
                    split_positions.push(attach_left);
                }
            }
        }
    }

    // Pass 3: emit Words at the chosen split positions.
    let mut pos = 0usize;
    let mut idx = 0usize;
    Box::new(std::iter::from_fn(move || {
        if pos >= line.len() {
            return None;
        }
        let end = if idx < split_positions.len() {
            let e = *split_positions.get(idx)?;
            idx += 1;
            e
        } else {
            line.len()
        };
        let slice = line.get(pos..end)?;
        let word = textwrap::core::Word::from(slice);
        pos = end;
        Some(word)
    }))
}

pub fn cell_wrap_joins(source: &str, lines: &[String]) -> Vec<CellJoin> {
    let want = lines.len().saturating_sub(1);
    if want == 0 {
        return Vec::new();
    }
    let mut joins = Vec::with_capacity(want);
    let mut cursor = 0usize;
    let mut prev_end: Option<usize> = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if prev_end.is_some() {
                joins.push(CellJoin::Tight);
            }
            continue;
        }
        let Some(rest) = source.get(cursor..) else {
            if prev_end.is_some() {
                joins.push(CellJoin::Tight);
            }
            continue;
        };
        let Some(off) = rest.find(trimmed) else {
            if prev_end.is_some() {
                joins.push(CellJoin::Tight);
            }
            continue;
        };
        let start = cursor + off;
        let end = start + trimmed.len();
        if let Some(prev) = prev_end {
            if prev <= start
                && start <= source.len()
                && source.is_char_boundary(prev)
                && source.is_char_boundary(start)
            {
                if start > prev {
                    match source.get(prev..start) {
                        Some(gap) => joins.push(CellJoin::Gap(gap.to_string())),
                        None => joins.push(CellJoin::Tight),
                    }
                } else {
                    joins.push(CellJoin::Tight);
                }
            } else {
                joins.push(CellJoin::Tight);
            }
        }
        prev_end = Some(end);
        cursor = end;
    }
    joins.resize(want, CellJoin::Tight);
    joins.truncate(want);
    joins
}
