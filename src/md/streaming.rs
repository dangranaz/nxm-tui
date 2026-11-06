//! Streaming/incremental markdown renderer.
//!
//! Provides `StreamingMarkdownRenderer`, which renders markdown that arrives in chunks (e.g., from an LLM streaming response).
//!
//! # How It Works
//!
//! Instead of re-rendering the entire document on each chunk, it:
//! 1. Accumulates incoming chunks into an internal buffer
//! 2. Detects "checkpoints", stable block boundaries where output won't change
//! 3. Freezes rendered output up to the last checkpoint
//! 4. Only re-renders the "tail" after the checkpoint
//!
//! This reduces complexity from O(N²) to approximately O(N) for streaming.
//!
//! # Example
//!
//! ```ignore
//! let mut renderer = StreamingMarkdownRenderer::new(style, true);
//!
//! // As tokens arrive from LLM:
//! for token in stream {
//!     renderer.push_and_render(&token, Some(&syntect));
//!     let view = renderer.view();
//!     display(view.lines);
//! }
//! ```
//!
//! Derived from xai-grok-markdown/src/streaming.rs (xai-org/grok-build, Apache-2.0). Modified: crate::md paths; split for the 300-line cap; render helper from crate::md (T14a); LatexDelimiterNormalizer is the stub (latex deferred); inline tests adapted in tests/. Struct fields widened private→pub(crate) for the split (rerender_tail lives in streaming_tail.rs, same pattern as the T6d render split); upstream `#[cfg(test)]` items (count_trailing_blank_lines, cfg(test) uses) NOT ported — headless tests in tests/md_streaming.rs.

use crate::md::buffers::MarkdownBuffers;
use crate::md::latex::LatexDelimiterNormalizer;
use crate::md::open_code_highlighter::OpenCodeHighlighter;
use crate::md::output::{MarkdownRenderOutput, MarkdownRenderView};
use crate::md::style::MarkdownStyle;
use crate::md::syntax::Syntect;

/// Tracks the frozen state for truncation.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FrozenState {
    /// Number of frozen lines (= number of frozen line_source_map entries).
    pub(crate) lines_len: usize,
    /// Number of frozen source bytes.
    pub(crate) source_bytes: usize,
    /// Next link ID, advanced ONLY when a checkpoint advances the frozen boundary (i.e. when frozen lines and their hyperlinks become permanent).
    /// IDs assigned to url_scan hits in the unfrozen tail are regenerated on every `rerender_tail` call.
    /// They only become stable once the line they live on becomes frozen.
    pub(crate) next_link_id: u32,
}

/// Incremental markdown renderer that efficiently handles streaming input.
///
/// Maintains frozen (stable) content and only re-renders the unfrozen tail, reducing render time for long streaming content.
pub struct StreamingMarkdownRenderer {
    /// Accumulated source text (all chunks concatenated).
    pub(crate) source: String,

    /// Single output buffer: frozen content at start, tail appended after.
    pub(crate) output: MarkdownRenderOutput,

    /// Frozen state: where to truncate before re-rendering tail.
    pub(crate) frozen: FrozenState,

    /// Reusable buffers for highlighting and rendering (avoids allocation per render).
    pub(crate) buffers: MarkdownBuffers,

    /// Rendering style.
    pub(crate) style: MarkdownStyle,

    /// Whether to use pretty mode (hide markdown syntax).
    pub(crate) pretty: bool,

    /// Maximum width for rendered tables (in display columns).
    pub(crate) max_table_width: Option<usize>,

    /// Whether CommonMark soft breaks collapse to a space (default `true`).
    /// Set `false` for source-faithful rendering (plan preview).
    pub(crate) collapse_soft_breaks: bool,

    /// Incremental highlighter for the trailing still-open fenced code block.
    /// Cleared (so it rebuilds) on any state reset that would change output: theme/style, pretty mode, table width, soft-break mode, or `clear()`.
    pub(crate) open_code: Option<OpenCodeHighlighter>,

    /// Streaming LaTeX delimiter normalizer.
    /// Rewrites `\(…\)` / `\[…\]` / `\begin{equation}` into the canonical `$` / `$$` forms before text is appended to `source`.
    /// Held-back ambiguous bytes (a partial delimiter at a chunk boundary) are flushed by `finish()`.
    pub(crate) normalizer: LatexDelimiterNormalizer,
}

impl std::fmt::Debug for StreamingMarkdownRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingMarkdownRenderer")
            .field("source_len", &self.source.len())
            .field("frozen_lines", &self.frozen.lines_len)
            .field("frozen_bytes", &self.frozen.source_bytes)
            .field("output_lines", &self.output.lines.len())
            .field("pretty", &self.pretty)
            .finish()
    }
}

impl Clone for StreamingMarkdownRenderer {
    fn clone(&self) -> Self {
        // Create a fresh renderer and push all source text
        // This recreates the frozen state correctly.
        // We must propagate `max_table_width` BEFORE pushing/rendering so the clone produces identical output to the original
        let mut new = Self::new(self.style, self.pretty);
        new.set_max_table_width(self.max_table_width);
        new.set_collapse_soft_breaks(self.collapse_soft_breaks);
        // `self.source` is already normalized, so append it verbatim rather than re-running the normalizer
        // A re-run could hold back a trailing ambiguous suffix and make the clone's source diverge
        // Copy the normalizer state separately so any held-back bytes survive the clone
        new.push_normalized(&self.source);
        new.render(None);
        new.normalizer = self.normalizer.clone();
        new
    }
}

