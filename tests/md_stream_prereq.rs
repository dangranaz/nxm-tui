//! Contract tests per i prerequisiti streaming T14a:
//! `LatexDelimiterNormalizer` (stub passthrough), `render_markdown_ratatui_with_link_id`
//! e i builder `MarkdownParser::link_id_start` / `open_code`.

use nxm_tui::md::buffers::MarkdownBuffers;
use nxm_tui::md::latex::LatexDelimiterNormalizer;
use nxm_tui::md::open_code_highlighter::OpenCodeHighlighter;
use nxm_tui::md::parse::MarkdownParser;
use nxm_tui::md::render_markdown_ratatui_with_link_id;
use nxm_tui::md::style::MarkdownStyle;
use nxm_tui::md::syntax::Syntect;

fn style_syntect() -> (MarkdownStyle, Syntect) {
    (
        MarkdownStyle::default(),
        Syntect::new().expect("syntect defaults"),
    )
}

#[test]
fn latex_normalizer_is_passthrough() {
    let mut n = LatexDelimiterNormalizer::new();
    assert_eq!(n.push("abc"), "abc");
    assert_eq!(n.finish(), "");
    n.reset();
    assert_eq!(n.push("x"), "x");
    assert_eq!(n.finish(), "");
    let _ = n.clone();
}

#[test]
fn parser_link_id_start_uses_given_id() {
    // Un link consuma l'id di partenza e avanza il counter di uno.
    let (style, syntect) = style_syntect();
    let mut buffers = MarkdownBuffers::new();
    let parsed = MarkdownParser::new("[a](http://x)", style, &mut buffers, Some(&syntect))
        .link_id_start(10)
        .parse();
    assert_eq!(parsed.next_link_id, 11);

    // Nessun link: il counter resta al valore di partenza.
    let (style, syntect) = style_syntect();
    let mut buffers = MarkdownBuffers::new();
    let parsed = MarkdownParser::new("solo testo", style, &mut buffers, Some(&syntect))
        .link_id_start(10)
        .parse();
    assert_eq!(parsed.next_link_id, 10);
}

#[test]
fn parser_open_code_builder_chains() {
    let (style, syntect) = style_syntect();
    let mut buffers = MarkdownBuffers::new();
    let mut highlighter = OpenCodeHighlighter::new(&syntect);

    let parsed = MarkdownParser::new("codice", style, &mut buffers, Some(&syntect))
        .open_code(Some(&mut highlighter))
        .parse();
    assert_eq!(parsed.text, "codice");
}

#[test]
fn render_with_link_id_advances_counter() {
    let (style, syntect) = style_syntect();
    let mut buffers = MarkdownBuffers::new();

    let (_output, _checkpoint, next_link_id) = render_markdown_ratatui_with_link_id(
        "vedi [esempio](http://example.org) qui",
        style,
        true,
        &mut buffers,
        Some(&syntect),
        None,
        5,
        true,
        None,
    );

    assert!(next_link_id >= 5);
    // Un link nel testo avanza il counter oltre il valore di partenza.
    assert!(next_link_id > 5);
}
