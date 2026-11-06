//! Derived from xai-grok-pager-render/src/render/wrapping.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; crate::md::line_utils paths; inline tests adapted in tests/.
//! Split part 2/3 (prod lines 237-282, 330-509, 580-634): word_wrap_line(s)(_with_joiners) + span slicing.
//! Provides word-aware wrapping that preserves styled spans and tracks soft vs hard line breaks (via joiners).

use crate::md::line_utils::{fit_line_to_width, push_owned_lines};
use crate::md::wrap::{blockquote_prefix_len, is_table_line, wrap_ranges_trim, RtOptions};
use crate::md::wrap_input::IntoLineInput;
use ratatui::text::{Line, Span};
use std::borrow::Cow;
use std::ops::Range;
use textwrap::Options;

/// Wrap a single line, preserving styles.
#[must_use]
pub fn word_wrap_line<'a, O>(line: &'a Line<'a>, width_or_options: O) -> Vec<Line<'a>>
where
    O: Into<RtOptions<'a>>,
{
    let (lines, _joiners) = word_wrap_line_with_joiners(line, width_or_options);
    lines
}

fn flatten_line_and_bounds<'a>(
    line: &'a Line<'a>,
) -> (String, Vec<(Range<usize>, ratatui::style::Style)>) {
    let mut flat = String::new();
    let mut span_bounds = Vec::new();
    let mut acc = 0usize;
    for s in &line.spans {
        let text = s.content.as_ref();
        let start = acc;
        flat.push_str(text);
        acc += text.len();
        span_bounds.push((start..acc, s.style));
    }
    (flat, span_bounds)
}

fn build_wrapped_line_from_range<'a>(
    indent: Line<'a>,
    original: &'a Line<'a>,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    range: &Range<usize>,
    cursor: &mut usize,
) -> Line<'a> {
    let mut out = indent.style(original.style);
    let sliced = slice_line_spans(original, span_bounds, range, cursor);
    let mut spans = out.spans;
    spans.append(
        &mut sliced
            .spans
            .into_iter()
            .map(|s| s.patch_style(original.style))
            .collect(),
    );
    out.spans = spans;
    out
}
/// First line is `None`. Continuation joiners are the exact substring skipped at the wrap boundary (often spaces, possibly empty).
pub fn word_wrap_line_with_joiners<'a, O>(
    line: &'a Line<'a>,
    width_or_options: O,
) -> (Vec<Line<'a>>, Vec<Option<String>>)
where
    O: Into<RtOptions<'a>>,
{
    let mut rt_opts: RtOptions<'a> = width_or_options.into();

    // Never wrap tables. Pad to content width: terminal clipping desyncs on wider-than-measured glyphs and strands a ghost cell.
    if is_table_line(line) {
        let fitted = fit_line_to_width(line.clone(), rt_opts.width);
        return (vec![fitted], vec![None]);
    }

    let (flat, span_bounds) = flatten_line_and_bounds(line);

    // Blockquote lines start with │ (U+2502) prefix(es)
    // Set subsequent_indent to the prefix so continuation lines repeat the quote marker
    // Skip if the caller already set a subsequent_indent (caller intent wins).
    let bq_len = blockquote_prefix_len(&flat);
    if bq_len > 0 && bq_len < flat.len() && rt_opts.subsequent_indent.width() == 0 {
        let prefix = slice_line_spans(line, &span_bounds, &(0..bq_len), &mut 0);
        rt_opts = rt_opts.subsequent_indent(prefix);
    }

    let opts = Options::new(rt_opts.width)
        .line_ending(rt_opts.line_ending)
        .break_words(rt_opts.break_words)
        .wrap_algorithm(rt_opts.wrap_algorithm)
        .word_separator(rt_opts.word_separator)
        .word_splitter(rt_opts.word_splitter);

    let mut out: Vec<Line<'a>> = Vec::new();
    let mut joiners: Vec<Option<String>> = Vec::new();

    // The first output line uses the initial indent and a reduced available width.
    let initial_width_available = opts
        .width
        .saturating_sub(rt_opts.initial_indent.width())
        .max(1);
    let initial_wrapped = wrap_ranges_trim(&flat, opts.clone().width(initial_width_available));
    let Some(first_line_range) = initial_wrapped.first() else {
        out.push(rt_opts.initial_indent.clone());
        joiners.push(None);
        return (out, joiners);
    };

    // Rows are emitted in increasing byte order, so each row resumes the span scan where the previous one stopped instead of rescanning from 0
    // This shared cursor (see `slice_line_spans`) is what keeps wrapping one huge line linear
    let mut span_cursor = 0usize;

    let first_line = build_wrapped_line_from_range(
        rt_opts.initial_indent.clone(),
        line,
        &span_bounds,
        first_line_range,
        &mut span_cursor,
    );
    out.push(first_line);
    joiners.push(None);

    // Wrap the remainder using subsequent indent width.
    let mut base = first_line_range.end;
    let skip_leading_spaces = match flat.get(base..) {
        Some(rest) => rest.chars().take_while(|c| *c == ' ').count(),
        None => 0,
    };
    let joiner_end = base.saturating_add(skip_leading_spaces);
    let joiner_first = match flat.get(base..joiner_end) {
        Some(s) => s.to_string(),
        None => String::new(),
    };
    base = joiner_end;

    let subsequent_width_available = opts
        .width
        .saturating_sub(rt_opts.subsequent_indent.width())
        .max(1);
    let Some(remaining) = flat.get(base..) else {
        return (out, joiners);
    };
    let remaining_wrapped = wrap_ranges_trim(remaining, opts.width(subsequent_width_available));

    let mut prev_end = 0usize;
    for (i, r) in remaining_wrapped.iter().enumerate() {
        if r.is_empty() {
            continue;
        }

        let joiner = if i == 0 {
            joiner_first.clone()
        } else {
            match remaining.get(prev_end..r.start) {
                Some(s) => s.to_string(),
                None => String::new(),
            }
        };
        prev_end = r.end;

        let offset_range = (r.start + base)..(r.end + base);
        let subsequent_line = build_wrapped_line_from_range(
            rt_opts.subsequent_indent.clone(),
            line,
            &span_bounds,
            &offset_range,
            &mut span_cursor,
        );
        out.push(subsequent_line);
        joiners.push(Some(joiner));
    }

    (out, joiners)
}

/// Wrap a sequence of lines, applying the initial indent only to the very first output line.
/// All later wrapped pieces use the subsequent indent.
#[allow(private_bounds)]
pub fn word_wrap_lines<'a, I, O, L>(lines: I, width_or_options: O) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
    O: Into<RtOptions<'a>>,
{
    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'static>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            let mut o = base_opts.clone();
            let sub = o.subsequent_indent.clone();
            o = o.initial_indent(sub);
            o
        };
        let wrapped = word_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }

    out
}

