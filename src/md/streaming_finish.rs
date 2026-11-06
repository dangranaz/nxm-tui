//! Derived from xai-grok-markdown/src/streaming.rs (xai-org/grok-build, Apache-2.0). Modified: crate::md paths; split for the 300-line cap; render helper from crate::md (T14a); LatexDelimiterNormalizer is the stub (latex deferred); inline tests adapted in tests/.
//!
//! `finish`/`finish_into_output` (corpo upstream streaming.rs righe 427-490) sono
//! splittati in questo file: `streaming.rs` con loro sforava il cap 300
//! (terzo step dello split, come previsto dal ticket: "se un file sfora 300,
//! spezza ancora"). Nessuna modifica al corpo.

use crate::md::buffers::MarkdownBuffers;
use crate::md::output::{MarkdownRenderOutput, MarkdownRenderView};
use crate::md::render_markdown_ratatui_with_link_id;
use crate::md::streaming::{FrozenState, StreamingMarkdownRenderer};
use crate::md::syntax::Syntect;

impl StreamingMarkdownRenderer {
    /// Finalize streaming with a full re-render.
    /// `render()` also runs the URL detector and sort, so `finish()` adds nothing those calls didn't already produce.
    pub fn finish(&mut self, syntect: Option<&Syntect>) -> MarkdownRenderView<'_> {
        // Flush any bytes the normalizer held back at the last chunk boundary (e.g. a trailing partial delimiter).
        // The full re-render must see the complete, normalized source
        let flushed = self.normalizer.finish();
        self.source.push_str(&flushed);

        // Do a full re-render of the entire source, preserving max_table_width.
        let mut buffers = MarkdownBuffers::new();
        let (full_output, _, full_next_link_id) = render_markdown_ratatui_with_link_id(
            &self.source,
            self.style,
            self.pretty,
            &mut buffers,
            syntect,
            self.max_table_width,
            // The full render restarts link IDs at 0, so our counter must also reset to the post-render value
            0,
            self.collapse_soft_breaks,
            // finish() is a full batch re-render: never use the incremental cache.
            None,
        );

        self.output = full_output;

        // Scan rendered lines for plain, non-md URLs that pulldown-cmark didn't emit as Tag::Link
        // Dedup against existing hyperlinks by (line_index, column_range) overlap to avoid double-linking
        let (extra_links, post_scan_next_id) = crate::md::url_scan::detect_plain_urls(
            &self.output.lines,
            &self.output.hyperlinks,
            full_next_link_id,
        );
        self.output.hyperlinks.extend(extra_links);

        // Sort hyperlinks by (line_index, column_range.start) so downstream consumers see a well-ordered list
        self.output
            .hyperlinks
            .sort_by_key(|h| (h.line_index, h.column_range.start));

        // Mark everything as frozen (streaming is complete)
        self.frozen = FrozenState {
            lines_len: self.output.lines.len(),
            source_bytes: self.source.len(),
            next_link_id: post_scan_next_id,
        };

        // Streaming is over: release the highlighter caches (open-block state and closed-fence memo) rather than keep them for the block's lifetime
        // They are lazily rebuilt if rendering ever resumes
        self.open_code = None;

        self.output.as_view()
    }

    /// Finalize streaming and return owned output.
    ///
    /// Combines `finish()` and `into_output()`: does a full re-render and returns the owned result.
    pub fn finish_into_output(mut self, syntect: Option<&Syntect>) -> MarkdownRenderOutput {
        self.finish(syntect);
        self.output
    }
}
