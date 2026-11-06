//! Derived from xai-grok-markdown/src/render.rs (xai-org/grok-build, Apache-2.0). Modified: ratatui path only (render_ansi/Stylize excluded); split for the 300-line cap (RenderLoopState + apply_transforms + build_render_events(_into)); apply_transforms/build_render_events(_into) widened private→pub(crate) for the cross-file impl split; crate::md paths; StyleInto reused from parse.rs; let-chains→nested ifs (Rust 2021); loop locals grouped into RenderLoopState.

use ratatui::text::Line;

use std::borrow::Cow;

use crate::md::buffers::{RenderEvent, RenderEventKind};
use crate::md::checkpoint::{Checkpoint, CheckpointKind};
use crate::md::parse::ParsedMarkdown;

/// Render-loop state shared by the `render_ratatui` helpers (T6d split).
/// Upstream keeps these as stack locals of the single `render_ratatui`
/// function; grouping them is the mechanical rename (upstream local `x` →
/// `state.x`) that keeps every helper on the 300-line cap while preserving
/// the copy fidelity.
pub(crate) struct RenderLoopState {
    /// Upstream `last_pos`: cursor into the source text of what has been emitted so far.
    pub(crate) last_pos: usize,
    /// Upstream `replace`: index of the syntax-highlighted `Replace` under construction, if any.
    pub(crate) replace: Option<usize>,
    /// Upstream `table_replace`: index of the `TableReplace` under construction, if any.
    pub(crate) table_replace: Option<usize>,
    /// Upstream `mermaid_replace`: index of the `MermaidReplace` under construction, if any.
    pub(crate) mermaid_replace: Option<usize>,
    /// Upstream `skip_leading_newline`: suppress the newline after a hidden marker.
    pub(crate) skip_leading_newline: bool,
    /// Upstream `in_hidden_code_block`: inside a pretty-hidden code fence.
    pub(crate) in_hidden_code_block: bool,
    /// Upstream `next_link_idx`: monotonic cursor into `link_targets`.
    pub(crate) next_link_idx: usize,
    /// Upstream `cur_col_in_line`: running display column on the in-progress line.
    pub(crate) cur_col_in_line: usize,
    /// Upstream `current_source_line`: source line of the output line being built.
    pub(crate) current_source_line: usize,
    /// Upstream `last_line_count_pos`: source byte up to which the source-line counter has been advanced.
    pub(crate) last_line_count_pos: usize,
    /// Upstream `pending_line_is_code`: code-block membership of the chunk that produced the pending spans.
    pub(crate) pending_line_is_code: bool,
    /// Upstream `code_bg_style`: pre-adapted code background style.
    pub(crate) code_bg_style: ratatui::style::Style,
    /// Upstream `checkpoint_info`: `(kind, byte)` of the last parse-time checkpoint.
    pub(crate) checkpoint_info: Option<(CheckpointKind, usize)>,
    /// Upstream `checkpoint_output_lines`: captured output-line count at the checkpoint, once frozen.
    pub(crate) checkpoint_output_lines: Option<usize>,
}

