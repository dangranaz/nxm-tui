//! Contract tests for `src/md/checkpoint.rs` (V3/T5a).
//! Copied from `xai-grok-markdown/src/checkpoint.rs` (Apache-2.0).

use nxm_tui::md::checkpoint::{Checkpoint, CheckpointKind};

/// `CheckpointKind` ha 8 varianti con i nomi upstream; PartialEq funziona.
#[test]
fn checkpoint_kind_has_all_variants() {
    assert_eq!(CheckpointKind::Heading, CheckpointKind::Heading);
    assert_eq!(CheckpointKind::Paragraph, CheckpointKind::Paragraph);
    assert_eq!(CheckpointKind::CodeBlock, CheckpointKind::CodeBlock);
    assert_eq!(CheckpointKind::BlockQuote, CheckpointKind::BlockQuote);
    assert_eq!(CheckpointKind::List, CheckpointKind::List);
    assert_eq!(CheckpointKind::ThematicBreak, CheckpointKind::ThematicBreak);
    assert_eq!(CheckpointKind::Table, CheckpointKind::Table);
    assert_eq!(CheckpointKind::HtmlBlock, CheckpointKind::HtmlBlock);

    // Varianti diverse sono disuguali
    assert_ne!(CheckpointKind::Heading, CheckpointKind::List);
    assert_ne!(CheckpointKind::Paragraph, CheckpointKind::Table);
    assert_ne!(CheckpointKind::CodeBlock, CheckpointKind::HtmlBlock);
}

/// `Checkpoint` ha i 3 campi ed è Copy/Clone/PartialEq.
#[test]
fn checkpoint_fields_and_copy() {
    let cp = Checkpoint {
        source_bytes: 42,
        output_lines: 3,
        kind: CheckpointKind::Heading,
    };

    assert_eq!(cp.source_bytes, 42);
    assert_eq!(cp.output_lines, 3);
    assert_eq!(cp.kind, CheckpointKind::Heading);

    // Copy: assegnazione non muove il valore
    let cp2 = cp;
    assert_eq!(cp, cp2);
    assert_eq!(cp.source_bytes, 42);
    assert_eq!(cp.output_lines, 3);

    // Copy: assegnazione diretta copia il valore (Checkpoint è Copy)
    let cp3 = cp;
    assert_eq!(cp3.kind, CheckpointKind::Heading);
    assert_eq!(cp.source_bytes, 42); // cp ancora usabile: non è stato spostato

    // PartialEq con kind diverso
    let cp4 = Checkpoint {
        source_bytes: 42,
        output_lines: 3,
        kind: CheckpointKind::List,
    };
    assert_ne!(cp, cp4);
}
