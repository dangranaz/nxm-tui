//! Derived from xai-grok-markdown/src/syntax.rs (xai-org/grok-build, Apache-2.0). Modified (option B): syntaxes from SyntaxSet::load_defaults_newlines() instead of the two-face syntaxes.bin build dump; no assets/Swift grammar; expect() replaced with error handling; syntax_highlight_raw widened pub(crate)→pub for `tests/` reach (RULES visibility-for-testability). Option A (two-face) is a deferred ticket.
//!
//! Syntax highlighting support using syntect built-in defaults (option B).
//!
//! This module loads syntax definitions from syntect's built-in defaults
//! (`SyntaxSet::load_defaults_newlines()`) instead of the two-face
//! `syntaxes.bin` build dump. No `.tmTheme` assets, no `build.rs`, no Swift
//! grammar patching. Option A (two-face + assets) is a deferred ticket.

use std::path::Path;
use std::sync::OnceLock;

use syntect::{
    easy::HighlightLines,
    highlighting::{Theme as SyntectTheme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

/// Syntax highlighting configuration.
///
/// Create one instance and pass it to the markdown renderer.
pub struct Syntect {
    /// The color theme for syntax highlighting.
    pub theme: SyntectTheme,
    /// The syntax definitions (syntect built-in defaults).
    pub syntax_set: SyntaxSet,
}

impl Syntect {
    /// Create a new Syntect instance using syntect built-in defaults.
    ///
    /// Loads `SyntaxSet::load_defaults_newlines()` and picks `base16-eighties.dark`
    /// (same dark theme as `src/highlight.rs` V2), falling back to any available
    /// theme. No `two-face`, no `build.rs`, no assets. Returns `None` only if the
    /// default theme set is empty (should not happen with syntect built-ins).
    pub fn new() -> Option<Self> {
        let syntax_set = syntax_set().clone();
        let themes = &theme_set().themes;
        let theme = themes
            .get("base16-eighties.dark")
            .or_else(|| themes.values().next())?
            .clone();
        Some(Self { theme, syntax_set })
    }

    /// Find a syntax definition by file path extension.
    pub fn find_syntax_by_file_path(&self, file_path: &Path) -> Option<&SyntaxReference> {
        let ext = file_path.extension()?.to_str()?;
        self.syntax_set.find_syntax_by_extension(ext)
    }

    /// Find a syntax definition by language token (e.g., "rust", "python").
    pub fn find_syntax_by_token(&self, token: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_token(token)
    }

    /// Create a highlighter for the given file path.
    pub fn highlight_lines_by_file_path(&self, file_path: &Path) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_by_file_path(file_path)?,
            &self.theme,
        ))
    }

    /// Create a highlighter for the given language token.
    pub fn highlight_lines_for_token(&self, token: &str) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_by_token(token)?,
            &self.theme,
        ))
    }

    /// Highlighter for a fenced code block *info* string: a language token (`rust`, `python`) or a `lineStart:lineEnd:path` line-range citation.
    /// A citation's path resolves via [`Syntect::find_syntax_by_file_path`].
    /// When the path has no known syntax, the whole string falls back to [`Syntect::find_syntax_by_token`], so plain ` ```lang` blocks keep working.
    pub fn highlight_lines_for_fence_info(&self, fence_info: &str) -> Option<HighlightLines<'_>> {
        Some(HighlightLines::new(
            self.find_syntax_for_fence_info(fence_info)?,
            &self.theme,
        ))
    }

    /// Resolve the [`SyntaxReference`] for a fenced code block *info* string, with the same rules as [`Syntect::highlight_lines_for_fence_info`].
    /// This is exposed so the incremental open-code highlighter can build its resumable `ParseState`/`HighlightState` from the same syntax.
    /// That keeps its output byte-identical to the batch `HighlightLines` path.
    pub(crate) fn find_syntax_for_fence_info(&self, fence_info: &str) -> Option<&SyntaxReference> {
        let citation = parse_line_citation_fence_info(fence_info);
        if let Some((_, _, path)) = citation {
            if let Some(s) = self.find_syntax_by_file_path(Path::new(path)) {
                return Some(s);
            }
        }
        self.find_syntax_by_token(fence_info)
    }
}

/// Shared `SyntaxSet`, loaded once from syntect defaults.
fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Shared `ThemeSet`, loaded once from syntect defaults.
fn theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// The path is the segment after the **second** colon; it is then parsed with [`Path::new`].
/// Paths with extra colons in the first two segments are not supported; use a repo-relative or forward-slash form.
pub fn parse_line_citation_fence_info(info: &str) -> Option<(&str, &str, &str)> {
    let mut it = info.splitn(3, ':');
    let start = it.next()?;
    let end = it.next()?;
    let path = it.next()?;
    if start.is_empty() || !start.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if end.is_empty() || !end.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if path.is_empty() {
        return None;
    }
    Some((start, end, path))
}

/// Syntax highlight code, returning raw styled segments per line.
/// `fence_info` is the fenced code block *info* string (language tag or citation); see [`Syntect::highlight_lines_for_fence_info`].
/// This function lives here (not in `parse`) so both the parser and the streaming highlighter caches depend one-way on `syntax`.
pub fn syntax_highlight_raw(
    syntect: Option<&Syntect>,
    fence_info: &str,
    text: &str,
) -> Option<Vec<Vec<(syntect::highlighting::Style, String)>>> {
    let syn = syntect?;
    let mut hl = syn.highlight_lines_for_fence_info(fence_info)?;
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(text) {
        let highlighted = hl.highlight_line(line, &syn.syntax_set).ok()?;
        lines.push(
            highlighted
                .into_iter()
                .map(|(s, t)| (s, t.to_string()))
                .collect(),
        );
    }
    Some(lines)
}