impl<'a, 'b> ParsedMarkdown<'a, 'b> {
    pub(crate) fn apply_transforms<'t>(
        &self,
        text: &'t str,
        start: usize,
        pretty: bool,
    ) -> Cow<'t, str> {
        if self.buffers.transforms.is_empty() {
            return Cow::Borrowed(text);
        }
        // Raw mode applies only `force` transforms (e.g. soft-break collapse).
        if !pretty && !self.buffers.transforms.iter().any(|t| t.force) {
            return Cow::Borrowed(text);
        }

        let end = start + text.len();
        let mut result = String::new();
        let mut pos = start;
        let mut applied = false;

        for transform in &self.buffers.transforms {
            if transform.range.end <= start || transform.range.start >= end {
                continue;
            }
            if !pretty && !transform.force {
                continue;
            }
            applied = true;
            // Clamp transform range to our text range
            let t_start = transform.range.start.max(start);
            let t_end = transform.range.end.min(end);

            // Copy text before transform
            if t_start > pos {
                if let Some(before) = text.get((pos - start)..(t_start - start)) {
                    result.push_str(before);
                }
            }

            // Apply transform
            result.push_str(&transform.to);

            pos = t_end;
        }

        if !applied {
            Cow::Borrowed(text)
        } else {
            // Copy remaining text
            if pos < end {
                if let Some(rest) = text.get(pos - start..) {
                    result.push_str(rest);
                }
            }
            Cow::Owned(result)
        }
    }

    /// Build sorted render events into the provided Vec.
    pub(crate) fn build_render_events_into(&self, events: &mut Vec<RenderEvent>) {
        events.clear();
        let capacity = self.buffers.highlights.len() * 2
            + self.buffers.replaces.len() * 2
            + self.buffers.table_replaces.len() * 2
            + self.buffers.mermaid_replaces.len() * 2;
        events.reserve(capacity);

        for (i, hl) in self.buffers.highlights.iter().enumerate() {
            events.push(RenderEvent {
                pos: hl.range.start,
                kind: RenderEventKind::Highlight,
                index: i,
                is_end: false,
            });
            events.push(RenderEvent {
                pos: hl.range.end,
                kind: RenderEventKind::Highlight,
                index: i,
                is_end: true,
            });
        }
        for (i, r) in self.buffers.replaces.iter().enumerate() {
            events.push(RenderEvent {
                pos: r.range.start,
                kind: RenderEventKind::Replace,
                index: i,
                is_end: false,
            });
            events.push(RenderEvent {
                pos: r.range.end,
                kind: RenderEventKind::Replace,
                index: i,
                is_end: true,
            });
        }
        for (i, t) in self.buffers.table_replaces.iter().enumerate() {
            events.push(RenderEvent {
                pos: t.range.start,
                kind: RenderEventKind::Table,
                index: i,
                is_end: false,
            });
            events.push(RenderEvent {
                pos: t.range.end,
                kind: RenderEventKind::Table,
                index: i,
                is_end: true,
            });
        }
        for (i, m) in self.buffers.mermaid_replaces.iter().enumerate() {
            events.push(RenderEvent {
                pos: m.range.start,
                kind: RenderEventKind::Mermaid,
                index: i,
                is_end: false,
            });
            events.push(RenderEvent {
                pos: m.range.end,
                kind: RenderEventKind::Mermaid,
                index: i,
                is_end: true,
            });
        }
        events.sort_unstable();
    }

    /// Build sorted render events into a new Vec.
    pub(crate) fn build_render_events(&self) -> Vec<RenderEvent> {
        let mut events = Vec::new();
        self.build_render_events_into(&mut events);
        events
    }

    /// Upstream tail of `render_ratatui` (verbatim modulo the renames): if the
    /// checkpoint was not captured during event processing, freeze the output
    /// line count from the newlines up to the checkpoint byte, then pair it
    /// with the checkpoint info.
    pub(crate) fn finalize_checkpoint(
        &self,
        state: &mut RenderLoopState,
        lines: &[Line<'static>],
        line_source_map: &[usize],
    ) -> Option<Checkpoint> {
        // If checkpoint wasn't captured during event processing, compute it based on the number of newlines in the text up to checkpoint byte
        // This handles cases where there are no events past the checkpoint (e.g., incomplete list items).
        if state.checkpoint_output_lines.is_none() {
            if let Some((_, cp_byte)) = state.checkpoint_info {
                // The checkpoint is at the start of the NEXT block, so the frozen content must not include anything at or after cp_byte
                // line_source_map[i] is the source line at which output line i was created; source_line_at_cp is the source line containing cp_byte

                let source_line_at_cp = self
                    .text
                    .get(..cp_byte.min(self.text.len()))
                    .unwrap_or("")
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count();

                // When the checkpoint is at or past the end of the text, ALL output lines are frozen (the checkpointed block consumed the entire input)
                // Otherwise, output lines created at source lines strictly before the checkpoint source line are frozen
                let complete_lines = if cp_byte >= self.text.len() {
                    lines.len()
                } else {
                    line_source_map
                        .iter()
                        .take_while(|&&src_line| src_line < source_line_at_cp)
                        .count()
                };

                state.checkpoint_output_lines = Some(complete_lines.min(lines.len()));
            }
        }
        match (state.checkpoint_info, state.checkpoint_output_lines) {
            (Some((kind, source_bytes)), Some(output_lines)) => Some(Checkpoint {
                source_bytes,
                output_lines,
                kind,
            }),
            _ => None,
        }
    }
}
