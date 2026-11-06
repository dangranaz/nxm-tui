//! Contract tests for `src/md/open_code_highlighter.rs` (V3/T5d).
//! Adapted from `xai-grok-markdown/src/open_code_highlighter.rs` (Apache-2.0).
//! Tests use `Syntect::new()` (option B defaults) instead of the upstream
//! `test_syntect()` helper (which depended on `include_bytes!(tokyo-night.tmTheme)`).

use syntect::util::LinesWithEndings;

use nxm_tui::md::open_code_highlighter::{HlLine, OpenCodeHighlighter, CLOSED_MEMO_CAP_BYTES};
use nxm_tui::md::syntax::Syntect;

/// Batch reference highlight, mirroring `parse::syntax_highlight_raw`.
fn batch(syn: &Syntect, fence: &str, text: &str) -> Vec<HlLine> {
    let mut hl = syn
        .highlight_lines_for_fence_info(fence)
        .expect("syntax for fence");
    LinesWithEndings::from(text)
        .map(|line| {
            hl.highlight_line(line, &syn.syntax_set)
                .expect("highlight line")
                .into_iter()
                .map(|(s, t)| (s, t.to_string()))
                .collect()
        })
        .collect()
}

/// Crescita append-only = batch: per ogni prefisso crescente, `highlight(...)`
/// == highlight batch dello stesso prefisso.
#[test]
fn append_only_growth_matches_fresh_full_highlight() {
    let syn = Syntect::new().expect("syntect defaults");
    let full = "foo: 1\nbar:\n  - a\n  - b\nbaz: true\n";
    let mut cache = OpenCodeHighlighter::new(&syn);
    // Grow one byte at a time; every prefix must equal a one-shot batch highlight of that same prefix
    for end in 1..=full.len() {
        let Some(prefix) = full.get(..end) else {
            continue;
        };
        let got = cache.highlight(&syn, "yaml", 0, prefix).expect("hl");
        assert_eq!(got, batch(&syn, "yaml", prefix), "prefix len {end}");
    }
}

/// Cambio fence invalida la cache.
#[test]
fn fence_change_invalidates() {
    let syn = Syntect::new().expect("syntect defaults");
    let text = "let x = 1;\nfn main() {}\n";
    let mut cache = OpenCodeHighlighter::new(&syn);
    // Prime with yaml, then re-key to rust: output must match a fresh rust batch, proving the persisted yaml state was discarded
    let _ = cache.highlight(&syn, "yaml", 0, text).expect("hl yaml");
    let got = cache.highlight(&syn, "rust", 0, text).expect("hl rust");
    assert_eq!(got, batch(&syn, "rust", text));
}

/// Cambio start-offset invalida.
#[test]
fn start_offset_change_invalidates() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    let a = "alpha: 1\nbeta: 2\n";
    let _ = cache.highlight(&syn, "yaml", 0, a).expect("hl a");
    // Same language, different block position and body: the new body must be highlighted fresh (no stale committed lines from the old block)
    let b = "gamma: 3\ndelta: 4\n";
    let got = cache.highlight(&syn, "yaml", 42, b).expect("hl b");
    assert_eq!(got, batch(&syn, "yaml", b));
}

/// Fence sconosciuto → None (no panic).
#[test]
fn unknown_fence_returns_none() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    assert!(cache
        .highlight(&syn, "definitely-not-a-language-xyz", 0, "data\n")
        .is_none(),);
}

// ── highlight_closed (closed-fence memo) ─────────────────────────

/// Memo closed-fence = batch + idempotente.
#[test]
fn closed_memo_matches_batch_and_is_idempotent() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    let body = "fn answer(x: u64) -> u64 {\n    x.wrapping_mul(42)\n}\n";

    // The first call computes; it must equal the batch reference exactly
    let first = cache.highlight_closed(&syn, "rust", body).expect("hl");
    assert_eq!(first, batch(&syn, "rust", body));
    assert_eq!(cache.closed_memo_bytes(), body.len());

    // The second call is a memo hit: identical output, no new entry
    let second = cache.highlight_closed(&syn, "rust", body).expect("hl");
    assert_eq!(second, first);
    assert_eq!(cache.closed_memo_bytes(), body.len());
}

/// Memo distingue fence_info e body.
#[test]
fn closed_memo_distinguishes_fence_info_and_body() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    let body_a = "key: value\n";
    let body_b = "other: thing\n";

    let yaml_a = cache.highlight_closed(&syn, "yaml", body_a).expect("hl");
    let yaml_b = cache.highlight_closed(&syn, "yaml", body_b).expect("hl");
    let rust_a = cache.highlight_closed(&syn, "rust", body_a).expect("hl");

    assert_eq!(yaml_a, batch(&syn, "yaml", body_a));
    assert_eq!(yaml_b, batch(&syn, "yaml", body_b));
    assert_eq!(rust_a, batch(&syn, "rust", body_a));
}

/// Memo closed non disturba lo stato open-block.
#[test]
fn closed_memo_does_not_disturb_open_block_state() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    // Interleave closed-memo calls with open-block incremental growth (a tail with one closed fence above an open one)
    // Open-block output must stay batch-identical throughout
    let closed = "name: pinned\n";
    let full = "a = 1\nb = 2\nc = 3\n";
    for end in 1..=full.len() {
        let Some(prefix) = full.get(..end) else {
            continue;
        };
        let _ = cache.highlight_closed(&syn, "yaml", closed).expect("memo");
        let got = cache.highlight(&syn, "python", 7, prefix).expect("hl");
        assert_eq!(got, batch(&syn, "python", prefix), "prefix len {end}");
    }
    assert_eq!(cache.closed_memo_bytes(), closed.len());
}

/// Cap overflow: output resta corretto dopo eviction.
#[test]
fn closed_memo_cap_overflow_keeps_output_correct() {
    let syn = Syntect::new().expect("syntect defaults");
    let mut cache = OpenCodeHighlighter::new(&syn);
    // The bodies are sized so a handful of distinct ones cross the byte budget and trigger the wholesale clear
    // Output must stay batch-identical before, at, and after the eviction
    let filler = "x".repeat(CLOSED_MEMO_CAP_BYTES / 4);
    for i in 0..6 {
        let body = format!("key_{i}: \"{filler}\"\n");
        let got = cache.highlight_closed(&syn, "yaml", &body).expect("hl");
        assert_eq!(got, batch(&syn, "yaml", &body), "iteration {i}");
    }
    assert!(cache.closed_memo_bytes() <= CLOSED_MEMO_CAP_BYTES);
}
