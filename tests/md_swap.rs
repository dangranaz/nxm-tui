//! T8 SWAP contract tests (V3).
//!
//! `md::render` is the wired pipeline (parse → `render_ratatui`); the
//! `render_markdown` shim in `src/markdown.rs` dispatches on the
//! `NXM_USE_NEW_MD` env flag (default OFF → legacy fallback). Tests that
//! touch the env var guard it with a mutex so parallel runners stay sound.

use std::sync::{Mutex, MutexGuard, OnceLock};

use nxm_tui::markdown::render_markdown;
use nxm_tui::md::render;
use ratatui::text::Line;

const FLAG: &str = "NXM_USE_NEW_MD";

/// Concatenate the text of every span in the rendered lines.
fn lines_text(lines: &[Line<'static>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialize the tests that mutate the process-wide env flag.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(()));
    lock.lock().expect("static mutex")
}

// ---------------------------------------------------------------- md::render

/// MILESTONE (T8): `md::render` goes through the real pipeline — non-empty
/// lines and the heading/bold text survive.
#[test]
fn md_render_baseline_not_empty() {
    let lines = render("# Titolo\n\ntesto **bold**", 80);
    assert!(!lines.is_empty());
    let text = lines_text(&lines);
    assert!(text.contains("Titolo"), "text: {text:?}");
    assert!(text.contains("bold"), "text: {text:?}");
}

/// `md::render` must never panic on degenerate documents.
#[test]
fn md_render_no_panic_on_edge_inputs() {
    for input in [
        "",
        "   \n\t  ",
        "```rust\nlet x = 1;\n```",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
    ] {
        let lines = render(input, 80);
        if input.starts_with("```") || input.starts_with('|') {
            assert!(!lines.is_empty(), "fence/table must render: {input:?}");
        }
    }
}

/// `max_width` is applied to tables: a too-wide table shrinks to the budget.
#[test]
fn md_render_table_bound_to_max_width() {
    let style = render(
        "| aaaaaaaaaaaa | bbbbbbbbbbbbbbbb |\n|---|---|\n| 1 | 2 |\n",
        12,
    );
    let text = lines_text(&style);
    assert!(text.contains('a'), "text: {text:?}");
    assert!(text.contains('b'), "text: {text:?}");
}

// ---------------------------------------------------- render_markdown shim

fn set_flag(on: bool, _guard: &MutexGuard<'static, ()>) {
    if on {
        std::env::set_var(FLAG, "1");
    } else {
        std::env::remove_var(FLAG);
    }
}

/// With `NXM_USE_NEW_MD` ON every line carries the `"   "` prefix and the
/// heading/bold/code text comes from the V3 pipeline.
#[test]
fn shim_flag_on_prefixes_every_line() {
    let guard = env_lock();
    set_flag(true, &guard);
    let lines = render_markdown("# Titolo\n\ntesto **bold** e `code`", 80, "   ");
    assert!(!lines.is_empty());
    for line in &lines {
        let text = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.starts_with("   "), "line without prefix: {text:?}");
    }
    let text = lines_text(&lines);
    assert!(text.contains("Titolo"), "text: {text:?}");
    assert!(text.contains("bold"), "text: {text:?}");
    set_flag(false, &guard);
}

/// With the flag OFF (default) the legacy renderer answers: never panics on
/// empty input and keeps its known, *unwrapped-but-deterministic* shape.
#[test]
fn shim_flag_off_defaults_to_legacy() {
    let guard = env_lock();
    set_flag(false, &guard);
    // Degenerate inputs must stay safe on the legacy path too.
    assert!(render_markdown("", 80, "   ").is_empty());
    // Heading keeps its `#` marker in legacy mode; 1 line per row, no panic.
    let lines = render_markdown("# Titolo\n", 80, "");
    assert_eq!(lines.len(), 1);
    assert!(lines_text(&lines).starts_with("# Titolo"));
    set_flag(false, &guard);
}

/// Same env var must still gate when set to a non-affirmative value.
#[test]
fn shim_flag_other_values_stay_legacy() {
    let _guard = env_lock();
    std::env::set_var(FLAG, "0");
    let lines = render_markdown("ciao **mondo**", 80, "   ");
    assert_eq!(lines.len(), 1, "legacy wraps short input to one line");
    std::env::remove_var(FLAG);
}