impl StreamingMarkdownRenderer {
    /// Create a new streaming renderer.
    pub fn new(style: MarkdownStyle, pretty: bool) -> Self {
        Self {
            source: String::new(),
            output: MarkdownRenderOutput::new(),
            frozen: FrozenState::default(),
            buffers: MarkdownBuffers::new(),
            style,
            pretty,
            max_table_width: None,
            collapse_soft_breaks: true,
            open_code: None,
            normalizer: LatexDelimiterNormalizer::new(),
        }
    }

    /// Replace the markdown style and trigger a full re-render.
    ///
    /// Used when the theme changes at runtime so existing blocks pick up the new colors on the next render pass.
    pub fn set_style(&mut self, style: MarkdownStyle) {
        self.style = style;
        self.frozen = FrozenState::default();
        self.output.clear();
        // Theme/style change alters colors, so any cached highlight is stale.
        self.open_code = None;
    }

    /// Set the maximum width for rendered tables.
    /// When set, column widths are shrunk proportionally so the table fits within the given number of display columns.
    /// If the width changes, frozen state is reset to ensure consistent rendering.
    pub fn set_max_table_width(&mut self, width: Option<usize>) {
        if self.max_table_width != width {
            self.max_table_width = width;
            // Reset frozen state since table formatting may change
            self.frozen = FrozenState::default();
            self.output.clear();
            self.open_code = None;
        }
    }

    /// Set whether CommonMark soft breaks collapse to a space.
    /// Resets frozen state when the mode changes.
    pub fn set_collapse_soft_breaks(&mut self, collapse: bool) {
        if self.collapse_soft_breaks != collapse {
            self.collapse_soft_breaks = collapse;
            self.frozen = FrozenState::default();
            self.output.clear();
            self.open_code = None;
        }
    }

    /// Push a new chunk of markdown text (no rendering).
    /// Call `render()` to process accumulated content, or use `push_and_render()` for convenience.
    pub fn push(&mut self, chunk: &str) {
        let normalized = self.normalizer.push(chunk);
        self.source.push_str(&normalized);
    }

    /// Append already-normalized source text, bypassing the delimiter normalizer.
    /// Used by `clone()` to reproduce an existing (already normalized) `source` exactly.
    /// The cloned normalizer state is copied separately so any held-back bytes are preserved.
    fn push_normalized(&mut self, text: &str) {
        self.source.push_str(text);
    }

    /// Render accumulated content.
    /// Pass `None` for syntect to disable syntax highlighting for code blocks.
    /// The `syntect` theme must stay stable between renders; switch themes via [`set_style`](Self::set_style), which clears that cache.
    pub fn render(&mut self, syntect: Option<&Syntect>) {
        self.rerender_tail(syntect);
    }

    /// Push a chunk and render immediately (convenience method).
    /// Equivalent to `push(chunk)` followed by `render(syntect)`.
    /// Use this for real-time streaming where you want to display after each chunk.
    pub fn push_and_render(&mut self, chunk: &str, syntect: Option<&Syntect>) {
        let normalized = self.normalizer.push(chunk);
        self.source.push_str(&normalized);
        self.rerender_tail(syntect);
    }

    /// Get a view of the current rendered output.
    /// This is cheap: it returns a reference to cached output.
    /// The output was computed during `render()` or `push_and_render()`.
    pub fn view(&self) -> MarkdownRenderView<'_> {
        self.output.as_view()
    }

    /// Get the accumulated source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the number of frozen source bytes.
    pub fn frozen_bytes(&self) -> usize {
        self.frozen.source_bytes
    }

    /// Get the number of frozen output lines.
    pub fn frozen_lines_count(&self) -> usize {
        self.frozen.lines_len
    }

    /// Reset the renderer, clearing all accumulated content.
    /// Also resets `max_table_width` to `None` for symmetry with the freshly-constructed state.
    /// Otherwise a later `set_max_table_width(Some(prev_width))` is silently a no-op (no state reset) because the equality check sees no change.
    pub fn clear(&mut self) {
        self.source.clear();
        self.output.clear();
        self.frozen = FrozenState::default();
        self.max_table_width = None;
        self.open_code = None;
        self.normalizer.reset();
    }

    /// Set pretty mode (true = hide syntax, false = show raw markdown).
    ///
    /// If the mode changes, frozen state is reset to ensure consistent rendering.
    pub fn set_pretty(&mut self, pretty: bool) {
        if self.pretty != pretty {
            self.pretty = pretty;
            // Reset frozen state so everything re-renders with the new mode
            self.frozen = FrozenState::default();
            self.output.clear();
            self.open_code = None;
        }
    }

    /// Get current pretty mode.
    pub fn pretty(&self) -> bool {
        self.pretty
    }

    /// Consume the renderer and return the owned output.
    ///
    /// Use this when streaming is complete and you want owned data.
    pub fn into_output(self) -> MarkdownRenderOutput {
        self.output
    }
}
