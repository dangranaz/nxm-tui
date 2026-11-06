//! Derived from xai-grok-markdown/src/buffers.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; table types here, core types in buffers.rs.

use pulldown_cmark::Alignment;
use ratatui::text::Line;
use std::ops::Range;

use super::buffers::{CellJoin, CellSpan, StyledCell};

/// State for buffering table content during parsing.
#[derive(Debug, Clone)]
pub struct TableState {
    /// Column alignments from the table header.
    pub alignments: Vec<Alignment>,
    /// Header row cells.
    pub header: Vec<StyledCell>,
    /// Body rows.
    pub rows: Vec<Vec<StyledCell>>,
    /// Current row being built.
    pub current_row: Vec<StyledCell>,
    /// Current cell content being accumulated.
    pub current_cell: StyledCell,
    /// Current style state for the cell.
    pub cell_bold: bool,
    pub cell_italic: bool,
    pub cell_code: bool,
    pub cell_strike: bool,
    /// Current link state: `Some((url, id))` while inside a `Tag::Link` / `Tag::Image` inside a table cell.
    /// Text events while this is set produce link-tagged `CellSpan`s so the table renderer can apply link styling and emit `HyperlinkTarget`s.
    pub cell_link: Option<(String, u32)>,
    /// Whether we're in the header section.
    pub in_header: bool,
    /// Source byte range of the entire table.
    pub range: Range<usize>,
}

impl TableState {
    pub fn new(alignments: Vec<Alignment>, start: usize) -> Self {
        Self {
            alignments,
            header: Vec::new(),
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: StyledCell::new(),
            cell_bold: false,
            cell_italic: false,
            cell_code: false,
            cell_strike: false,
            cell_link: None,
            in_header: false,
            range: start..start,
        }
    }

    /// Push text with current styling to the cell.
    pub fn push_text(&mut self, text: &str) {
        self.current_cell.spans.push(CellSpan::new(
            text.to_string(),
            self.cell_bold,
            self.cell_italic,
            self.cell_code,
            self.cell_strike,
            self.cell_link.clone(),
        ));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCellCopy {
    pub text: String,
    pub joins: Vec<CellJoin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCopyMeta {
    pub line_index: usize,
    pub line_count: usize,
    pub n_cols: usize,
    pub cells: Vec<TableCellCopy>,
}

/// One hyperlink target inside a formatted table.
///
/// Coordinates are local to the table's `styled_lines`; the renderer adds the current absolute line count to produce a public `HyperlinkTarget`.
#[derive(Debug, Clone)]
pub struct TableHyperlink {
    /// Index within `TableReplace::styled_lines`.
    pub line_offset: usize,
    /// Column range (display cells) on that line.
    pub column_range: Range<usize>,
    /// Destination URL.
    pub url: String,
    /// Stable identifier shared with the paragraph link path.
    pub id: u32,
}

/// Formatted table replacement for pretty mode rendering.
#[derive(Debug, Clone)]
pub struct TableReplace {
    /// Formatted table lines (plain strings for ANSI rendering).
    pub lines: Vec<String>,
    /// Styled table lines for ratatui rendering.
    pub styled_lines: Vec<Line<'static>>,
    /// Source byte range this replaces.
    pub range: Range<usize>,
    /// Per-rendered-line source offset from the table start.
    /// The renderer uses this to produce correct `line_source_map` entries instead of the naive `table_start + line_idx`.
    pub line_source_offsets: Vec<usize>,
    /// Hyperlinks for `[label](url)` / autolinks inside table cells.
    /// The paragraph link path (`LinkTarget` then `chunk_link_offsets`) cannot project links onto a rendered table.
    /// The parser instead emits `TableHyperlink`s during table formatting, with positions in table-local coordinates.
    pub hyperlinks: Vec<TableHyperlink>,
    pub cell_copies: Vec<TableCellCopy>,
    pub n_cols: usize,
}

/// Rendered Mermaid diagram replacement for pretty mode rendering.
#[derive(Debug, Clone)]
pub struct MermaidReplace {
    /// Plain lines for ANSI rendering.
    pub lines: Vec<String>,
    /// Styled lines for ratatui rendering.
    pub styled_lines: Vec<Line<'static>>,
    /// Source byte range this replaces.
    pub range: Range<usize>,
}
