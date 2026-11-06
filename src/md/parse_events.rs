//! Derived from xai-grok-markdown/src/parse.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; latex/format_table stubbed (dedicated/T5g tickets); mermaid/math branches stubbed.
use crate::md::buffers::{Replace, Transform};
use crate::md::latex;
use crate::md::parse::{find_substring, is_br_tag, MarkdownParser};
use crate::md::syntax::syntax_highlight_raw;
use anstyle::Style;
use pulldown_cmark::{CodeBlockKind, Event, Tag};
use std::ops::Range;
impl<'a, 'b, 'syn, 'oc> MarkdownParser<'a, 'b, 'syn, 'oc> {
    /// Dispatch a parsed markdown event to the appropriate handler.
    pub(crate) fn on_event(&mut self, event: Event<'a>, range: Range<usize>) {
        let mut parent_code_block = None;
        // Apply ALL ancestors' inner styles to non-marker events.
        let skip_inner_style = matches!(
            event,
            Event::Start(_) | Event::End(_) | Event::Code(_) | Event::InlineMath(_)
        );
        // Collect ancestor styles first (to avoid borrow issues)
        let ancestor_styles: Vec<Option<Style>> = if !skip_inner_style {
            let in_link = self
                .tag_stack
                .iter()
                .any(|t| matches!(t, Tag::Link { .. } | Tag::Image { .. }));
            let strip_fg_in_link =
                |style: Style| if in_link { style.fg_color(None) } else { style };
            self.tag_stack
                .iter()
                .filter_map(|ancestor| match ancestor {
                    Tag::Heading { level, .. } => {
                        let level_usize = (*level as usize).saturating_sub(1).min(5);
                        self.ms.heading_outer.get(level_usize).copied().map(Some)
                    }
                    Tag::Emphasis => Some(Some(strip_fg_in_link(self.ms.emphasis_inner))),
                    Tag::Strong => Some(Some(strip_fg_in_link(self.ms.strong_inner))),
                    Tag::Strikethrough => Some(Some(strip_fg_in_link(self.ms.strikethrough_inner))),
                    Tag::Link { .. } | Tag::Image { .. } => Some(None),
                    Tag::CodeBlock(block) => {
                        parent_code_block = Some(match block {
                            CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                                Some(lang.to_owned())
                            }
                            _ => None,
                        });
                        None
                    }
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        for style in &ancestor_styles {
            self.push_highlight(*style, &range);
        }
        match event {
            Event::Start(tag) => self.on_start(tag, range),
            Event::End(tag_end) => self.on_end(tag_end, range),
            Event::Text(text) => {
                // Capture text into table cell if we're inside a table
                self.push_table_text_linking_urls(&text);
                // Record the enclosing fenced block's raw byte range and its de-prefixed body content
                // pulldown merges the body into one text event, but accumulate defensively in case it is split
                if parent_code_block.is_some() {
                    if let Some(pending) = self.pending_code_block.as_mut() {
                        if pending.body_seen {
                            pending.body_range.start = pending.body_range.start.min(range.start);
                            pending.body_range.end = pending.body_range.end.max(range.end);
                        } else {
                            pending.body_range = range.clone();
                            pending.body_seen = true;
                        }
                        pending.body_text.push_str(&text);
                    }
                }
                if let Some(parent_code_block) = parent_code_block {
                    // Closed mermaid fences render as a diagram; open ones fall through so the source shows while still streaming
                    if let Some(lang) = parent_code_block.as_deref() {
                        if lang
                            .split_whitespace()
                            .next()
                            .is_some_and(|t| t.eq_ignore_ascii_case("mermaid"))
                            && range.end < self.text.len()
                            && self.try_push_mermaid(&text, &range)
                        {
                            return;
                        }
                    }
                    let highlighted = match parent_code_block {
                        Some(lang) => {
                            if let Some(syn) = self.syntect {
                                if let Some(cache) = self.open_code.as_deref_mut() {
                                    // Streaming tail: the cache routes between the incremental open-block path and the closed-fence memo
                                    cache.highlight_block(
                                        syn,
                                        &lang,
                                        range.start,
                                        range.end >= self.text.len(),
                                        &text,
                                    )
                                } else {
                                    // Batch render (no streaming caches attached).
                                    syntax_highlight_raw(self.syntect, &lang, &text)
                                }
                            } else {
                                None
                            }
                        }
                        None => None,
                    };
                    if let Some(highlighted) = highlighted {
                        self.buffers.replaces.push(Replace {
                            highlighted,
                            range: range.clone(),
                        });
                    } else {
                        self.push_highlight(Some(self.ms.code_untagged), &range);
                        self.buffers.untagged_code_ranges.push(range.clone());
                    }
                } else {
                    if ancestor_styles.is_empty() {
                        // Apply the default text style only when no ancestor (heading, strong, emphasis, etc.) already provides a color
                        // Otherwise ms.text would override them
                        self.push_highlight(Some(self.ms.text), &range);
                    } else {
                        self.push_highlight(None, &range);
                    }
                    if self.table_state.is_none() {
                        self.scan_inline_html_entities(&range);
                    }
                }
            }
            Event::Code(code) => {
                self.on_code_event(&code, &range);
            }
            Event::InlineMath(math) => {
                // `$...$` inline math: render the TeX to Unicode and swap it in via a pretty-mode transform
                // Falls back to inline-code presentation when conversion declines (oversized input) or produces nothing visible
                let rendered = latex::latex_to_unicode_inline(&math).filter(|r| !r.is_empty());
                if let Some(ref mut state) = self.table_state {
                    match &rendered {
                        Some(r) => {
                            let prev_italic = state.cell_italic;
                            state.cell_italic = true;
                            state.push_text(r);
                            state.cell_italic = prev_italic;
                        }
                        None => {
                            let prev_code = state.cell_code;
                            state.cell_code = true;
                            state.push_text(&math);
                            state.cell_code = prev_code;
                        }
                    }
                }
                match rendered {
                    Some(r) => {
                        // One highlight and one transform span the entire `$...$` range
                        // Pretty mode shows the rendered math, raw mode shows the TeX source in the math style
                        self.push_highlight(Some(self.ms.math), &range);
                        self.buffers.transforms.push(Transform {
                            range: range.clone(),
                            to: r,
                            force: false,
                        });
                    }
                    None => self.style_inline_code_span(&math, &range),
                }
            }
            Event::SoftBreak => {
                // Collapse soft breaks to spaces unless the byte after pulldown's SoftBreak range is a list-item indent or blockquote `>` marker
                // In that case the line ending belongs to a block continuation and the renderer shows it as its own visual line
                // The transform spans the full range so CRLF (`\r\n`, 2 bytes) preserves byte length
                if let Some(ref mut state) = self.table_state {
                    state.push_text(" ");
                } else {
                    let next = self.text.as_bytes().get(range.end);
                    let is_continuation = matches!(next, Some(b' ' | b'\t' | b'>' | b'|'));
                    if self.collapse_soft_breaks && !is_continuation {
                        let span = range.end - range.start;
                        debug_assert!(span >= 1, "SoftBreak range must cover at least one byte");
                        self.buffers.transforms.push(Transform {
                            range: range.clone(),
                            to: " ".repeat(span),
                            force: true,
                        });
                    }
                }
                self.push_highlight(None, &range);
            }
            Event::HardBreak => {
                if let Some(ref mut state) = self.table_state {
                    state.push_text("\n");
                }
                self.push_highlight(None, &range);
            }
            Event::Html(_) => {
                // Render HTML block content as regular text (not code).
                // pulldown-cmark treats XML-like tags (e.g. <example>) as HTML blocks, which would otherwise get code-block styling via Replace.
                self.push_highlight(Some(self.ms.text), &range);
            }
            Event::InlineHtml(html) => {
                if is_br_tag(&html) {
                    if let Some(ref mut state) = self.table_state {
                        state.push_text("\n");
                        self.push_highlight(Some(self.ms.text), &range);
                    } else {
                        self.buffers.transforms.push(Transform {
                            range: range.clone(),
                            to: "\n".to_string(),
                            force: false,
                        });
                        self.push_highlight(None, &range);
                    }
                } else if let Some(ref mut state) = self.table_state {
                    state.push_text(&html);
                    self.push_highlight(Some(self.ms.text), &range);
                } else if let Some(highlighted) = syntax_highlight_raw(self.syntect, "html", &html)
                {
                    self.buffers.replaces.push(Replace {
                        highlighted,
                        range: range.clone(),
                    });
                }
            }
            Event::DisplayMath(math) => {
                // `$$...$$` display math: render to Unicode block lines.
                if let Some(ref mut state) = self.table_state {
                    // Inside a table cell there is no room for a block: render single-line (rows joined with `; `)
                    match latex::latex_to_unicode_inline(&math).filter(|r| !r.is_empty()) {
                        Some(r) => {
                            let prev_italic = state.cell_italic;
                            state.cell_italic = true;
                            state.push_text(&r);
                            state.cell_italic = prev_italic;
                        }
                        None => {
                            let prev_code = state.cell_code;
                            state.cell_code = true;
                            state.push_text(&math);
                            state.cell_code = prev_code;
                        }
                    }
                    self.push_highlight(Some(self.ms.math), &range);
                } else if self.push_display_math_block(range.clone(), &math) {
                    // Raw mode shows the TeX source in the math style; pretty mode consumes the range via the block replacement
                    self.push_highlight(Some(self.ms.math), &range);
                } else {
                    // Fallback (conversion declined or nothing visible): highlight the TeX source as code
                    self.push_highlight(Some(self.ms.code_outer), &range);
                    if let Some(outer_text) = self.text.get(range.clone()) {
                        if let Some(r) = find_substring(outer_text, &math, true, false) {
                            let inner_range = (range.start + r.start)..(range.start + r.end);
                            if let Some(highlighted) =
                                syntax_highlight_raw(self.syntect, "tex", &math)
                            {
                                self.buffers.replaces.push(Replace {
                                    highlighted,
                                    range: inner_range,
                                });
                            } else {
                                self.push_highlight(Some(self.ms.code_untagged), &inner_range);
                            }
                        }
                    }
                }
            }
            Event::FootnoteReference(_) => {
                self.on_footnote_reference(range);
            }
            Event::Rule => {
                self.on_rule(range);
            }
            Event::TaskListMarker(checked) => {
                self.on_task_list_marker(checked, range);
            }
        }
    }
}
