//! Contract tests for `src/md/syntax.rs` (V3/T5b, option B).
//! Adapted from `xai-grok-markdown/src/syntax.rs` (Apache-2.0).
//! Only non-Swift tests are reported; Swift tests are deferred (option A).

use nxm_tui::md::syntax::{parse_line_citation_fence_info, syntax_highlight_raw, Syntect};

/// `parse_line_citation_fence_info` parse start:end:path.
#[test]
fn line_citation_fence_parses_start_end_path() {
    assert_eq!(
        parse_line_citation_fence_info("37:65:path.rs"),
        Some(("37", "65", "path.rs"))
    );
}

/// `parse_line_citation_fence_info` rifiuta numeri non validi.
#[test]
fn line_citation_rejects_non_numeric_line() {
    assert_eq!(parse_line_citation_fence_info("37:ab:f.rs"), None);
}

/// `parse_line_citation_fence_info` rifiuta token di linguaggio semplici.
#[test]
fn line_citation_rejects_plain_lang_token() {
    assert_eq!(parse_line_citation_fence_info("rust"), None);
    assert_eq!(parse_line_citation_fence_info(""), None);
}

/// `Syntect` di default costruibile senza panic; `find_syntax_by_token("rust")` è `Some`.
#[test]
fn syntect_default_builds_and_finds_rust() {
    let syntect = Syntect::new().expect("default theme set must have a theme");
    assert!(syntect.find_syntax_by_token("rust").is_some());
}

/// `syntax_highlight_raw` produce più span di stile distinto su uno snippet rust.
#[test]
fn syntax_highlight_raw_produces_spans() {
    let syntect = Syntect::new().expect("default theme set must have a theme");
    let lines = syntax_highlight_raw(Some(&syntect), "rust", "fn main() {}")
        .expect("rust fence must highlight");
    assert!(!lines.is_empty(), "must produce at least one line");
    let has_multi = lines
        .iter()
        .any(|line| line.len() >= 2 && line.windows(2).any(|w| w[0].0 != w[1].0));
    assert!(
        has_multi,
        "expected at least one line with >=2 distinct styles"
    );
}

/// `syntax_highlight_raw(None, ...)` == `None`; lang ignoto → `None` (no panic).
#[test]
fn syntax_highlight_raw_none_and_unknown_lang() {
    let syntect = Syntect::new().expect("default theme set must have a theme");

    // None syntect → None
    assert!(syntax_highlight_raw(None, "rust", "fn main() {}").is_none());

    // Unknown lang → None
    assert!(
        syntax_highlight_raw(Some(&syntect), "zzz-nope", "code").is_none(),
        "unknown lang must return None, not panic"
    );
}
