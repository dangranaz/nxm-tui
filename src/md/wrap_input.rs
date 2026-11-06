//! Derived from xai-grok-pager-render/src/render/wrapping.rs (xai-org/grok-build, Apache-2.0). Modified: split for the 300-line cap; crate::md::line_utils paths; inline tests adapted in tests/.
//! Split part 3/3 (prod lines 511-579, 634-690): LineInput/IntoLineInput (pub: visibility-for-testability for tests/md_wrap.rs) + header wrap helpers.

use crate::md::wrap_line::word_wrap_lines;
use ratatui::text::{Line, Span};
use std::borrow::Cow;

/// Utilities to allow wrapping either borrowed or owned lines.
#[derive(Debug)]
pub enum LineInput<'a> {
    Borrowed(&'a Line<'a>),
    Owned(Line<'a>),
}

impl<'a> LineInput<'a> {
    pub(crate) fn as_ref(&self) -> &Line<'a> {
        match self {
            LineInput::Borrowed(line) => line,
            LineInput::Owned(line) => line,
        }
    }
}

/// This trait makes it easier to pass whatever we need into word_wrap_lines.
pub trait IntoLineInput<'a> {
    fn into_line_input(self) -> LineInput<'a>;
}

impl<'a> IntoLineInput<'a> for &'a Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for &'a mut Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Borrowed(self)
    }
}

impl<'a> IntoLineInput<'a> for Line<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(self)
    }
}

impl<'a> IntoLineInput<'a> for String {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for &'a str {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Cow<'a, str> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Span<'a> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}

impl<'a> IntoLineInput<'a> for Vec<Span<'a>> {
    fn into_line_input(self) -> LineInput<'a> {
        LineInput::Owned(Line::from(self))
    }
}
/// Word-wrap a header line with hanging indent.
///
/// The first span stays on line 1; remaining content wraps with `extra_indent + prefix_width` hanging indent on continuation lines.
pub fn wrap_header_hanging(
    header: Line<'static>,
    width: usize,
    extra_indent: usize,
) -> Vec<Line<'static>> {
    if header.spans.is_empty() {
        return vec![header];
    }

    let Some(prefix_span) = header.spans.first().cloned() else {
        return vec![header];
    };
    let prefix_width = unicode_width::UnicodeWidthStr::width(prefix_span.content.as_ref());
    let total_indent = extra_indent + prefix_width;
    let wrap_width = width.saturating_sub(total_indent);

    if wrap_width == 0 || header.spans.len() < 2 {
        return word_wrap_lines(std::iter::once(header), width);
    }

    let Some(rest) = header.spans.get(1..) else {
        return word_wrap_lines(std::iter::once(header), width);
    };
    let content_line = Line::from(rest.to_vec());
    let mut wrapped = word_wrap_lines(std::iter::once(content_line), wrap_width);

    if let Some(first) = wrapped.first_mut() {
        first.spans.insert(0, prefix_span);
    }
    if total_indent > 0 {
        let indent_str: String = " ".repeat(total_indent);
        for line in wrapped.iter_mut().skip(1) {
            line.spans
                .insert(0, ratatui::text::Span::raw(indent_str.clone()));
        }
    }

    wrapped
}

/// Continuation lines indent to a fixed column (usually the bullet width), not under the content after the prefix.
pub fn wrap_header_flush(header: Line<'static>, width: usize, indent: usize) -> Vec<Line<'static>> {
    let wrap_width = width.saturating_sub(indent);
    let mut wrapped = word_wrap_lines(std::iter::once(header), wrap_width);
    if indent > 0 {
        let indent_str: String = " ".repeat(indent);
        for line in wrapped.iter_mut().skip(1) {
            line.spans
                .insert(0, ratatui::text::Span::raw(indent_str.clone()));
        }
    }
    wrapped
}
