//! Markdown transcript renderer — the `USE_NEW_MD` swap (V3/T8).
//!
//! `render_markdown` is the entry point `history.rs` uses (frozen signature:
//! `render_markdown(content, max_width, prefix) -> Vec<Line<'static>>`). It
//! dispatches on the `USE_NEW_MD` rollout flag:
//!
//! - flag **OFF** (default): legacy V1 renderer (`markdown_legacy`) — wraps
//!   prose to `max_width` display columns and boxes code fences.
//! - flag **ON** (`NXM_USE_NEW_MD=1|true|yes|on`): V3 pipeline
//!   `crate::md::render` (rich headings/lists/bold/inline+code/tables/links);
//!   the legacy `prefix` is then re-applied to every line.
//!
//! The legacy path stays intact as the fallback; full text wrapping for the
//! V3 path lands with T9/T10.

use ratatui::text::{Line, Span};

/// Whether the V3 markdown swap is enabled: env `NXM_USE_NEW_MD`
/// (`1`/`true`/`yes`/`on`, case-insensitive). Default **OFF** — the legacy V1
/// renderer stays live until a reviewer/user opts in for visual testing on a
/// real TTY.
fn use_new_md() -> bool {
    match std::env::var("NXM_USE_NEW_MD") {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Prepend `prefix` as the first raw [`Span`] of `line`.
///
/// The V3 renderer emits unprefixed lines; `history.rs` renders the transcript
/// with a `"   "` gutter and relies on the prefix being present on every row.
/// Alignment/style of the incoming line are preserved.
fn with_prefix(mut line: Line<'static>, prefix: &str) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len() + 1);
    spans.push(Span::raw(String::from(prefix)));
    spans.append(&mut line.spans);
    line.spans = spans;
    line
}

/// Render markdown into styled terminal lines.
///
/// Prose is word-wrapped to `max_width` **display columns** (CJK/emoji count as
/// two, malformed width as zero), code fences are boxed and truncated to the
/// same budget, and `prefix` is prepended to every line (legacy path). With
/// `NXM_USE_NEW_MD` enabled the content is rendered by the V3 pipeline
/// (`crate::md::render`) instead and only the prefix is re-applied.
///
/// ```
/// use nxm_tui::markdown::render_markdown;
///
/// let lines = render_markdown("hello **world**", 40, "   ");
/// assert_eq!(lines.len(), 1);
/// ```
pub fn render_markdown(content: &str, max_width: usize, prefix: &str) -> Vec<Line<'static>> {
    if use_new_md() {
        tracing::debug!(target: "nexum::markdown::swap", max_width, "USE_NEW_MD on: V3 pipeline");
        return crate::md::render(content, max_width)
            .into_iter()
            .map(|line| with_prefix(line, prefix))
            .collect();
    }
    tracing::debug!(target: "nexum::markdown::swap", max_width, "USE_NEW_MD off: legacy renderer");
    crate::markdown_legacy::render_markdown_legacy(content, max_width, prefix)
}
