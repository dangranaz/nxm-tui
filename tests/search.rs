//! Tests for the `search` module: `TextMatcher` smart-case matching and
//! `next_index_after` / `prev_index_before` wrapping navigation.
//!
//! Ported from grok-build (xai-grok-pager-render/src/search), with an added
//! case for the `Option<Regex>` fallback introduced in this port.

use nxm_tui::search::{next_index_after, prev_index_before, QueryKind, TextMatcher};

#[test]
fn substring_is_matched_literally() {
    let m = TextMatcher::new("a.c", QueryKind::Substring);
    assert!(m.is_match("xa.cx"));
    assert!(!m.is_match("abc"));
}

#[test]
fn smart_case_is_insensitive_for_lowercase_query() {
    let m = TextMatcher::new("alpha", QueryKind::Substring);
    assert!(m.is_match("ALPHA"));
    assert!(m.is_match("Alpha"));
}

#[test]
fn smart_case_is_sensitive_when_query_has_uppercase() {
    let m = TextMatcher::new("Alpha", QueryKind::Substring);
    assert!(m.is_match("Alpha"));
    assert!(!m.is_match("alpha"));
}

#[test]
fn smart_case_applies_to_regex_queries() {
    let lower = TextMatcher::new("a.c", QueryKind::Regex);
    assert!(lower.is_match("AXC"));
    let upper = TextMatcher::new("A.C", QueryKind::Regex);
    assert!(!upper.is_match("axc"));
}

#[test]
fn regex_query_matches() {
    let m = TextMatcher::new("^(a|b)$", QueryKind::Regex);
    assert!(!m.is_error());
    assert!(m.is_match("a"));
    assert!(!m.is_match("ab"));
}

#[test]
fn bad_regex_flags_error_and_never_matches() {
    let m = TextMatcher::new("[invalid", QueryKind::Regex);
    assert!(m.is_error());
    assert!(!m.is_match("invalid"));
}

#[test]
fn bad_regex_has_no_compiled_regex() {
    // New behaviour for this port: the fallback is `None`, not a never-match
    // regex, so no `unwrap` is ever needed.
    let bad = TextMatcher::new("(unterminated", QueryKind::Regex);
    assert!(bad.is_error());
    assert!(bad.compiled_regex().is_none());

    let good = TextMatcher::new("ok", QueryKind::Substring);
    assert!(!good.is_error());
    assert!(good.compiled_regex().is_some());
    assert_eq!(good.query(), "ok");
}

#[test]
fn next_index_after_wraps() {
    let sorted = [0usize, 2, 4];
    assert_eq!(next_index_after(&sorted, 0), Some(1));
    assert_eq!(next_index_after(&sorted, 2), Some(2));
    assert_eq!(next_index_after(&sorted, 4), Some(0));
    assert_eq!(next_index_after(&[], 3), None);
    let one = [5usize];
    assert_eq!(next_index_after(&one, 5), Some(0));
    assert_eq!(next_index_after(&one, 1), Some(0));
}

#[test]
fn prev_index_before_wraps() {
    let sorted = [0usize, 2, 4];
    assert_eq!(prev_index_before(&sorted, 4), Some(1));
    assert_eq!(prev_index_before(&sorted, 2), Some(0));
    assert_eq!(prev_index_before(&sorted, 0), Some(2));
    assert_eq!(prev_index_before(&[], 3), None);
    let one = [5usize];
    assert_eq!(prev_index_before(&one, 5), Some(0));
    assert_eq!(prev_index_before(&one, 9), Some(0));
}
