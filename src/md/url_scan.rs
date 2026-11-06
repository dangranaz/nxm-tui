//! Derived from xai-grok-markdown/src/url_scan.rs (xai-org/grok-build, Apache-2.0). Modified: tests reduced to those not depending on StreamingMarkdownRenderer (T16); fns widened pub(crate)→pub for `tests/` reach (RULES visibility-for-testability).

//! Plain-URL detection over rendered display ratatui Lines.

use std::ops::Range;

use linkify::{LinkFinder, LinkKind};
use ratatui::text::Line;

use crate::md::buffers::unicode_display_width;
use crate::md::output::HyperlinkTarget;

/// Scan `lines` for plain URLs and return new `HyperlinkTarget` entries that don't overlap any existing target in `existing`.
///
/// `next_id` is the first id to assign; the returned `u32` is the post-scan counter, suitable for stuffing back into `FrozenState::next_link_id`.
pub fn detect_plain_urls(
    lines: &[Line<'_>],
    existing: &[HyperlinkTarget],
    next_id: u32,
) -> (Vec<HyperlinkTarget>, u32) {
    detect_plain_urls_with_offset(lines, 0, existing, next_id)
}

/// Like [`detect_plain_urls`] but scans `lines` whose first element represents document line `line_index_offset`.
/// Lines fully inside `0..line_index_offset` are assumed to be in `existing` already and are not re-scanned.
/// The dedup overlap check still works because emitted targets use document-absolute `line_index = line_index_offset + i`.
pub fn detect_plain_urls_with_offset(
    lines: &[Line<'_>],
    line_index_offset: usize,
    existing: &[HyperlinkTarget],
    next_id: u32,
) -> (Vec<HyperlinkTarget>, u32) {
    let mut result = Vec::new();
    let mut current_id = next_id;

    for (i, line) in lines.iter().enumerate() {
        let line_index = line_index_offset + i;
        // Scan the joined line so a URL split across style spans
        // (pretty-mode link coloring) is one target, not a truncated prefix.
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        for_each_plain_link(&line_text, |range, url| {
            let col_start = unicode_display_width(line_text.get(..range.start).unwrap_or(""));
            let col_end =
                col_start + unicode_display_width(line_text.get(range.clone()).unwrap_or(""));

            // Dedup: skip if any existing or already-added target overlaps on the same line
            let overlaps = existing.iter().chain(result.iter()).any(|h| {
                h.line_index == line_index
                    && col_start < h.column_range.end
                    && h.column_range.start < col_end
            });

            if !overlaps {
                result.push(HyperlinkTarget {
                    line_index,
                    column_range: col_start..col_end,
                    url,
                    id: current_id,
                });
                current_id += 1;
            }
        });
    }

    (result, current_id)
}

/// Call `f` with the byte range and destination of every plain URL or email
/// in `text`. Emails become `mailto:`; scp remotes (`git@host:path`) are skipped.
pub fn for_each_plain_link(text: &str, mut f: impl FnMut(Range<usize>, String)) {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url, LinkKind::Email]);

    for link in finder.links(text) {
        let start = link.start();
        let end = link.end();
        if start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            continue;
        }
        let url = match link.kind() {
            LinkKind::Email => {
                if matches!(text.as_bytes().get(end), Some(b':' | b'/')) {
                    continue;
                }
                format!("mailto:{}", link.as_str())
            }
            _ => link.as_str().to_string(),
        };
        f(start..end, url);
    }
}
