//! Contract tests per `StreamingMarkdownRenderer` (V3/T14b).
//! Adapted from `xai-grok-markdown/src/streaming.rs` (Apache-2.0).
//! Upstream inline tests NOT ported (dipendono da `render_markdown_ratatui_full`/
//! `test_syntect` non portati): qui si usa solo l'API pubblica dello streamer.
//!
//! Test CHIAVE: **`finish()` == one-shot** — l'intero markdown pushato in più
//! chunk, dopo `finish()`, deve coincidere (numero righe E testo concatenato)
//! col render one-shot dello stesso testo intero via
//! `render_markdown_ratatui_with_link_id` (helper T14a).

use nxm_tui::md::buffers::MarkdownBuffers;
use nxm_tui::md::render_markdown_ratatui_with_link_id;
use nxm_tui::md::streaming::StreamingMarkdownRenderer;
use nxm_tui::md::style::MarkdownStyle;
use nxm_tui::md::syntax::Syntect;
use ratatui::text::Line;

fn style_fixed() -> MarkdownStyle {
    nxm_tui::md::theme::fixed_style()
}

/// Concatena il testo (contenuto degli span) di ogni linea renderizzata.
fn lines_to_text(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

/// Render one-shot di riferimento (helper T14a, stessi parametri di `finish()`).
fn one_shot(text: &str, style: MarkdownStyle) -> Vec<String> {
    let mut buffers = MarkdownBuffers::new();
    let syntect = Syntect::new();
    let (output, _, _) = render_markdown_ratatui_with_link_id(
        text,
        style,
        true,
        &mut buffers,
        syntect.as_ref(),
        None,
        0,
        true,
        None,
    );
    lines_to_text(&output.lines)
}

/// TEST CHIAVE: `finish()` == one-shot. L'intero markdown arriva in 3 chunk,
/// poi `finish()`; il risultato deve coincidere col render one-shot dello
/// stesso testo intero (stesso numero di righe E stesso testo concatenato).
#[test]
fn finish_equals_one_shot_render() {
    let style = style_fixed();
    let text = "# Titolo\n\nParagrafo con **bold** e `code`.\n\n- voce uno\n- voce due\n\n> una citazione\n\n---\n\nChiusura con [link](https://example.org) e bare URL https://example.com/x.";
    let chunks: [&str; 3] = [
        "# Titolo\n\nParagrafo con **bold** e `code`.\n\n",
        "- voce uno\n- voce due\n\n> una citazione\n\n---\n\n",
        "Chiusura con [link](https://example.org) e bare URL https://example.com/x.",
    ];

    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    let syntect = Syntect::new();
    for chunk in chunks {
        renderer.push_and_render(chunk, syntect.as_ref());
    }
    let view = renderer.finish(syntect.as_ref());

    let got = lines_to_text(view.lines);
    let want = one_shot(text, style);

    assert_eq!(got.len(), want.len(), "line count: finish() vs one-shot");
    assert_eq!(got, want, "rendered text: finish() vs one-shot");
}

/// Come il test chiave, ma il confronto avviene anche senza syntect (None):
/// `finish(None)` deve restare coerente col one-shot `syntect=None`.
#[test]
fn finish_equals_one_shot_render_no_syntect() {
    let style = style_fixed();
    let text = "# Solo testo\n\nriga1\nriga2\n\n- a\n- b\n";
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    renderer.push("# Solo testo\n\n");
    renderer.push("riga1\nriga2\n\n- a\n- b\n");
    let view = renderer.finish(None);

    let got = lines_to_text(view.lines);
    let want = one_shot_no_syntect(text, style);
    assert_eq!(got.len(), want.len(), "line count: finish() vs one-shot");
    assert_eq!(got, want, "rendered text: finish() vs one-shot");
}

fn one_shot_no_syntect(text: &str, style: MarkdownStyle) -> Vec<String> {
    let mut buffers = MarkdownBuffers::new();
    let (output, _, _) = render_markdown_ratatui_with_link_id(
        text,
        style,
        true,
        &mut buffers,
        None,
        None,
        0,
        true,
        None,
    );
    lines_to_text(&output.lines)
}

/// FrozenState (costruzione): i 3 campi upstream partono da zero — verificati
/// tramite gli accessor pubblici `frozen_bytes()`/`frozen_lines_count()`
/// (FrozenState è `pub(crate)` come upstream). Dopo `finish()` tutto è frozen.
#[test]
fn frozen_state_starts_empty_and_covers_all_after_finish() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    assert_eq!(renderer.frozen_bytes(), 0);
    assert_eq!(renderer.frozen_lines_count(), 0);

    let text = "# H\n\ntesto\n\n---\n\naltro paragrafo\n";
    renderer.push_and_render(text, None);
    let _ = renderer.finish(None);

    // A fine stream: frozen copre intero source e tutte le linee
    assert_eq!(renderer.frozen_bytes(), renderer.source().len());
    assert_eq!(renderer.frozen_lines_count(), renderer.view().lines.len());
}

