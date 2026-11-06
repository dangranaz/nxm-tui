//! Derived from xai-grok-markdown-core/src/lib.rs (xai-org/grok-build, Apache-2.0).
//! Modified: copied only the parsing core (parser_options, offset_events,
//! DoubleTildeOnlyStrike); MarkdownStats/analyze deferred to T3b.
//!
//! [`parser_options`] is the single source of truth for the parser feature set,
//! so analysis and rendering agree on what counts as strikethrough. After
//! parsing, [`offset_events`] keeps only `~~…~~` as strikethrough: single-tilde
//! pairs (`~text~`), which pulldown treats as strike, are demoted to literal `~`
//! text so LLM output like `~**10%**` is not struck.

#![deny(clippy::indexing_slicing)]

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// The exact `pulldown-cmark` option set Grok Build uses to render markdown.
/// With `ENABLE_STRIKETHROUGH`, pulldown treats both `~~…~~` and single-`~` pairs as strike.
/// Callers must consume events via [`offset_events`] so only double-tilde strikethrough is retained.
pub fn parser_options() -> Options {
    Options::ENABLE_GFM
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_MATH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_TABLES
}

/// Returns Grok's parser events with source byte ranges, single-tilde strikethrough already demoted.
///
/// Prefer this over `Parser::new_ext(...).into_offset_iter()` so analysis and rendering agree on what counts as strikethrough.
pub fn offset_events(text: &str) -> impl Iterator<Item = (Event<'_>, Range<usize>)> + '_ {
    DoubleTildeOnlyStrike {
        text,
        events: Parser::new_ext(text, parser_options()).into_offset_iter(),
    }
}

/// Filter with no pairing stack: pulldown gives Start and End the same byte span, so each is classified by whether that span opens with `~~`.
/// A single-tilde Start or End becomes its `~` delimiter as a `Text` event, since pulldown emits no separate delimiter events.
struct DoubleTildeOnlyStrike<'a, I> {
    text: &'a str,
    events: I,
}

/// True when the strike span at `range.start` is the double-tilde form.
fn is_double_tilde_strike(text: &str, range: &Range<usize>) -> bool {
    text.get(range.start..).is_some_and(|s| s.starts_with("~~"))
}

/// Returns the opening or closing delimiter byte as a `Text` event, with the matching source range.
fn strike_delim_text<'a>(
    text: &'a str,
    range: &Range<usize>,
    opening: bool,
) -> (Event<'a>, Range<usize>) {
    let delim = if opening {
        let end = range.start + 1;
        debug_assert!(text.is_char_boundary(end) && end <= text.len());
        (range.start..end, text.get(range.start..end).unwrap_or(""))
    } else {
        let Some(start) = range.end.checked_sub(1) else {
            return (Event::Text("".into()), range.clone());
        };
        debug_assert!(text.is_char_boundary(start) && start < text.len());
        (start..range.end, text.get(start..range.end).unwrap_or(""))
    };
    (Event::Text(delim.1.into()), delim.0)
}

impl<'a, I> Iterator for DoubleTildeOnlyStrike<'a, I>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    type Item = (Event<'a>, Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let (event, range) = self.events.next()?;
        match &event {
            Event::Start(Tag::Strikethrough) if !is_double_tilde_strike(self.text, &range) => {
                Some(strike_delim_text(self.text, &range, true))
            }
            Event::End(TagEnd::Strikethrough) if !is_double_tilde_strike(self.text, &range) => {
                Some(strike_delim_text(self.text, &range, false))
            }
            _ => Some((event, range)),
        }
    }
}
