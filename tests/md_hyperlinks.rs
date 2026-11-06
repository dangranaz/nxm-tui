//! Contract tests for `src/md/hyperlinks.rs` (V3/T6c).
//! Cover the link projection ported from upstream `hyperlinks.rs`
//! (Apache-2.0): `chunk_link_offsets`, `emit_segment_hyperlinks`, and
//! `source_to_chunk_offset`, exercised directly by hand-building
//! `LinkTarget`s and chunk text (upstream inline tests all go through the
//! renderer; those are out of scope until T16).

use nxm_tui::md::buffers::LinkTarget;
use nxm_tui::md::hyperlinks::{
    chunk_link_offsets, emit_segment_hyperlinks, source_to_chunk_offset,
};

fn link(source_range: std::ops::Range<usize>, id: u32) -> LinkTarget {
    LinkTarget {
        source_range,
        url: "https://example.com/docs".into(),
        id,
    }
}

/// A `LinkTarget` whose source range sits inside the chunk projects to a
/// `ChunkLinkRange` with the expected transformed-byte bounds and link index;
/// a link outside the chunk is not projected.
#[test]
fn chunk_link_offsets_projects_link_inside_chunk() {
    let targets = &[link(7..12, 1), link(20..25, 2)];
    let ranges = chunk_link_offsets(targets, 0, 0, 12, false, &[]);
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].xform_start, 7);
    assert_eq!(ranges[0].xform_end, 12);
    assert_eq!(ranges[0].link_idx, 0);
}

/// `source_to_chunk_offset` without transforms is the identity mapping:
/// `src_pos - chunk_start`.
#[test]
fn source_to_chunk_offset_without_transforms_is_identity() {
    assert_eq!(source_to_chunk_offset(5, 0, &[]), 5);
    assert_eq!(source_to_chunk_offset(15, 10, &[]), 5);
    assert_eq!(source_to_chunk_offset(0, 0, &[]), 0);
}

/// An ASCII link projects onto the segment at display-cell columns equal to
/// its byte offsets (1 cell per glyph).
#[test]
fn emit_segment_hyperlinks_ascii_column_range() {
    let targets = vec![link(6..10, 7)];
    let chunk_links = chunk_link_offsets(&targets, 0, 0, 14, false, &[]);
    let mut out = Vec::new();
    emit_segment_hyperlinks(&chunk_links, &targets, "go to docs now", 0, 0, 0, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].line_index, 0);
    assert_eq!(out[0].column_range, 6..10);
    assert_eq!(out[0].url, "https://example.com/docs");
    assert_eq!(out[0].id, 7);
}

/// A segment with a CJK prefix counts 2 display cells per glyph: `col_start`
/// lands after the 2-glyph prefix (4 cells) + the space (1 cell).
#[test]
fn emit_segment_hyperlinks_cjk_prefix_counts_two_cells() {
    let targets = vec![link(7..12, 3)]; // "label" after "你好 " in the chunk
    let chunk_links = chunk_link_offsets(&targets, 0, 0, 12, false, &[]);
    let mut out = Vec::new();
    emit_segment_hyperlinks(&chunk_links, &targets, "你好 label", 0, 0, 0, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].column_range.start, 5); // 你(2) 好(2) space(1)
    assert_eq!(out[0].column_range.end, 10); // + width("label") = 5
}

/// A link whose text wraps across two segments (split on `\n`) emits one
/// `HyperlinkTarget` per segment, all sharing the same `id`.
#[test]
fn emit_segment_hyperlinks_wrapped_link_same_id_across_segments() {
    let targets = vec![link(0..12, 9)]; // "first\nsecond", no transforms
    let chunk_links = chunk_link_offsets(&targets, 0, 0, 12, false, &[]);
    let mut out = Vec::new();
    // Segment "first" at transformed-byte offset 0, rendered on line 0.
    emit_segment_hyperlinks(&chunk_links, &targets, "first", 0, 0, 0, &mut out);
    // Segment "second" at transformed-byte offset 6 ("first\n"), on line 1.
    emit_segment_hyperlinks(&chunk_links, &targets, "second", 6, 0, 1, &mut out);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, 9);
    assert_eq!(out[1].id, 9);
    assert_eq!(out[0].line_index, 0);
    assert_eq!(out[0].column_range, 0..5);
    assert_eq!(out[1].line_index, 1);
    assert_eq!(out[1].column_range, 0..6);
}
