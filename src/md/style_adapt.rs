//! Derived from xai-grok-markdown/src/style.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; adapt_style stubbed to identity pending colors.rs.
//!
//! Color-adaptation shim for `MarkdownStyle::adapt()`.
//!
//! The real implementation (truecolor → 256/16 downgrade) will live in
//! `src/md/colors.rs` and arrive with a future ticket. Until then this is an
//! identity function so that every `adapt()` call site remains stable —
//! swapping in the real downgrade is a one-line import change in `style.rs`.

use anstyle::Style;

/// Adapt a style for the terminal's color capabilities.
///
/// **Placeholder (T1):** returns `s` unchanged (identity). When the real
/// `colors.rs` downgrade lands, this body is replaced; `pub(crate)` visibility
/// matches the upstream so the import in `style.rs` can swap to
/// `crate::colors::adapt_style` unchanged.
pub(crate) fn adapt_style(s: Style) -> Style {
    s
}
