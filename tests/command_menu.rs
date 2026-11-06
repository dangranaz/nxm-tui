//! Tests for the slash-command completion popup: alphabetical ordering,
//! prefix filtering, selection navigation, completion, and headless render.

#![allow(dead_code, unused_imports)]

use nxm_tui::app::{self, App};
use nxm_tui::autocomplete;
use nxm_tui::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen_text(w: u16, h: u16, draw: impl FnOnce(&mut ratatui::Frame)) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal.draw(draw).expect("draw must not panic");
    let buf = terminal.backend().buffer().clone();
    let (bw, bh) = (buf.area.width as usize, buf.area.height as usize);
    let mut out = String::new();
    for y in 0..bh {
        for x in 0..bw {
            if let Some(cell) = buf.cell((x as u16, y as u16)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

/// Type a string into the prompt (as insert_char would during real input).
fn type_prompt(app: &mut App, s: &str) {
    for c in s.chars() {
        app.prompt_state.insert_char(c);
    }
    app.command_menu_dismissed = false;
    app.clamp_command_menu();
}

#[test]
fn menu_matches_are_alphabetical_for_bare_slash() {
    let menu = autocomplete::menu_matches("/");
    let cmds: Vec<&str> = menu.iter().map(|(c, _)| *c).collect();
    let mut sorted = cmds.clone();
    sorted.sort();
    assert_eq!(cmds, sorted, "commands must be listed alphabetically");
    // Spot-check the alphabetical extremes.
    assert_eq!(cmds.first(), Some(&"/clear"));
    assert_eq!(cmds.last(), Some(&"/sidebar"));
}

#[test]
fn menu_matches_filters_by_prefix() {
    let menu = autocomplete::menu_matches("/se");
    let cmds: Vec<&str> = menu.iter().map(|(c, _)| *c).collect();
    assert_eq!(cmds, vec!["/server"]);
}

#[test]
fn menu_matches_prefix_is_case_insensitive() {
    let menu = autocomplete::menu_matches("/PRO");
    let cmds: Vec<&str> = menu.iter().map(|(c, _)| *c).collect();
    assert_eq!(cmds, vec!["/provider"]);
}

#[test]
fn menu_matches_empty_when_not_a_slash_command() {
    assert!(autocomplete::menu_matches("hello").is_empty());
    assert!(autocomplete::menu_matches("").is_empty());
}

#[test]
fn args_do_not_narrow_the_menu() {
    // Once past the root token, the popup still lists the root command only.
    let menu = autocomplete::menu_matches("/provider use foo");
    let cmds: Vec<&str> = menu.iter().map(|(c, _)| *c).collect();
    assert_eq!(cmds, vec!["/provider"]);
}

#[test]
fn popup_opens_only_for_slash_prefix() {
    let mut app = App::new();
    assert!(!app.command_menu_open());
    type_prompt(&mut app, "hi");
    assert!(!app.command_menu_open());

    let mut app = App::new();
    type_prompt(&mut app, "/");
    assert!(app.command_menu_open());
}

#[test]
fn selection_wraps_and_completes() {
    let mut app = App::new();
    type_prompt(&mut app, "/");
    assert_eq!(app.command_menu_selection(), Some("/clear"));

    app.command_menu_up(); // wrap to last
    assert_eq!(app.command_menu_selection(), Some("/sidebar"));

    app.command_menu_down(); // wrap back to first
    assert_eq!(app.command_menu_selection(), Some("/clear"));

    app.command_menu_down(); // -> /config
    assert_eq!(app.command_menu_selection(), Some("/config"));
    app.complete_command();
    // /config takes args -> trailing space, popup dismissed.
    assert_eq!(app.prompt_state.text(), "/config ");
    assert!(!app.command_menu_open());
}

#[test]
fn completing_argless_command_leaves_no_trailing_space() {
    let mut app = App::new();
    type_prompt(&mut app, "/qu");
    assert_eq!(app.command_menu_selection(), Some("/quit"));
    app.complete_command();
    assert_eq!(app.prompt_state.text(), "/quit");
    assert!(!app.command_menu_open());
}

#[test]
fn editing_reopens_after_dismiss() {
    let mut app = App::new();
    type_prompt(&mut app, "/qu");
    app.command_menu_dismissed = true;
    assert!(!app.command_menu_open());
    // Simulate another keystroke.
    type_prompt(&mut app, "i");
    assert!(app.command_menu_open());
}

#[test]
fn popup_renders_commands_over_prompt() {
    let mut app = App::new();
    app.state = app::RunState::Running;
    type_prompt(&mut app, "/");
    let text = screen_text(100, 30, |f| ui::render(f, &mut app));
    assert!(text.contains("Commands"));
    assert!(text.contains("/clear"));
    assert!(text.contains("/provider"));
    assert!(text.contains("/quit"));
}

#[test]
fn popup_filtered_render_shows_single_match() {
    let mut app = App::new();
    app.state = app::RunState::Running;
    type_prompt(&mut app, "/pro");
    let text = screen_text(100, 30, |f| ui::render(f, &mut app));
    assert!(text.contains("/provider"));
    assert!(!text.contains("/quit"));
}
