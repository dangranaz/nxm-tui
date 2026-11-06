//! Derived from xai-grok-markdown .../theme/md_style.rs + theme/tokyonight.rs (xai-org/grok-build, Apache-2.0). Modified: Theme::current() replaced by inlined tokyonight constants (fixed single theme; multi-theme deferred).

use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use ratatui::style::{Color as RatColor, Modifier};

use super::style::MarkdownStyle;

/// Converts a ratatui color to an anstyle color. `Reset` maps to `None`
/// (NO_COLOR / terminal default).
pub fn to_anstyle(c: RatColor) -> Option<Color> {
    Some(match c {
        RatColor::Reset => return None,
        RatColor::Rgb(r, g, b) => Color::Rgb(anstyle::RgbColor(r, g, b)),
        RatColor::Indexed(n) => Color::Ansi256(Ansi256Color(n)),
        // Named ANSI colors (from 16-color quantization).
        RatColor::Black => Color::Ansi(AnsiColor::Black),
        RatColor::Red => Color::Ansi(AnsiColor::Red),
        RatColor::Green => Color::Ansi(AnsiColor::Green),
        RatColor::Yellow => Color::Ansi(AnsiColor::Yellow),
        RatColor::Blue => Color::Ansi(AnsiColor::Blue),
        RatColor::Magenta => Color::Ansi(AnsiColor::Magenta),
        RatColor::Cyan => Color::Ansi(AnsiColor::Cyan),
        RatColor::Gray => Color::Ansi(AnsiColor::White),
        RatColor::DarkGray => Color::Ansi(AnsiColor::BrightBlack),
        RatColor::LightRed => Color::Ansi(AnsiColor::BrightRed),
        RatColor::LightGreen => Color::Ansi(AnsiColor::BrightGreen),
        RatColor::LightYellow => Color::Ansi(AnsiColor::BrightYellow),
        RatColor::LightBlue => Color::Ansi(AnsiColor::BrightBlue),
        RatColor::LightMagenta => Color::Ansi(AnsiColor::BrightMagenta),
        RatColor::LightCyan => Color::Ansi(AnsiColor::BrightCyan),
        RatColor::White => Color::Ansi(AnsiColor::BrightWhite),
    })
}

/// `anstyle::Style` with the given foreground color (converted from ratatui).
pub fn fg(c: RatColor) -> Style {
    Style::new().fg_color(to_anstyle(c))
}

/// `anstyle::Style` with the given background color (converted from ratatui).
pub fn bg(c: RatColor) -> Style {
    Style::new().bg_color(to_anstyle(c))
}

pub fn modifier_to_anstyle(m: Modifier) -> Style {
    let mut s = Style::new();
    if m.contains(Modifier::BOLD) {
        s = s.bold();
    }
    if m.contains(Modifier::ITALIC) {
        s = s.italic();
    }
    if m.contains(Modifier::UNDERLINED) {
        s = s.underline();
    }
    if m.contains(Modifier::DIM) {
        s = s.dimmed();
    }
    if m.contains(Modifier::HIDDEN) {
        s = s.hidden();
    }
    if m.contains(Modifier::CROSSED_OUT) {
        s = s.strikethrough();
    }
    s
}

pub fn heading_inner_styles(colors: [RatColor; 6], mods: [Modifier; 6]) -> [Style; 6] {
    std::array::from_fn(|i| {
        let color_style = fg(colors[i]);
        let mod_style = modifier_to_anstyle(mods[i]);
        // Combine fg color with modifier effects.
        let mut s = color_style;
        let effects = mod_style.get_effects();
        if !effects.is_plain() {
            s = s.effects(s.get_effects() | effects);
        }
        s
    })
}

/// Build heading outer styles (dimmed and hidden, for syntax markers).
pub fn heading_outer_styles(colors: [RatColor; 6]) -> [Style; 6] {
    colors.map(|c| fg(c).dimmed().hidden())
}

