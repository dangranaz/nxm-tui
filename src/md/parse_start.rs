//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; latex/format_table stubbed (dedicated/T5g tickets); mermaid/math branches stubbed.

use std::ops::Range;

use anstyle::Style;
use pulldown_cmark::{CodeBlockKind, Tag};

use crate::md::buffers::{Highlight, Transform};
use crate::md::buffers_table::TableState;
use crate::md::parse::{find_substring, MarkdownParser, PendingCodeBlock};

impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Handle tag start events.
    pub(crate) fn on_start(&mut self, tag: Tag<'a>, range: Range<usize>) {
        // Track nesting depth for checkpoint detection (upstream parse.rs on_start).
        match &tag {
            Tag::BlockQuote(_) | Tag::List(_) | Tag::Item | Tag::Table(_) => {
                self.depth += 1;
            }
            _ => {}
        }
        if matches!(&tag, Tag::BlockQuote(_)) {
            self.bq_depth += 1;
        }

        if let Some(style) = self.on_start_tag(tag, range.clone()) {
            self.push_highlight(Some(style), &range);
        }
    }

    /// Dispatch tag start to the appropriate handler.
    fn on_start_tag(&mut self, tag: Tag<'a>, range: Range<usize>) -> Option<Style> {
        let mut more = Vec::new();
        match tag {
            Tag::Paragraph => None,
            Tag::Heading { level, .. } => {
                let level_usize: usize = match level {
                    pulldown_cmark::HeadingLevel::H1 => 0usize,
                    pulldown_cmark::HeadingLevel::H2 => 1usize,
                    pulldown_cmark::HeadingLevel::H3 => 2usize,
                    pulldown_cmark::HeadingLevel::H4 => 3usize,
                    pulldown_cmark::HeadingLevel::H5 => 4usize,
                    pulldown_cmark::HeadingLevel::H6 => 5usize,
                }
                .saturating_sub(1)
                .min(5);
                let heading_outer = self.ms.heading_outer.get(level_usize).copied();
                let heading_text = self.text.get(range.clone());
                if let Some(marker_end) =
                    heading_text.and_then(|t| t.find(|c: char| c != '#' && c != ' '))
                {
                    let marker_range = range.start..range.start + marker_end;
                    more.push(Highlight {
                        style: heading_outer,
                        range: marker_range,
                    });
                    None
                } else {
                    heading_outer
                }
            }
            Tag::BlockQuote(_) => {
                self.on_start_blockquote(range, &mut more);
                None
            }
            Tag::CodeBlock(code) => {
                self.on_start_codeblock(&code, range, &mut more);
                None
            }
            Tag::HtmlBlock => None,
            Tag::List(_) => None,
            Tag::Item => {
                self.on_start_item(range, &mut more);
                None
            }
            Tag::Table(alignments) => {
                self.table_state = Some(TableState::new(alignments.to_vec(), range.start));
                Some(self.ms.table_outer)
            }
            Tag::TableHead => {
                if let Some(ref mut state) = self.table_state {
                    state.in_header = true;
                }
                Some(self.ms.table_outer)
            }
            Tag::TableRow => {
                if let Some(ref mut state) = self.table_state {
                    state.current_row.clear();
                }
                Some(self.ms.table_outer)
            }
            Tag::TableCell => {
                if let Some(ref mut state) = self.table_state {
                    state.current_cell.clear();
                }
                self.push_highlight(None, &range);
                None
            }
            Tag::Emphasis => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_italic = true;
                }
                Some(self.ms.emphasis_outer)
            }
            Tag::Strong => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_bold = true;
                }
                Some(self.ms.strong_outer)
            }
            Tag::Strikethrough => {
                if let Some(ref mut state) = self.table_state {
                    state.cell_strike = true;
                }
                Some(self.ms.strikethrough_outer)
            }
            Tag::Link {
                ref dest_url,
                ref title,
                ..
            }
            | Tag::Image {
                ref dest_url,
                ref title,
                ..
            } => self.on_start_link(&tag, range, &mut more, dest_url, title),
            _ => None,
        }
    }

    /// Handle blockquote start: transform `>` to `│` per line.
    fn on_start_blockquote(
        &mut self,
        range: Range<usize>,
        more: &mut Vec<Highlight>,
    ) -> Option<Style> {
        if let Some(bq_text) = self.text.get(range.clone()) {
            let mut pos = range.start;

            for line in bq_text.split_inclusive('\n') {
                let at_line_start = pos == 0 || self.text.as_bytes().get(pos - 1) == Some(&b'\n');
                let skip = if at_line_start { self.bq_depth - 1 } else { 0 };

                let mut found = 0usize;
                for (byte_offset, ch) in line.char_indices() {
                    if ch == '>' {
                        if found == skip {
                            let gt_pos = pos + byte_offset;
                            self.buffers.transforms.push(Transform {
                                range: gt_pos..gt_pos + 1,
                                to: "│".to_string(),
                                force: false,
                            });
                            more.push(Highlight {
                                style: Some(self.ms.blockquote_outer),
                                range: gt_pos..gt_pos + 1,
                            });
                            break;
                        }
                        found += 1;
                    }
                }
                pos += line.len();
            }
        }

        None
    }

    /// Handle code block start: track pending code block and style fence.
    fn on_start_codeblock(
        &mut self,
        code: &CodeBlockKind,
        range: Range<usize>,
        more: &mut Vec<Highlight>,
    ) -> Option<Style> {
        self.pending_code_block = match code {
            CodeBlockKind::Fenced(lang) => {
                let body_start = self
                    .text
                    .get(range.start..)
                    .and_then(|s| s.find('\n'))
                    .map_or(range.end, |nl| range.start + nl + 1);
                Some(PendingCodeBlock {
                    info: lang.to_string(),
                    body_range: body_start..body_start,
                    body_text: String::new(),
                    body_seen: false,
                })
            }
            CodeBlockKind::Indented => None,
        };

        let line_start = self
            .text
            .get(..range.start)
            .and_then(|s| s.rfind('\n'))
            .map_or(0, |p| p + 1);
        let fence_start = if self
            .text
            .get(line_start..range.start)
            .is_some_and(|s| s.bytes().all(|b| b == b' ' || b == b'\t'))
        {
            line_start
        } else {
            range.start
        };
        more.push(Highlight {
            style: Some(self.ms.code_outer),
            range: fence_start..range.end,
        });
        match code {
            CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                if let Some(slice) = self.text.get(range.clone()) {
                    if let Some(r) = find_substring(slice, lang, true, false) {
                        let lang_range = (r.start + range.start)..(r.end + range.start);
                        more.push(Highlight {
                            style: Some(self.ms.code_language),
                            range: lang_range,
                        });
                    }
                }
            }
            _ => (),
        }
        None
    }

    /// Handle list item start: style list marker and transform bullet.
    fn on_start_item(&mut self, range: Range<usize>, more: &mut Vec<Highlight>) -> Option<Style> {
        if let Some(item_text) = self.text.get(range.clone()) {
            let trimmed = item_text.trim_start();
            let leading_ws = item_text.len() - trimmed.len();

            let digits_before = |pos: usize| {
                pos > 0
                    && trimmed
                        .get(..pos)
                        .is_some_and(|s| s.chars().all(|c| c.is_ascii_digit()))
            };
            let marker_len = if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                2
            } else if let Some(pos) = trimmed.find(". ") {
                if digits_before(pos) {
                    pos + 2
                } else {
                    0
                }
            } else if let Some(pos) = trimmed.find(") ") {
                if digits_before(pos) {
                    pos + 2
                } else {
                    0
                }
            } else {
                0
            };

            if marker_len > 0 {
                let marker_start = range.start + leading_ws;
                let marker_end = marker_start + marker_len;
                more.push(Highlight {
                    style: Some(self.ms.list_item),
                    range: marker_start..marker_end,
                });

                if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    self.buffers.transforms.push(Transform {
                        range: marker_start..marker_start + 1,
                        to: "•".to_string(),
                        force: false,
                    });
                }
            }
        }
        None
    }
}
