//! Contract tests for `src/md/url_scan.rs` (V3/T5c).
//! Adapted from `xai-grok-markdown/src/url_scan.rs` (Apache-2.0).
//! Only tests that do not depend on StreamingMarkdownRenderer are included.

use nxm_tui::md::output::HyperlinkTarget;
use nxm_tui::md::url_scan::{
    detect_plain_urls, detect_plain_urls_with_offset, for_each_plain_link,
};
use ratatui::text::{Line, Span};

/// URL in prosa: `for_each_plain_link` cattura l'URL senza il punto finale.
#[test]
fn for_each_plain_link_captures_url_without_trailing_dot() {
    let mut found = Vec::new();
    for_each_plain_link("See https://example.com.\n", |range, url| {
        found.push((range, url));
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "https://example.com");
}

/// email → `mailto:`.
#[test]
fn for_each_plain_link_email_becomes_mailto() {
    let mut found = Vec::new();
    for_each_plain_link("Email foo@bar.com please.", |range, url| {
        found.push((range, url));
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "mailto:foo@bar.com");
}

/// scp `git@host:path` NON diventa mailto.
#[test]
fn for_each_plain_link_scp_git_remote_not_mailto() {
    let mut found = Vec::new();
    for_each_plain_link("clone git@github.com:org/repo.git", |range, url| {
        found.push((range, url));
    });
    assert!(
        found.iter().all(|(_, url)| !url.starts_with("mailto:")),
        "scp-style git remotes must not become mailto links: {found:?}"
    );
}

/// punto finale escluso dall'URL.
#[test]
fn for_each_plain_link_trailing_period_excluded() {
    let mut found = Vec::new();
    for_each_plain_link("See https://example.com.", |range, url| {
        found.push((range, url));
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "https://example.com");
}

/// `detect_plain_urls` con URL spezzato su 2 span dà 1 solo target con URL intero.
#[test]
fn detect_plain_urls_split_across_style_spans_is_one_target() {
    let line = Line::from(vec![
        Span::raw("https://tracker.example.com/"),
        Span::raw("projects/issues/#12345"),
    ]);
    let (found, _next_id) = detect_plain_urls(&[line], &[], 0);
    assert_eq!(found.len(), 1, "style boundary must not split the URL");
    assert_eq!(
        found[0].url,
        "https://tracker.example.com/projects/issues/#12345"
    );
    assert_eq!(found[0].line_index, 0);
    assert_eq!(found[0].column_range.start, 0);
    assert_eq!(
        found[0].column_range.end,
        "https://tracker.example.com/projects/issues/#12345".len()
    );
}

/// `detect_plain_urls(&[], &[], 0)` → vuoto.
#[test]
fn detect_plain_urls_empty_document_returns_empty() {
    let (found, _next_id) = detect_plain_urls(&[], &[], 0);
    assert!(
        found.is_empty(),
        "empty document should produce no hyperlinks"
    );
}

/// `detect_plain_urls_with_offset` con offset di linea.
#[test]
fn detect_plain_urls_with_offset_applies_line_index_offset() {
    let line = Line::from("See https://example.com.");
    let (found, _next_id) = detect_plain_urls_with_offset(&[line], 5, &[], 0);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line_index, 5);
    assert_eq!(found[0].url, "https://example.com");
}

/// `detect_plain_urls` con existing target: non duplica.
#[test]
fn detect_plain_urls_dedup_overlaps_existing() {
    let line = Line::from("See https://example.com.");
    let existing = vec![HyperlinkTarget {
        line_index: 0,
        column_range: 4..18,
        url: "https://example.com".to_string(),
        id: 0,
    }];
    let (found, _next_id) = detect_plain_urls(&[line], &existing, 0);
    assert!(found.is_empty(), "existing target must prevent duplicate");
}

/// `detect_plain_urls` assegna id progressivi.
#[test]
fn detect_plain_urls_assigns_incremental_ids() {
    let line = Line::from("a https://a.example b https://b.example c");
    let (found, _next_id) = detect_plain_urls(&[line], &[], 10);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].id, 10);
    assert_eq!(found[1].id, 11);
    assert_eq!(found[0].url, "https://a.example");
    assert_eq!(found[1].url, "https://b.example");
}

/// `HyperlinkTarget` campi e PartialEq.
#[test]
fn hyperlink_target_fields_and_eq() {
    let h = HyperlinkTarget {
        line_index: 3,
        column_range: 5..15,
        url: "https://example.com".to_string(),
        id: 7,
    };
    assert_eq!(h.line_index, 3);
    assert_eq!(h.column_range, 5..15);
    assert_eq!(h.url, "https://example.com");
    assert_eq!(h.id, 7);

    let h2 = h.clone();
    assert_eq!(h, h2);

    let h3 = HyperlinkTarget {
        line_index: 3,
        column_range: 5..15,
        url: "https://other.com".to_string(),
        id: 7,
    };
    assert_ne!(h, h3);
}
