//! Derived from xai-grok-markdown/src/streaming.rs (xai-org/grok-build, Apache-2.0). Modified: crate::md paths; split for the 300-line cap; render helper from crate::md (T14a); LatexDelimiterNormalizer is the stub (latex deferred); inline tests adapted in tests/.
//!
//! `rerender_tail` (metodo privato upstream in streaming.rs, righe 257-373) è
//! splittato in questo file per il cap 300. Visibilità allargata
//! private→pub(crate) per lo split (stesso pattern di `anstyle_to_ratatui_color`
//! nel T6d): `render`/`push_and_render` in `streaming.rs` la chiamano.

use crate::md::open_code_highlighter::OpenCodeHighlighter;
use crate::md::render_markdown_ratatui_with_link_id;
use crate::md::streaming::{FrozenState, StreamingMarkdownRenderer};
use crate::md::syntax::Syntect;

impl StreamingMarkdownRenderer {
    /// Internal: render the unfrozen tail and update frozen state.
    pub(crate) fn rerender_tail(&mut self, syntect: Option<&Syntect>) {
        // Truncate output to frozen state (discard stale tail)
        self.output.lines.truncate(self.frozen.lines_len);
        self.output.line_source_map.truncate(self.frozen.lines_len);
        // Discard stale tail hyperlinks (keep frozen ones)
        self.output
            .hyperlinks
            .retain(|h| h.line_index < self.frozen.lines_len);
        self.output
            .tables
            .retain(|t| t.line_index < self.frozen.lines_len);
        // Discard stale tail code-block spans, keeping frozen ones (those whose body lies entirely within the frozen prefix)
        // A still-open fence in the tail has no span, so spans become stable only once frozen
        self.output
            .code_blocks
            .retain(|cb| cb.output_line_range.end <= self.frozen.lines_len);

        // Render the tail (unfrozen portion) using reusable buffers.
        // When the frozen source ends without a trailing newline (e.g., a thematic break `---` at the end of a chunk), the tail can start with `\n`
        // That newline is the block-terminating newline the frozen block consumed; skip it to avoid a spurious blank line
        let mut tail_start = self.frozen.source_bytes;
        if tail_start > 0
            && self.source.as_bytes().get(tail_start - 1) != Some(&b'\n')
            && self.source.as_bytes().get(tail_start) == Some(&b'\n')
        {
            tail_start += 1;
        }
        let Some(tail) = self.source.get(tail_start..) else {
            return;
        };
        // Lazily create the incremental open-code cache once syntect is present.
        // It rebuilds itself on fence/offset change, so a stale cache from a previous tail (e.g. after a checkpoint advanced) is self-correcting.
        let open_code = match syntect {
            Some(syn) => Some(
                self.open_code
                    .get_or_insert_with(|| OpenCodeHighlighter::new(syn)),
            ),
            None => None,
        };
        let (tail_output, checkpoint, tail_next_link_id) = render_markdown_ratatui_with_link_id(
            tail,
            self.style,
            self.pretty,
            &mut self.buffers,
            syntect,
            self.max_table_width,
            self.frozen.next_link_id,
            self.collapse_soft_breaks,
            open_code,
        );

        // Append tail to output
        self.output.lines.extend(tail_output.lines);
        self.output
            .line_source_map
            .extend(tail_output.line_source_map);

        // Offset tail hyperlink line indices by frozen line count and append
        let frozen_lines = self.frozen.lines_len;
        self.output
            .hyperlinks
            .extend(tail_output.hyperlinks.into_iter().map(|mut h| {
                h.line_index += frozen_lines;
                h
            }));
        self.output
            .tables
            .extend(tail_output.tables.into_iter().map(|mut t| {
                t.line_index += frozen_lines;
                t
            }));

        // Append tail code-block spans, rebasing their tail-relative ranges to document coordinates, mirroring the hyperlink offsetting
        // Output lines shift by the frozen line count, source bytes by the tail's start offset
        self.output
            .code_blocks
            .extend(tail_output.code_blocks.into_iter().map(|mut cb| {
                cb.output_line_range.start += frozen_lines;
                cb.output_line_range.end += frozen_lines;
                cb.source_byte_range.start += tail_start;
                cb.source_byte_range.end += tail_start;
                cb
            }));

        // Detect plain URLs` suffix in pretty-mode markdown links, bare URLs in prose).
        // (a) Non-streaming callers` during session replay) never call `finish()`.
        // Without url_scan here their URLs would never become HyperlinkTargets
        let tail_lines = self.output.lines.get(frozen_lines..).unwrap_or(&[]);
        let (extra_links, post_scan_next_id) = crate::md::url_scan::detect_plain_urls_with_offset(
            tail_lines,
            frozen_lines,
            &self.output.hyperlinks,
            tail_next_link_id,
        );
        self.output.hyperlinks.extend(extra_links);

        // Sort hyperlinks by (line_index, column_range.start), matching the invariant `finish()` enforces
        // Downstream consumers (`map_hyperlinks_to_overlay`, link map builders) rely on that order
        self.output
            .hyperlinks
            .sort_by_key(|h| (h.line_index, h.column_range.start));

        // If checkpoint found, update frozen state.
        // The checkpoint's source_bytes is relative to the tail we rendered, so add tail_start
        // tail_start may be greater than frozen.source_bytes if we skipped a leading newline
        if let Some(cp) = checkpoint {
            self.frozen = FrozenState {
                lines_len: self.frozen.lines_len + cp.output_lines,
                source_bytes: tail_start + cp.source_bytes,
                next_link_id: post_scan_next_id,
            };
        }
    }
}
