//! Contract tests for `src/md/theme.rs` (V3/T2 fixed tokyonight theme).
//! Moved from the upstream `#[cfg(test)] mod tests` in `md_style.rs`.

use anstyle::{AnsiColor, Color, Effects};
use nxm_tui::md::style::MarkdownStyle;
use nxm_tui::md::theme::{bg, fg, fixed_style, to_anstyle};
use ratatui::style::Color as RatColor;

/// Regression: `Reset` used to fall back to a concrete `AnsiColor::White` (ANSI-7 silver).
/// That rendered Reset-themed markdown washed-out gray on light terminals and broke the `NO_COLOR` opt-out.
#[test]
fn reset_maps_to_no_color() {
    assert_eq!(to_anstyle(RatColor::Reset), None);
    assert_eq!(fg(RatColor::Reset).get_fg_color(), None);
    assert_eq!(bg(RatColor::Reset).get_bg_color(), None);
}

/// Spot checks around the Gray/DarkGray naming mismatch between ratatui and anstyle.
#[test]
fn named_colors_map_concretely() {
    assert_eq!(
        to_anstyle(RatColor::DarkGray),
        Some(Color::Ansi(AnsiColor::BrightBlack))
    );
    assert_eq!(
        to_anstyle(RatColor::Gray),
        Some(Color::Ansi(AnsiColor::White))
    );
    assert_eq!(to_anstyle(RatColor::Red), Some(Color::Ansi(AnsiColor::Red)));
}

/// The fixed tokyonight theme exposes the expected concrete fields.
#[test]
fn fixed_style_has_tokyonight_fields() {
    let s: MarkdownStyle = fixed_style();

    // `text`: truecolor fg, not None.
    assert!(s.text.get_fg_color().is_some());

    // `strong_inner`: BOLD.
    assert!(s.strong_inner.get_effects().contains(Effects::BOLD));

    // `emphasis_inner`: ITALIC.
    assert!(s.emphasis_inner.get_effects().contains(Effects::ITALIC));

    // `link_text`: UNDERLINED.
    assert!(s.link_text.get_effects().contains(Effects::UNDERLINE));

    // `heading_inner[0]`: fg + BOLD (tokyonight H1 = TEAL + BOLD).
    assert!(s.heading_inner[0].get_fg_color().is_some());
    assert!(s.heading_inner[0].get_effects().contains(Effects::BOLD));
}
