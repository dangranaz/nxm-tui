//! Contract tests for `src/md/core.rs` (V3/T3a core parsing).
//! Copied from `xai-grok-markdown-core/src/lib.rs` (Apache-2.0).

use nxm_tui::md::core::{offset_events, parser_options};
use pulldown_cmark::{Event, Options, Tag, TagEnd};

/// `parser_options()` abilita tutte e 5 le flags GFM.
#[test]
fn parser_options_contains_all_flags() {
    let opts = parser_options();
    assert!(opts.contains(Options::ENABLE_GFM), "ENABLE_GFM missing");
    assert!(
        opts.contains(Options::ENABLE_STRIKETHROUGH),
        "ENABLE_STRIKETHROUGH missing"
    );
    assert!(opts.contains(Options::ENABLE_MATH), "ENABLE_MATH missing");
    assert!(
        opts.contains(Options::ENABLE_TASKLISTS),
        "ENABLE_TASKLISTS missing"
    );
    assert!(
        opts.contains(Options::ENABLE_TABLES),
        "ENABLE_TABLES missing"
    );
}

/// `offset_events`: `~~x~~` produce eventi Strikethrough Start/End.
#[test]
fn offset_events_double_tilde_produces_strikethrough() {
    let text = "~~bar~~";
    let events: Vec<_> = offset_events(text).collect();
    let strikethrough_events: Vec<_> = events
        .iter()
        .filter(|(event, _)| {
            matches!(
                event,
                Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough)
            )
        })
        .collect();
    assert!(
        !strikethrough_events.is_empty(),
        "~~bar~~ must produce Strikethrough events, got: {events:?}"
    );
}

/// `offset_events`: single-tilde `~x~` NON è strikethrough.
#[test]
fn offset_events_single_tilde_not_strikethrough() {
    let text = "~bar~";
    let events: Vec<_> = offset_events(text).collect();
    let strikethrough_events: Vec<_> = events
        .iter()
        .filter(|(event, _)| {
            matches!(
                event,
                Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough)
            )
        })
        .collect();
    assert!(
        strikethrough_events.is_empty(),
        "~bar~ must NOT produce Strikethrough events, got: {events:?}"
    );
    // I tilde devono apparire come Text
    let text_events: Vec<_> = events
        .iter()
        .filter(|(event, _)| matches!(event, Event::Text(_)))
        .collect();
    assert!(
        !text_events.is_empty(),
        "~bar~ must produce Text events, got: {events:?}"
    );
}

/// Gli offset (range byte) sono coerenti con le sottostringhe dell'input.
#[test]
fn offset_events_byte_ranges_consistent() {
    let text = "**hello**";
    let events: Vec<_> = offset_events(text).collect();
    // Ogni range deve essere valido e non vuoto
    for (_event, range) in &events {
        let substring = &text[range.clone()];
        assert!(
            !substring.is_empty(),
            "range {:?} produced empty substring",
            range
        );
        // Il range deve stare dentro i limiti del testo
        assert!(
            range.end <= text.len(),
            "range {:?} exceeds text length {}",
            range,
            text.len()
        );
    }
    // Almeno un evento deve esistere
    assert!(!events.is_empty(), "offset_events must produce events");
}
