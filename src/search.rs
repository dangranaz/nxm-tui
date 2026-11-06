// Adapted from grok-build (xai-grok-pager-render/src/search), Copyright 2023-2026 SpaceXAI, Apache-2.0.

//! Text search primitives for the TUI.
//!
//! [`TextMatcher`] compiles a substring or regex query (smart-case) once and
//! answers match queries; it owns no corpus and no UI state.
//! [`next_index_after`] and [`prev_index_before`] step through a sorted slice of
//! match positions for `n`/`N` traversal, wrapping at the ends.
//!
//! # Deviation from the source
//!
//! The upstream `TextMatcher` stored a `regex::Regex` and relied on
//! `unwrap()` for its "never match" fallbacks (`"(?:)"` and `r"\z."`). This
//! port forbids production `unwrap()`/`expect()` (see `RULES.md`), so the
//! field is `Option<regex::Regex>` instead: a failed or unbuildable regex
//! becomes `None`, [`TextMatcher::is_match`] returns `false` for `None`, and
//! no code path can ever panic.

/// How a query string should be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    /// Case-insensitive substring (compiled to a regex via `regex::escape`).
    Substring,
    /// User-supplied regex pattern.
    Regex,
}

/// Both substring and regex queries compile to a `regex::Regex`.
///
/// Matching is smart-case: case-insensitive unless the query contains an
/// uppercase character (Vim `smartcase` / ripgrep `--smart-case`).
///
/// The compiled regex is held as an `Option`: when the query is a bad regex
/// the matcher stores `None` (never matches) and flags [`is_error`](Self::is_error),
/// so matching never panics.
#[derive(Debug, Clone)]
pub struct TextMatcher {
    regex: Option<regex::Regex>,
    query: String,
    is_error: bool,
}

impl TextMatcher {
    /// Compile `query` under the given interpretation.
    ///
    /// A regex that fails to compile sets [`is_error`](Self::is_error) and the
    /// matcher then never matches anything.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nxm_tui::search::{QueryKind, TextMatcher};
    ///
    /// // Substring is matched literally and smart-case lowercases.
    /// let m = TextMatcher::new("alpha", QueryKind::Substring);
    /// assert!(m.is_match("ALPHA"));
    /// assert!(!m.is_error());
    ///
    /// // A bad regex flags an error and never matches.
    /// let bad = TextMatcher::new("[invalid", QueryKind::Regex);
    /// assert!(bad.is_error());
    /// assert!(!bad.is_match("invalid"));
    /// ```
    pub fn new(query: impl Into<String>, kind: QueryKind) -> Self {
        let query = query.into();
        let smart_ci = !query.chars().any(|c| c.is_uppercase());
        let pattern = match kind {
            QueryKind::Substring => regex::escape(&query),
            QueryKind::Regex => query.clone(),
        };
        let (regex, is_error) = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(smart_ci)
            .build()
        {
            // A `Substring` query is always a valid escaped pattern, so this
            // `Err` arm is only reachable for a bad user-supplied `Regex`.
            Ok(re) => (Some(re), false),
            Err(_) => (None, true),
        };
        tracing::debug!(
            target: "nexum::search",
            ?kind,
            query = %query,
            is_error,
            smart_case = smart_ci,
            "compiled text matcher"
        );
        Self {
            regex,
            query,
            is_error,
        }
    }

    /// The raw query string the user typed.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether the regex failed to compile (bad user regex).
    pub fn is_error(&self) -> bool {
        self.is_error
    }

    /// The compiled regex, for callers that highlight matches.
    ///
    /// Returns `None` when the query failed to compile (see [`is_error`](Self::is_error)).
    pub fn compiled_regex(&self) -> Option<&regex::Regex> {
        self.regex.as_ref()
    }

    /// Whether `haystack` contains a match. A matcher with a bad query (`None`
    /// regex) never matches.
    pub fn is_match(&self, haystack: &str) -> bool {
        match &self.regex {
            Some(re) => re.is_match(haystack),
            None => false,
        }
    }
}

/// Position in ascending `sorted` of the first index after `current`, wrapping
/// to the front. Returns `None` when `sorted` is empty.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::search::next_index_after;
///
/// let sorted = [0usize, 2, 4];
/// assert_eq!(next_index_after(&sorted, 0), Some(1));
/// assert_eq!(next_index_after(&sorted, 4), Some(0)); // wraps
/// assert_eq!(next_index_after(&[], 3), None);
/// ```
pub fn next_index_after(sorted: &[usize], current: usize) -> Option<usize> {
    if sorted.is_empty() {
        tracing::debug!(target: "nexum::search", "next_index_after on empty slice");
        return None;
    }
    let pos = sorted.partition_point(|&i| i <= current);
    Some(if pos < sorted.len() { pos } else { 0 })
}

/// Position in ascending `sorted` of the last index before `current`, wrapping
/// to the back. Returns `None` when `sorted` is empty.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::search::prev_index_before;
///
/// let sorted = [0usize, 2, 4];
/// assert_eq!(prev_index_before(&sorted, 4), Some(1));
/// assert_eq!(prev_index_before(&sorted, 0), Some(2)); // wraps
/// assert_eq!(prev_index_before(&[], 3), None);
/// ```
pub fn prev_index_before(sorted: &[usize], current: usize) -> Option<usize> {
    if sorted.is_empty() {
        tracing::debug!(target: "nexum::search", "prev_index_before on empty slice");
        return None;
    }
    let pos = sorted.partition_point(|&i| i < current);
    Some(if pos > 0 { pos - 1 } else { sorted.len() - 1 })
}
