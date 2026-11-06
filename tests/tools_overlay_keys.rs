//! Tools overlay keyboard tests (T1): cursor movement, Space toggle,
//! a/n bulk enable-disable, and Esc close — all routed through `handle_key`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nxm_tui::app::App;
use nxm_tui::handler::handle_key;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn ctrl_e_opens_overlay_and_esc_closes_it() {
    let mut app = App::new();
    handle_key(&mut app, ctrl('e'));
    assert!(app.show_tools, "Ctrl+E opens the Tools overlay");
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.show_tools, "Esc closes the Tools overlay");
}

#[test]
fn arrow_keys_move_cursor_within_bounds() {
    let mut app = App::new();
    let len = app.tool_registry.tool_names().len();
    assert!(len > 0);
    handle_key(&mut app, ctrl('e'));

    // Up at the top wraps to the last row; Down advances and wraps to 0.
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.tools_cursor, len - 1, "Up at top wraps to last");
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.tools_cursor, 0, "Down from last wraps to first");
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.tools_cursor, 1, "Down advances by one");
}

#[test]
fn space_toggles_the_tool_under_cursor() {
    let mut app = App::new();
    handle_key(&mut app, ctrl('e'));
    let name = app.tool_registry.tool_names()[app.tools_cursor].to_string();
    assert!(!app.tool_registry.is_enabled(&name), "disabled by default");

    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert!(app.tool_registry.is_enabled(&name), "Space enables it");
    handle_key(&mut app, key(KeyCode::Char(' ')));
    assert!(
        !app.tool_registry.is_enabled(&name),
        "Space disables it again"
    );
}

#[test]
fn a_and_n_bulk_enable_and_disable() {
    let mut app = App::new();
    handle_key(&mut app, ctrl('e'));

    // MT-3: default manual set is empty.
    let none_enabled = app
        .tool_registry
        .tool_names()
        .iter()
        .all(|n| !app.tool_registry.is_enabled(n));
    assert!(none_enabled, "default: no tool manually enabled");

    handle_key(&mut app, key(KeyCode::Char('n')));
    let any_enabled = app
        .tool_registry
        .tool_names()
        .iter()
        .any(|n| app.tool_registry.is_enabled(n));
    assert!(!any_enabled, "n disables all tools");

    handle_key(&mut app, key(KeyCode::Char('a')));
    let all_enabled = app
        .tool_registry
        .tool_names()
        .iter()
        .all(|n| app.tool_registry.is_enabled(n));
    assert!(all_enabled, "a enables all tools");
}

#[test]
fn overlay_swallows_keys_that_are_not_shortcuts() {
    // Review finding: while the overlay is open no key may fall through to
    // the prompt (typing, Enter-to-submit) — same contract as search mode.
    let mut app = App::new();
    handle_key(&mut app, ctrl('e'));
    handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(
        app.prompt_state.text().is_empty(),
        "prompt untouched while overlay is open"
    );
    handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        app.messages.is_empty(),
        "Enter must not submit a message with the overlay open"
    );
    assert!(
        app.input.is_empty(),
        "Enter must not stage a message with the overlay open"
    );
    assert!(app.show_tools, "unknown keys leave the overlay open");
}

#[test]
fn overlay_disable_all_keeps_core_in_body() {
    // End-to-end seam: overlay disable-all -> App registry -> agent snapshot body.
    // With the MT-3 empty default, disable-all leaves only the automatic
    // fail-safe (Core) in the body when there is no user task.
    use nxm_tui::agent::Agent;
    use reqwest::Client;

    let mut app = App::new();
    handle_key(&mut app, ctrl('e'));
    handle_key(&mut app, key(KeyCode::Char('n'))); // disable all (manual set)

    let client = Client::new();
    let agent = Agent::new(
        &client,
        "http://localhost:1",
        "test-model",
        None,
        Vec::new(),
        None,
        app.tool_registry.clone(),
        app.plan_mode_handle.clone(),
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    );
    let body = agent.request_body();
    let names: Vec<&str> = body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert_eq!(names, vec!["read_file", "list_resources"]);
}
