//! Contract tests for `src/md/parse_table.rs` / `parse_table_fmt.rs` /
//! `parse_table_word.rs` (V3/T5g).
//! Adapted from `xai-grok-markdown/src/parse.rs` (Apache-2.0):
//! `wrap_cell_joins_tests` (zebraone_space_before_slash, foo_bar_punct_wrap,
//! hard_split_url, likes_long_walks) plus new tickets scenarios
//! (parse → TableReplace, border chars, max_table_width cap).

use unicode_width::UnicodeWidthStr;

use nxm_tui::md::buffers::{CellJoin, MarkdownBuffers};
use nxm_tui::md::parse::MarkdownParser;
use nxm_tui::md::style::{MarkdownStyle, TableBorders};

/// Reassemble the original text from visual lines + joins (upstream helper).
fn reconstruct(lines: &[String], joins: &[CellJoin]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            if let Some(CellJoin::Gap(s)) = joins.get(i - 1) {
                out.push_str(s);
            }
        }
        out.push_str(line.trim());
    }
    out
}

/// `parse()` on a GFM table populates the buffers with a `TableReplace`
/// carrying the rendered box-drawing table.
#[test]
fn parse_gfm_table_populates_table_replaces() {
    let src = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parsed = MarkdownParser::new(src, style, &mut buffers, None).parse();

    assert_eq!(parsed.text, src);
    assert_eq!(
        parsed.buffers.table_replaces.len(),
        1,
        "a GFM table must produce exactly one TableReplace"
    );
    let table = &parsed.buffers.table_replaces[0];
    assert!(
        !table.lines.is_empty(),
        "formatted table lines must be non-empty"
    );
    assert_eq!(table.n_cols, 2, "two GFM header cells");
    // Box-drawing borders are present in the plain lines.
    assert!(
        table.lines.iter().any(|l| l.contains('│')),
        "need a `│` border char in {lines:?}",
        lines = table.lines
    );
    // The last line is the bottom border: └ … ┘
    let last = table.lines.last().expect("bottom border line");
    assert!(
        last.starts_with('└') && last.ends_with('┘'),
        "bottom border suspicious: {last}"
    );
    // A plain cell value survives in the body lines.
    assert!(
        table.lines.iter().any(|l| l.contains("1")),
        "cell `1` must appear"
    );
}

/// `format_border_line` produces the correct border chars via `TableBorders`.
#[test]
fn format_border_line_produces_box_chars() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let parser = MarkdownParser::new("", style, &mut buffers, None);

    let b = TableBorders::BOX;
    let line = parser.format_border_line(&[3, 2], 1, b.c_tl(), b.t_t(), b.c_tr(), b.h());
    // width 3 → 3+2*1 h chars, mid char, width 2 → 2+2 h chars.
    assert_eq!(line, "┌─────┬────┐");

    let ascii = TableBorders::ASCII;
    let line =
        parser.format_border_line(&[1], 1, ascii.c_tl(), ascii.t_t(), ascii.c_tr(), ascii.h());
    assert_eq!(line, "+---+");
}

/// Upstream `zebraone_space_before_slash_is_gap`, adapted: when a line wraps
/// right before a `/`-leading fragment, the join keeps the omitted space.
#[test]
fn zebraone_space_before_slash_is_gap() {
    let src = "ZebraOne / BravoCat / Quick Fox";
    let mut saw_slash_wrap = false;
    for width in 8..src.len() {
        let (lines, joins) = MarkdownParser::wrap_cell_text_joins(src, width);
        if lines.len() <= 1 {
            continue;
        }
        assert_eq!(joins.len(), lines.len() - 1, "width={width}");
        assert_eq!(
            reconstruct(&lines, &joins),
            src,
            "width={width} lines={lines:?} joins={joins:?}"
        );
        for (i, join) in joins.iter().enumerate() {
            if lines
                .get(i + 1)
                .is_some_and(|l| l.trim_start().starts_with('/'))
            {
                assert!(
                    matches!(join, CellJoin::Gap(g) if g.contains(' ')),
                    "width={width} join before slash line must keep the space, \
                     join={join:?} lines={lines:?}"
                );
                saw_slash_wrap = true;
            }
        }
    }
    assert!(
        saw_slash_wrap,
        "expected some width to wrap onto a slash-leading line"
    );
}

/// Upstream `foo_bar_punct_wrap_is_tight`, adapted: punctuation wraps join Tight.
#[test]
fn foo_bar_punct_wrap_is_tight() {
    let src = "foo/bar";
    let (lines, joins) = MarkdownParser::wrap_cell_text_joins(src, 4);
    assert!(lines.len() > 1, "expected punct wrap, got {lines:?}");
    assert_eq!(joins.len(), lines.len().saturating_sub(1));
    assert_eq!(reconstruct(&lines, &joins), src);
    assert!(
        joins.iter().all(|j| matches!(j, CellJoin::Tight)),
        "punct wrap must be Tight, joins={joins:?} lines={lines:?}"
    );
}

/// Upstream `hard_split_url_is_all_tight`, adapted: URLs are unbreakable words;
/// a hard split across a URL joins Tight.
#[test]
fn hard_split_url_is_all_tight() {
    let src = "https://github.com/long/org/repo/pull/9";
    let (lines, joins) = MarkdownParser::wrap_cell_text_joins(src, 20);
    assert!(lines.len() > 1, "expected hard split, got {lines:?}");
    assert_eq!(joins.len(), lines.len().saturating_sub(1));
    assert_eq!(reconstruct(&lines, &joins), src);
    assert!(
        joins.iter().all(|j| matches!(j, CellJoin::Tight)),
        "hard-split must be Tight, joins={joins:?} lines={lines:?}"
    );
    assert_eq!(
        nxm_tui::md::parse_table_word::cell_wrap_joins(src, &lines),
        joins
    );
}

/// Upstream `likes_long_walks_gap_is_space`, adapted: prose wraps are Gap(" ").
#[test]
fn likes_long_walks_gap_is_space() {
    let src = "likes long walks";
    let (lines, joins) = MarkdownParser::wrap_cell_text_joins(src, 8);
    assert!(lines.len() > 1, "expected prose wrap, got {lines:?}");
    assert_eq!(joins.len(), lines.len().saturating_sub(1));
    assert_eq!(reconstruct(&lines, &joins), src);
    assert!(
        joins
            .iter()
            .all(|j| matches!(j, CellJoin::Gap(g) if g == " ")),
        "prose joins must be Gap(\" \"), joins={joins:?} lines={lines:?}"
    );
}

/// A table wider than `max_table_width` is constrained: every rendered line
/// fits within the width budget (display columns, not bytes).
#[test]
fn max_table_width_constrains_table_lines() {
    let src = "| alpha bravo charlie delta echo | foxtrot golf hotel india juliet |\n\
               |---|---|\n\
               | one long cell here indeed | another long body cell right |\n\
               | second row wide too | more content to wrap and shrink |\n";
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parsed = MarkdownParser::new(src, style, &mut buffers, None)
        .max_table_width(Some(30))
        .parse();

    let table = parsed
        .buffers
        .table_replaces
        .first()
        .cloned()
        .expect("table must produce a TableReplace");

    assert!(
        !table.lines.is_empty(),
        "capped table must still render lines"
    );
    for line in &table.lines {
        assert!(
            line.width() <= 30,
            "expected line to fit width 30, got {width}: {line}",
            width = line.width()
        );
    }
}
