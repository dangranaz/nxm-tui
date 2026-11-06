//! Contract tests for `src/md/source_map.rs` (V3/T6a).
//! Cover the upstream `SourceMap` API: add/to_source mapping, unmapped ranges,
//! extend_with_offsets when merging maps, and len/is_empty/truncate/clear.

use nxm_tui::md::source_map::SourceMap;

/// `add` + `to_source`: a rendered range maps back to its source range.
#[test]
fn add_then_to_source_maps_back() {
    let mut map = SourceMap::new();
    map.add(0, 5..10);
    assert_eq!(
        map.to_source(0..5),
        Some(5..10),
        "exact segment must map 1:1"
    );
}

/// `to_source` returns `None` for ranges outside any mapped segment.
#[test]
fn to_source_none_for_unmapped() {
    let mut map = SourceMap::new();
    assert_eq!(map.to_source(0..3), None, "empty map has no mapping");

    // add(0, 5..10) → segment rendered 0..5 ↔ source 5..10
    map.add(0, 5..10);
    assert_eq!(map.to_source(2..4), Some(7..9), "inside the segment maps");
    assert_eq!(map.to_source(5..7), None, "after the mapped segment");

    // Gap between two segments resolves to None.
    map.add(10, 20..25); // segment rendered 10..15
    assert_eq!(map.to_source(5..10), None, "between the two segments");
}

/// `to_source` clamps partial overlaps inside a segment.
#[test]
fn to_source_partial_overlap_returns_slice_of_source() {
    let mut map = SourceMap::new();
    map.add(0, 10..20);
    // rendered 4..7 overlaps segment 0..10 at 4..7 → source 14..17
    assert_eq!(map.to_source(4..7), Some(14..17));
}

/// `extend_with_offsets` merges another map applying both offsets: the merged
/// map carries the expected segments.
#[test]
fn extend_with_offsets_combines_maps() {
    let mut head = SourceMap::new();
    head.add(0, 0..4);

    let mut tail = SourceMap::new();
    tail.add(0, 0..3);

    // Combine: tail rendered at 4.., tail source starts at byte 8.
    head.extend_with_offsets(&tail, 4, 8);

    assert_eq!(head.len(), 2, "head + tail segments");
    let segs = head.segments();
    assert_eq!(segs[1].0, 4..7, "tail rendered range + rendered_offset");
    assert_eq!(segs[1].1, 8..11, "tail source range + source_offset");

    // Both halves keep resolving through the merged map.
    assert_eq!(head.to_source(0..4), Some(0..4));
    assert_eq!(head.to_source(4..7), Some(8..11));
}

/// `len`/`is_empty`/`truncate`/`clear` stay coherent.
#[test]
fn len_is_empty_truncate_clear_are_coherent() {
    let mut map = SourceMap::new();
    assert!(map.is_empty(), "fresh map is empty");

    map.add(0, 0..2);
    map.add(5, 10..13);
    assert!(!map.is_empty());
    assert_eq!(map.len(), 2);

    map.truncate(1);
    assert_eq!(map.len(), 1, "truncate keeps the first n segments");

    map.clear();
    assert!(map.is_empty(), "clear empties the map");
    assert_eq!(map.len(), 0);
    assert_eq!(map.to_source(0..2), None, "cleared map resolves nothing");
}