/// Like `word_wrap_lines`, but also returns a parallel vector of soft-wrap joiners.
#[allow(private_bounds)]
pub fn word_wrap_lines_with_joiners<'a, I, O, L>(
    lines: I,
    width_or_options: O,
) -> (Vec<Line<'static>>, Vec<Option<String>>)
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
    O: Into<RtOptions<'a>>,
{
    let base_opts: RtOptions<'a> = width_or_options.into();
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut joiners: Vec<Option<String>> = Vec::new();

    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            let mut o = base_opts.clone();
            let sub = o.subsequent_indent.clone();
            o = o.initial_indent(sub);
            o
        };

        let (wrapped, wrapped_joiners) = word_wrap_line_with_joiners(line_input.as_ref(), opts);
        for (l, j) in wrapped.into_iter().zip(wrapped_joiners) {
            out.push(crate::md::line_utils::line_to_static(&l));
            joiners.push(j);
        }
    }

    (out, joiners)
}
/// Monotonic cursor so wrapping is O(rows + spans), not O(rows × spans) — that quadratic spun the render thread on one huge line.
/// Does not rewind: a later range must not start before a previous one.
fn slice_line_spans<'a>(
    original: &'a Line<'a>,
    span_bounds: &[(Range<usize>, ratatui::style::Style)],
    range: &Range<usize>,
    cursor: &mut usize,
) -> Line<'a> {
    let start_byte = range.start;
    let end_byte = range.end;

    // Spans ending at or before this row's start are done for every later row too (ranges are contiguous and queries monotonic)
    // So advance the shared cursor past them once and never revisit them
    while span_bounds
        .get(*cursor)
        .is_some_and(|(r, _)| r.end <= start_byte)
    {
        *cursor += 1;
    }

    let mut acc: Vec<Span<'a>> = Vec::new();
    for (i, (r, style)) in span_bounds.iter().enumerate().skip(*cursor) {
        let s = r.start;
        let e = r.end;
        if s >= end_byte {
            break;
        }
        let seg_start = start_byte.max(s);
        let seg_end = end_byte.min(e);
        if seg_end > seg_start {
            let local_start = seg_start - s;
            let local_end = seg_end - s;
            let Some(content) = original.spans.get(i).map(|s| s.content.as_ref()) else {
                continue;
            };
            let Some(slice) = content.get(local_start..local_end) else {
                continue;
            };
            acc.push(Span {
                style: *style,
                content: Cow::Borrowed(slice),
            });
        }
        if e >= end_byte {
            break;
        }
    }
    Line {
        style: original.style,
        alignment: original.alignment,
        spans: acc,
    }
}