/// `push` accumula il source: `source()` == concatenazione dei chunk
/// (normalizer stub = passthrough).
#[test]
fn push_accumulates_source() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    renderer.push("primo ");
    renderer.push("secondo ");
    renderer.push("terzo");
    assert_eq!(renderer.source(), "primo secondo terzo");
}

/// `clear()` riazzera source, output e stato frozen.
#[test]
fn clear_resets_everything() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    renderer.push_and_render("# vecchio\n", None);
    assert!(!renderer.source().is_empty());
    assert!(!renderer.view().lines.is_empty());

    renderer.clear();
    assert!(renderer.source().is_empty());
    assert!(renderer.view().lines.is_empty());
    assert_eq!(renderer.frozen_bytes(), 0);
    assert_eq!(renderer.frozen_lines_count(), 0);
}

/// `finish` su stream vuoto: no panic, output vuoto.
#[test]
fn finish_on_empty_stream_no_panic() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    let view = renderer.finish(None);
    assert!(view.lines.is_empty());
}

/// Code-fence ancora aperto a metà stream (` ``` ` senza chiusura): no panic,
/// e la chiusura successiva produce lo stesso output del one-shot.
#[test]
fn open_code_fence_mid_stream_no_panic() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    let syntect = Syntect::new();
    renderer.push_and_render("```rust\nfn main() {\n", syntect.as_ref());
    renderer.push_and_render("    println!(\"hi\");\n", syntect.as_ref());
    // Fence ancora aperto: la view intermedia non deve panicare
    assert!(!renderer.view().lines.is_empty());
    // Chiusura + finish: coerente col one-shot
    renderer.push_and_render("}\n```\n\nfatto.\n", syntect.as_ref());
    let view = renderer.finish(syntect.as_ref());

    let got = lines_to_text(view.lines);
    let want = one_shot(
        "```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\nfatto.\n",
        style,
    );
    assert_eq!(got.len(), want.len(), "line count dopo chiusura fence");
    assert_eq!(got, want, "rendered text dopo chiusura fence");
}

/// `rerender_tail` (via `push_and_render`) dopo più push mantiene l'output
/// coerente col one-shot al `finish`: più checkpoint avanzano il frozen, il
/// tail viene ritruncato/rerenderizzato, il risultato finale non cambia.
#[test]
fn repeated_rerender_tail_still_matches_one_shot_at_finish() {
    let style = style_fixed();
    let mut renderer = StreamingMarkdownRenderer::new(style, true);
    let syntect = Syntect::new();
    let text = "# A\n\nprimo paragrafo.\n\n## B\n\nsecondo paragrafo.\n\n## C\n\nterzo paragrafo.\n\nfine.\n";
    // 6 push: ogni paragrafo arriva a pezzi, checkpoint multipli
    for chunk in [
        "# A\n\n",
        "primo paragrafo.\n\n",
        "## B\n\n",
        "secondo paragrafo.\n\n",
        "## C\n\nterzo paragrafo.\n\n",
        "fine.\n",
    ] {
        renderer.push_and_render(chunk, syntect.as_ref());
        // Intermedio: il frozen avanza senza mai superare il source
        assert!(renderer.frozen_bytes() <= renderer.source().len());
        assert!(renderer.frozen_lines_count() <= renderer.view().lines.len());
    }
    let view = renderer.finish(syntect.as_ref());

    let got = lines_to_text(view.lines);
    let want = one_shot(text, style);
    assert_eq!(got.len(), want.len(), "line count: finish() vs one-shot");
    assert_eq!(got, want, "rendered text: finish() vs one-shot");
}
