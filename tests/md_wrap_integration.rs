//! Integration tests for T10: `md::render` wraps the rendered output to the
//! pane width. Our own integration code (RULES piene) — assertions are
//! behavioural and calibrated on the observed V3 output, no invented values.
//!
//! Key regression guards:
//! - prose wider than the pane must wrap (was 1 row of 182 cols at width 40);
//! - GFM tables are never split by the wrap (`is_table_line` skips them);
//! - code-fence markers (``` rows) stay whole rows;
//! - styles survive the wrap.

use nxm_tui::md::render;

fn row_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn long_prose_fits_pane_width() {
    // ~180 display columns on a single row before T10 (verified by smoke).
    let prose = "Questa è una frase di prova deliberately molto lunga che supera ampiamente la larghezza del pannello e che oggi sborda nel terminale senza alcun wrapping applicato dal renderer V3.";
    let lines = render(prose, 40);
    assert!(
        lines.len() >= 2,
        "expected the prose to wrap, got {} rows",
        lines.len()
    );
    for line in &lines {
        assert!(
            line.width() <= 40,
            "row width {} exceeds max_width 40",
            line.width()
        );
    }
    // Word-boundary wrap: every word survives intact (none is cut mid-word
    // here, all words are shorter than the pane).
    let joined: String = lines.iter().map(|l| row_text(l)).collect();
    for word in ["Questa", "frase", "lunga", "terminale", "V3."] {
        assert!(joined.contains(word), "word {word:?} lost while wrapping");
    }
}

#[test]
fn short_prose_untouched() {
    let lines = render("short", 40);
    assert_eq!(lines.len(), 1, "a fitting line must not wrap");
    assert_eq!(lines[0].width(), 5);
    assert_eq!(row_text(&lines[0]), "short");
}

#[test]
fn table_rows_are_never_wrapped() {
    let lines = render("| a | b |\n|---|---|\n| 1 | 2 |", 80);
    // Observed V3 table shape: top/bottom borders, header and content rows.
    // If the wrap ever split a table row, the count would grow.
    assert_eq!(lines.len(), 5, "table rows must not be split by the wrap");
    let texts: Vec<String> = lines.iter().map(row_text).collect();
    let trimmed: Vec<&str> = texts.iter().map(|t| t.trim_end()).collect();
    assert_eq!(trimmed[0], "┌───┬───┐");
    assert_eq!(trimmed[1], "│ a │ b │");
    assert_eq!(trimmed[2], "├───┼───┤");
    assert_eq!(trimmed[3], "│ 1 │ 2 │");
    assert_eq!(trimmed[4], "└───┴───┘");
    for line in &lines {
        assert!(
            line.width() <= 80,
            "row width {} exceeds max_width 80",
            line.width()
        );
    }
}

#[test]
fn code_fence_rows_stay_whole() {
    let code = "```rust\nfn main() { let x = 12345; }\nlet very_long_line_that_exceeds_the_panel_width_for_sure_yes = 42;\n```";
    let lines = render(code, 40);
    assert!(
        lines.len() >= 4,
        "expected the fences plus wrapped code, got {} rows",
        lines.len()
    );
    // The ``` markers remain single, unsplit rows (observed output).
    assert_eq!(row_text(&lines[0]), "```rust");
    assert_eq!(row_text(&lines[lines.len() - 1]), "```");
    for line in &lines {
        assert!(
            line.width() <= 40,
            "row width {} exceeds max_width 40",
            line.width()
        );
    }
}

#[test]
fn styles_are_preserved_after_wrap() {
    // The V3 theme styles headings with an fg colour (no modifier); every
    // continuation row must carry the same heading colour after the wrap.
    let prose =
        "# Un titolo davvero molto lungo che supera di gran lunga i quaranta caratteri di larghezza del pannello";
    let lines = render(prose, 40);
    assert!(lines.len() >= 2, "expected the heading to wrap");
    let heading_fg = lines[0]
        .spans
        .last()
        .expect("heading row has spans")
        .style
        .fg;
    assert!(heading_fg.is_some(), "heading must be styled with an fg");
    for line in &lines[1..] {
        assert!(
            line.spans.iter().any(|s| s.style.fg == heading_fg),
            "heading style lost on a continuation row"
        );
    }
}
