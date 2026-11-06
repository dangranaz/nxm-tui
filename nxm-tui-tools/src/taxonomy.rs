//! Tool taxonomy for the per-task selection mechanism (TOOLS-1 Parte A, MT-1).
//!
//! Pure classification groundwork: one enum + two defaulted trait methods. The
//! automatic selection (`select(task)`/`manifest_for(task)`, MT-2) will group
//! on [`ToolKind`] and match on [`crate::Tool::keywords`]. Nothing here changes
//! the observable behavior of `manifest()` or `manifest_selected()`.

/// Task taxonomy of a tool: how the automatic per-task selection (MT-2) will
/// group it.
///
/// Variants map 1:1 onto the design decision (TOOLS-1, user decisions
/// 2026-10-20): [`ToolKind::Core`] tools are *always* sent to the model (they
/// form the automatic selection and the fail-safe set); every other kind is
/// pulled in by case-insensitive keyword matching on [`crate::Tool::keywords`].
/// The final model-visible set is the *union* of the automatic selection and
/// the manual overlay selection.
///
/// ```
/// use nxm_tui_tools::{Tool, ToolKind, WebFetch};
///
/// assert_eq!(WebFetch.kind(), ToolKind::Web);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Read-only access reserved for future tools: like [`ToolKind::Core`] it
    /// never writes and never touches the network, but it is *not* part of the
    /// fail-safe set. No built-in tool uses it yet (the read/navigate tools are
    /// [`ToolKind::Core`]).
    Read,
    /// Filesystem/semantic search (built-in: `grep`).
    Search,
    /// File mutation (built-in: `search_replace`).
    Edit,
    /// Always included by the automatic selection and the fail-safe set
    /// (built-in: `read_file`, `list_resources`) — read/navigate access, no
    /// write, no network.
    Core,
    /// Network access (built-in: `web_search`, `web_fetch`).
    Web,
    /// No classification given — the trait default ([`crate::Tool::kind`]).
    Other,
}
