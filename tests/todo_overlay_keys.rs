//! Todo overlay keyboard tests (MT-3): Ctrl+D opens/closes, Esc closes —
//! routed through `handle_key`.

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
fn todo_overlay_toggle_key() {
    // R4: Ctrl+D opens the Todo overlay; Esc closes it.
    let mut app = App::new();
    assert!(!app.show_todo, "todo overlay hidden by default");

    handle_key(&mut app, ctrl('d'));
    assert!(app.show_todo, "Ctrl+D opens the Todo overlay");

    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.show_todo, "Esc closes the Todo overlay");
}

#[test]
fn todo_overlay_toggle_key_idempotent() {
    // R4: pressing Ctrl+D again while open closes it.
    let mut app = App::new();
    handle_key(&mut app, ctrl('d'));
    assert!(app.show_todo);
    handle_key(&mut app, ctrl('d'));
    assert!(!app.show_todo, "Ctrl+D toggles the Todo overlay closed");
}

#[test]
fn todo_overlay_no_key_conflict() {
    // R5: Ctrl+D is not used by any other overlay/action. The existing
    // Ctrl bindings are: Ctrl+Q quit, Ctrl+S save session, Ctrl+H help,
    // Ctrl+T sessions, Ctrl+M metrics, Ctrl+U context, Ctrl+R thinking,
    // Ctrl+O tool detail, Ctrl+E tools overlay, Ctrl+B sidebar.
    // Ctrl+D is free.
    let mut app = App::new();

    // Verify other overlays still work as before (no regression).
    handle_key(&mut app, ctrl('e'));
    assert!(app.show_tools, "Ctrl+E still opens tools overlay");
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.show_tools, "Esc closes tools overlay");

    // Ctrl+D opens the todo overlay.
    handle_key(&mut app, ctrl('d'));
    assert!(app.show_todo, "Ctrl+D opens todo overlay");
    handle_key(&mut app, key(KeyCode::Esc));
    assert!(!app.show_todo, "Esc closes todo overlay");
}
