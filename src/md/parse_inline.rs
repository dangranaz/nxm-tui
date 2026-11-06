use crate::md::buffers::{Highlight, LinkTarget, Transform};
use crate::md::checkpoint::CheckpointKind;
use crate::md::parse::{decode_html_entity, find_substring, MarkdownParser};
use crate::md::url_scan;
use anstyle::Style;
use pulldown_cmark::{CowStr, Tag};
use std::ops::Range;
impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Push a highlight range into the buffers.
    pub(crate) fn push_highlight(&mut self, style: Option<Style>, range: &Range<usize>) {
        self.buffers.highlights.push(Highlight {
            style,
            range: range.clone(),
        });
    }
    /// Apply inline-code styling to a code/math span: dim the delimiters, style the content.
    /// Shared by `Event::Code` and the inline-math fallback path.
    pub(crate) fn style_inline_code_span(&mut self, code: &CowStr<'_>, range: &Range<usize>) {
        let Some(outer_text) = self.text.get(range.clone()) else {
            self.push_highlight(Some(self.ms.inline_code_inner), range);
            return;
        };
        if let Some(inner_range) = find_substring(outer_text, code, false, false)
            .or_else(|| find_substring(outer_text, code, true, false))
        {
            let absolute_inner = (range.start + inner_range.start)..(range.start + inner_range.end);
            if inner_range.start > 0 {
                self.push_highlight(
                    Some(self.ms.inline_code_outer),
                    &(range.start..absolute_inner.start),
                );
            }
            self.push_highlight(Some(self.ms.inline_code_inner), &absolute_inner);
            if range.end > absolute_inner.end {
                self.push_highlight(
                    Some(self.ms.inline_code_outer),
                    &(absolute_inner.end..range.end),
                );
            }
        } else {
            self.push_highlight(Some(self.ms.inline_code_inner), range);
        }
    }
    /// Scan a prose `Event::Text` source range for HTML character entity references (`&lt;`, `&gt;`, `&amp;`, numeric, …).
    /// Code spans and fenced blocks never reach here, so entities inside code stay literal.
    /// Panic-safety: pulldown-cmark guarantees `range` is a valid sub-slice of `self.text`.
    pub(crate) fn scan_inline_html_entities(&mut self, range: &Range<usize>) {
        let Some(slice) = self.text.get(range.clone()) else {
            debug_assert!(false, "pulldown-cmark text range out of bounds");
            return;
        };
        if !slice.contains('&') {
            return;
        }
        const MAX_ENTITY_LEN: usize = 33;
        let bytes = slice.as_bytes();
        let mut i = 0;
        while let Some(&b) = bytes.get(i) {
            if b != b'&' {
                i += 1;
                continue;
            }
            let max = (i + MAX_ENTITY_LEN).min(bytes.len());
            let mut j = i + 1;
            let end = loop {
                if j >= max {
                    break None;
                }
                match bytes.get(j) {
                    Some(b';') => break Some(j),
                    Some(b'#' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9') => j += 1,
                    _ => break None,
                }
            };
            if let Some(end) = end {
                if let Some(entity) = slice.get(i..=end) {
                    if let Some(decoded) = decode_html_entity(entity) {
                        let abs = (range.start + i)..(range.start + end + 1);
                        let overlaps = self
                            .buffers
                            .transforms
                            .iter()
                            .any(|t| t.range.start < abs.end && abs.start < t.range.end);
                        if !overlaps {
                            self.push_highlight(None, &abs);
                            self.buffers.transforms.push(Transform {
                                range: abs,
                                to: decoded,
                                force: false,
                            });
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
    /// Push text into the current table cell (no-op outside a table), tagging
    /// bare URLs/emails as link spans. The post-render URL scan runs after the
    /// table has wrapped the cell, so it would only catch the first line of a
    /// wrapped URL; tagging here uses the wrap-aware `TableHyperlink` path.
    pub(crate) fn push_table_text_linking_urls(&mut self, text: &str) {
        let Some(state) = self.table_state.as_mut() else {
            return;
        };
        if state.cell_link.is_some() {
            state.push_text(text);
            return;
        }
        let mut cursor = 0;
        url_scan::for_each_plain_link(text, |range, url| {
            if range.start > cursor {
                if let Some(prefix) = text.get(cursor..range.start) {
                    state.push_text(prefix);
                }
            }
            let id = self.link_id_counter;
            self.link_id_counter += 1;
            state.cell_link = Some((url, id));
            if let Some(slice) = text.get(range.clone()) {
                state.push_text(slice);
            }
            state.cell_link = None;
            cursor = range.end;
        });
        if cursor < text.len() {
            if let Some(rest) = text.get(cursor..) {
                state.push_text(rest);
            }
        }
    }
    /// Render a mermaid code block into a [`MermaidReplace`]; `true` if drawn.
    /// Stub for T5g — returns false until the mermaid module is implemented.
    /// Push a pretty-mode block replacement rendering `latex_src` as display math over `range`.
    /// Stub for T-latex — returns false until the latex module is implemented.
    /// Handle link/image start: push highlights and register link target.
    pub(crate) fn on_start_link(
        &mut self,
        tag: &Tag<'a>,
        range: Range<usize>,
        more: &mut Vec<Highlight>,
        dest_url: &pulldown_cmark::CowStr<'_>,
        title: &pulldown_cmark::CowStr<'_>,
    ) -> Option<Style> {
        if let Some(ref mut state) = self.table_state {
            let id = self.link_id_counter;
            self.link_id_counter += 1;
            state.cell_link = Some((dest_url.to_string(), id));
            self.tag_stack.push(tag.clone());
            return None;
        }
        let Some(tag_str) = self.text.get(range.clone()) else {
            self.buffers.link_targets.push(LinkTarget {
                source_range: range.clone(),
                url: dest_url.to_string(),
                id: self.link_id_counter,
            });
            self.link_id_counter += 1;
            self.push_highlight(Some(self.ms.link_outer), &range);
            self.tag_stack.push(tag.clone());
            return None;
        };
        if !title.is_empty() {
            for t in
                [format!("\"{title}\""), format!("'{title}'")].map(pulldown_cmark::CowStr::from)
            {
                if let Some(r) = find_substring(tag_str, &t, true, true) {
                    let title_range = (r.start + range.start)..(r.end + range.start);
                    more.push(Highlight {
                        style: Some(self.ms.link_title),
                        range: title_range,
                    });
                }
            }
        }
        let url_rel_opt = find_substring(tag_str, dest_url, true, true);
        if let Some(r) = &url_rel_opt {
            let url_range = (r.start + range.start)..(r.end + range.start);
            more.push(Highlight {
                style: Some(self.ms.link_url),
                range: url_range,
            });
        }
        let bracket_pos_opt = url_rel_opt.as_ref().and_then(|r| {
            tag_str
                .get(..r.start)
                .and_then(|s| s.rfind("]("))
                .map(|p| p..p + 2)
        });
        if let Some(bracket_pos) = bracket_pos_opt {
            let open_bracket = if tag_str.starts_with("![") { 1 } else { 0 };
            if open_bracket > 0 {
                more.push(Highlight {
                    style: Some(self.ms.link_outer),
                    range: range.start..range.start + 1,
                });
            }
            more.push(Highlight {
                style: Some(self.ms.link_outer),
                range: range.start + open_bracket..range.start + open_bracket + 1,
            });
            let text_start = range.start + open_bracket + 1;
            let text_end = bracket_pos.start + range.start;
            if text_end > text_start {
                more.push(Highlight {
                    style: Some(self.ms.link_text),
                    range: text_start..text_end,
                });
            }
            let bracket_abs = bracket_pos.start + range.start;
            more.push(Highlight {
                style: Some(self.ms.link_outer),
                range: bracket_abs..bracket_abs + 2,
            });
            more.push(Highlight {
                style: Some(self.ms.link_outer),
                range: range.end - 1..range.end,
            });
            self.buffers.transforms.push(Transform {
                range: range.start + open_bracket..range.start + open_bracket + 1,
                to: "".to_string(),
                force: false,
            });
            self.buffers.transforms.push(Transform {
                range: bracket_abs..bracket_abs + 2,
                to: " (".to_string(),
                force: false,
            });
            if text_end > text_start {
                self.buffers.link_targets.push(LinkTarget {
                    source_range: text_start..text_end,
                    url: dest_url.to_string(),
                    id: self.link_id_counter,
                });
                self.link_id_counter += 1;
            }
            None
        } else {
            self.buffers.link_targets.push(LinkTarget {
                source_range: range.clone(),
                url: dest_url.to_string(),
                id: self.link_id_counter,
            });
            self.link_id_counter += 1;
            Some(self.ms.link_outer)
        }
    }
    /// Handle footnote reference event.
    pub(crate) fn on_footnote_reference(&mut self, range: Range<usize>) {
        self.push_highlight(Some(self.ms.link_outer), &range);
    }
    /// Handle rule (horizontal rule) event.
    pub(crate) fn on_rule(&mut self, range: Range<usize>) {
        self.push_highlight(Some(self.ms.rule), &range);
        if let Some(rule_text) = self.text.get(range.clone()) {
            if let Some(marker_end) = rule_text.find('\n') {
                self.buffers.transforms.push(Transform {
                    range: range.start..range.start + marker_end,
                    to: "───".to_string(),
                    force: false,
                });
            } else {
                self.buffers.transforms.push(Transform {
                    range: range.clone(),
                    to: "───".to_string(),
                    force: false,
                });
            }
        }
        if self.depth == 0 {
            self.last_checkpoint = Some((CheckpointKind::ThematicBreak, range.end));
        }
    }
    /// Handle task list marker event.
    pub(crate) fn on_task_list_marker(&mut self, checked: bool, range: Range<usize>) {
        let style = if checked {
            self.ms.task_checked
        } else {
            self.ms.task_unchecked
        };
        self.push_highlight(Some(style), &range);
    }
    /// Handle code event: capture code content into table cell and apply inline-code styling.
    pub(crate) fn on_code_event(&mut self, code: &CowStr<'_>, range: &Range<usize>) {
        if let Some(ref mut state) = self.table_state {
            let prev_code = state.cell_code;
            state.cell_code = true;
            state.push_text(code);
            state.cell_code = prev_code;
        }
        self.style_inline_code_span(code, range);
    }
}
