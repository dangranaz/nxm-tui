//! Contract tests for `src/md/buffers.rs` and `src/md/buffers_table.rs` (V3/T4).
//! Copied from `xai-grok-markdown/src/buffers.rs` (Apache-2.0).

use nxm_tui::md::buffers::{
    unicode_display_width, CellJoin, CellSpan, MarkdownBuffers, StyledCell,
};
use nxm_tui::md::buffers_table::TableState;
use pulldown_cmark::Alignment;

/// `MarkdownBuffers::new()` + `clear()` funzionano.
#[test]
fn markdown_buffers_new_and_clear() {
    let mut buf = MarkdownBuffers::new();
    assert!(buf.highlights.is_empty());
    assert!(buf.replaces.is_empty());
    assert!(buf.transforms.is_empty());
    assert!(buf.table_replaces.is_empty());
    assert!(buf.mermaid_replaces.is_empty());
    assert!(buf.link_targets.is_empty());
    assert!(buf.code_blocks.is_empty());
    assert!(buf.render_events.is_empty());
    assert!(buf.current_spans.is_empty());
    assert!(buf.active_highlights.is_empty());

    // Aggiungi e poi clear
    buf.highlights.push(nxm_tui::md::buffers::Highlight {
        style: None,
        range: 0..5,
    });
    assert_eq!(buf.highlights.len(), 1);
    buf.clear();
    assert!(buf.highlights.is_empty());
}

/// `StyledCell::new()`/`plain_text()`/`clear()`.
#[test]
fn styled_cell_new_plain_text_clear() {
    let mut cell = StyledCell::new();
    assert!(cell.spans.is_empty());
    assert_eq!(cell.plain_text(), "");

    cell.spans.push(CellSpan::new(
        "hello".to_string(),
        true,
        false,
        false,
        false,
        None,
    ));
    cell.spans.push(CellSpan::new(
        " world".to_string(),
        false,
        true,
        false,
        false,
        None,
    ));
    assert_eq!(cell.plain_text(), "hello world");

    cell.clear();
    assert!(cell.spans.is_empty());
    assert_eq!(cell.plain_text(), "");
}

/// `unicode_display_width`: ascii = len, CJK/emoji = 2 celle.
#[test]
fn unicode_display_width_ascii_and_cjk() {
    assert_eq!(unicode_display_width("hello"), 5);
    assert_eq!(unicode_display_width("日本"), 4); // CJK = 2 celle ciascuno
    assert_eq!(unicode_display_width("a日本b"), 6); // 1 + 2 + 2 + 1
}

/// `TableState::new(...)` + `push_text`.
#[test]
fn table_state_new_and_push_text() {
    let mut state = TableState::new(vec![Alignment::Left, Alignment::Center], 0);
    assert_eq!(state.alignments.len(), 2);
    assert!(state.header.is_empty());
    assert!(state.rows.is_empty());
    assert!(!state.in_header);

    state.in_header = true;
    state.cell_bold = true;
    state.push_text("ciao");
    assert_eq!(state.current_cell.spans.len(), 1);
    assert_eq!(state.current_cell.plain_text(), "ciao");
    assert!(state.current_cell.spans[0].bold);
    assert!(!state.current_cell.spans[0].italic);
}

/// `CellJoin` enum variants.
#[test]
fn cell_join_variants() {
    let tight = CellJoin::Tight;
    let gap = CellJoin::Gap(" ".to_string());
    match tight {
        CellJoin::Tight => {}
        CellJoin::Gap(_) => panic!("expected Tight"),
    }
    match gap {
        CellJoin::Tight => panic!("expected Gap"),
        CellJoin::Gap(g) => assert_eq!(g, " "),
    }
}
