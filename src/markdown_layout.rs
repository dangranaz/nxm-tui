//! Display-column layout helpers shared by the markdown renderers (V1/T8).
//!
//! Moved verbatim from `src/markdown.rs` when `render_markdown` became the
//! `USE_NEW_MD` shim (V3/T8) and `RULES.md`'s 300-line cap required the split.
//! Cell-width aware: a CJK ideograph or emoji counts as two columns.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Word-wrap `text` to at most `max` display columns, returning at most one
/// line per wrapped row.
///
/// Adapted from openviking's `crates/ov_cli/src/commands/render_utils.rs`
/// (`wrap_display_text` + `append_word_to_line`, Apache-2.0 — see
/// `THIRD-PARTY-NOTICES.md`). Deviation from upstream: no `max_lines`/ellipsis
/// truncation, because the transcript must render every line (the viewport
/// scrolls already); whitespace runs collapse to single spaces.
pub(crate) fn wrap(text: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return Vec::new();
    }

    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in normalized.split(' ') {
        if append_word(&mut current, word, max) {
            continue;
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            if append_word(&mut current, word, max) {
                continue;
            }
        }

        // A single word wider than the whole row (URL, hash, path): split it on
        // character boundaries so the pane never overflows horizontally.
        let mut segment = String::new();
        let mut segment_width = 0usize;
        for ch in word.chars() {
            let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if !segment.is_empty() && segment_width + char_width > max {
                lines.push(std::mem::take(&mut segment));
                segment_width = 0;
            }
            segment.push(ch);
            segment_width += char_width;
        }
        current = segment;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

/// Append `word` to `line` (space-separated) while it fits in `max` columns.
///
/// Returns whether the word was appended.
fn append_word(line: &mut String, word: &str, max: usize) -> bool {
    let separator = usize::from(!line.is_empty());
    if line.width() + separator + word.width() > max {
        return false;
    }
    if !line.is_empty() {
        line.push(' ');
    }
    line.push_str(word);
    true
}

/// Truncate `text` to at most `max` display columns, never splitting a char.
pub(crate) fn truncate_to_width(text: &str, max: usize) -> String {
    let mut truncated = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + char_width > max {
            break;
        }
        truncated.push(ch);
        width += char_width;
    }
    truncated
}
