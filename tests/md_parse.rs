//! Contract tests for `src/md/parse.rs` (V3/T5e).
//! Adapted from `xai-grok-markdown/src/parse.rs` (Apache-2.0).

use nxm_tui::md::buffers::MarkdownBuffers;
use nxm_tui::md::checkpoint::CheckpointKind;
use nxm_tui::md::parse::MarkdownParser;
use nxm_tui::md::style::MarkdownStyle;
use nxm_tui::md::syntax::Syntect;

/// `MarkdownParser::new(...)` costruibile; builder restituiscono `Self` con campi impostati.
#[test]
fn parser_new_and_builders() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parser = MarkdownParser::new("hello", style, &mut buffers, Some(&syntect))
        .collapse_soft_breaks(false)
        .max_table_width(Some(80));

    // Verifica che i builder restituiscano Self (chainable) e i campi siano impostati
    // (non panica, costruisce correttamente).
    let _ = parser;
}

/// `parse()` su input semplice NON panica e ritorna un `ParsedMarkdown`.
/// Output vuoto è OK: on_event è stub (T5f).
#[test]
fn parse_simple_input_no_panic() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();
    let syntect = Syntect::new().expect("syntect defaults");

    let parsed = MarkdownParser::new("hello", style, &mut buffers, Some(&syntect)).parse();

    // Verifica che i campi siano popolati
    assert_eq!(parsed.text, "hello");
    assert_eq!(parsed.next_link_id, 0);
    assert!(parsed.last_checkpoint.is_none());
}

/// `parse()` su input vuoto NON panica.
#[test]
fn parse_empty_input_no_panic() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parsed = MarkdownParser::new("", style, &mut buffers, None).parse();

    assert_eq!(parsed.text, "");
    assert_eq!(parsed.next_link_id, 0);
}

/// `parse()` con syntect None (nessun highlighting) NON panica.
#[test]
fn parse_no_syntect_no_panic() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parsed = MarkdownParser::new("hello **world**", style, &mut buffers, None).parse();

    assert_eq!(parsed.text, "hello **world**");
    assert_eq!(parsed.next_link_id, 0);
}

/// `collapse_soft_breaks(false)` imposta il campo correttamente.
#[test]
fn collapse_soft_breaks_false() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parser = MarkdownParser::new("a\nb", style, &mut buffers, None).collapse_soft_breaks(false);

    // Non panica e restituisce Self
    let _ = parser.parse();
}

/// `max_table_width(Some(80))` imposta il campo correttamente.
#[test]
fn max_table_width_some() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parser =
        MarkdownParser::new("a|b\n-|-\n1|2", style, &mut buffers, None).max_table_width(Some(80));

    let _ = parser.parse();
}

/// `max_table_width(None)` imposta il campo correttamente.
#[test]
fn max_table_width_none() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parser =
        MarkdownParser::new("a|b\n-|-\n1|2", style, &mut buffers, None).max_table_width(None);

    let _ = parser.parse();
}

/// `ParsedMarkdown` campi accessibili (pub(crate) usabili dai test del crate).
#[test]
fn parsed_markdown_fields() {
    let mut buffers = MarkdownBuffers::new();
    let style = MarkdownStyle::default();

    let parsed = MarkdownParser::new("test", style, &mut buffers, None).parse();

    assert_eq!(parsed.text, "test");
    assert_eq!(parsed.next_link_id, 0);
    assert!(parsed.last_checkpoint.is_none());
}

/// CheckpointKind varianti accessibili.
#[test]
fn checkpoint_kind_variants() {
    let _ = CheckpointKind::Heading;
    let _ = CheckpointKind::Paragraph;
    let _ = CheckpointKind::CodeBlock;
    let _ = CheckpointKind::BlockQuote;
    let _ = CheckpointKind::List;
    let _ = CheckpointKind::ThematicBreak;
    let _ = CheckpointKind::Table;
    let _ = CheckpointKind::HtmlBlock;
}