/// TokyoNight palette constants (Night/Storm variant), inlined from
/// xai-grok-pager-render/src/theme/tokyonight.rs so the fixed theme does not
/// depend on the pager's `struct Theme`.
const C_MD_HEADING_H1: RatColor = RatColor::Rgb(26, 188, 156); // TEAL
const C_MD_HEADING_H2: RatColor = RatColor::Rgb(122, 162, 247); // BLUE
const C_MD_HEADING_H3: RatColor = RatColor::Rgb(255, 158, 100); // ORANGE
const C_MD_HEADING_H4: RatColor = RatColor::Rgb(247, 118, 142); // RED
const C_MD_HEADING_H5: RatColor = RatColor::Rgb(158, 206, 106); // GREEN
const C_MD_HEADING_H6: RatColor = RatColor::Rgb(187, 154, 247); // MAGENTA
const C_MD_CODE: RatColor = RatColor::Rgb(115, 218, 202); // GREEN1
const C_MD_TASK_CHECKED: RatColor = RatColor::Rgb(125, 207, 255); // CYAN
const C_MD_TASK_UNCHECKED: RatColor = RatColor::Rgb(122, 162, 247); // BLUE
const C_MD_MUTED: RatColor = RatColor::Rgb(86, 95, 137); // COMMENT
const C_MD_CODE_BG: RatColor = RatColor::Rgb(41, 46, 66); // BG_HIGHLIGHT
const C_MD_TEXT: RatColor = RatColor::Rgb(192, 202, 245); // FG
const C_LINK_FG: RatColor = RatColor::Rgb(122, 162, 247); // BLUE

/// Fixed single theme: a concrete tokyonight `MarkdownStyle`, built without
/// `Theme::current()`. Multi-theme is deferred.
///
/// Returns the same `MarkdownStyle` shape as the upstream `build_style()`,
/// field for field, but with inlined tokyonight RGB constants instead of
/// reading `theme.md_*`.
pub fn fixed_style() -> MarkdownStyle {
    let heading_colors = [
        C_MD_HEADING_H1,
        C_MD_HEADING_H2,
        C_MD_HEADING_H3,
        C_MD_HEADING_H4,
        C_MD_HEADING_H5,
        C_MD_HEADING_H6,
    ];
    let heading_mods = [
        Modifier::BOLD,
        Modifier::BOLD,
        Modifier::BOLD,
        Modifier::BOLD,
        Modifier::BOLD,
        Modifier::BOLD,
    ];

    MarkdownStyle {
        heading_inner: heading_inner_styles(heading_colors, heading_mods),
        heading_outer: heading_outer_styles(heading_colors),
        strong_inner: fg(C_MD_TEXT).bold(),
        strong_outer: Style::new().dimmed().hidden(),
        emphasis_inner: fg(C_MD_TEXT).italic(),
        emphasis_outer: Style::new().dimmed().hidden(),
        strikethrough_inner: fg(C_MD_TEXT).strikethrough(),
        strikethrough_outer: Style::new().dimmed().hidden(),
        inline_code_inner: fg(C_MD_CODE).bold(),
        inline_code_outer: fg(C_MD_CODE).dimmed().hidden(),
        blockquote_outer: fg(C_MD_MUTED).dimmed(),
        task_checked: fg(C_MD_TASK_CHECKED),
        task_unchecked: fg(C_MD_TASK_UNCHECKED).dimmed(),
        list_item: fg(C_MD_MUTED),
        rule: fg(C_MD_MUTED),
        link_outer: fg(C_MD_MUTED),
        link_text: fg(C_LINK_FG).underline(),
        link_url: fg(C_MD_MUTED),
        link_title: fg(C_MD_HEADING_H5),
        code_outer: fg(C_MD_CODE).dimmed().hidden(),
        code_language: fg(C_MD_HEADING_H3).hidden(),
        code_untagged: fg(C_MD_TEXT),
        code_background: bg(C_MD_CODE_BG),
        table_outer: fg(C_MD_HEADING_H2).hidden(),
        text: fg(C_MD_TEXT),
        math: fg(C_MD_TEXT).italic(),
    }
}
