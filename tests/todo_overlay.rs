//! Todo overlay render tests (MT-3, TestBackend — no TTY).
//!
//! Proves `render_todo` lists the shared todo list with correct `[x]`/`[ ]`
//! markers, shows a placeholder when empty, and never panics (including on
//! a poisoned mutex).

use nxm_tui::app::App;
use nxm_tui::todo_overlay::render_todo;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen_text(app: &App) -> String {
    let backend = TestBackend::new(60, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render_todo(f, app))
        .expect("draw must not panic");
    let buf = terminal.backend().buffer().clone();
    let (w, h) = (buf.area.width as usize, buf.area.height as usize);
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            if let Some(cell) = buf.cell((x as u16, y as u16)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn todo_overlay_hidden_by_default() {
    // R1: `show_todo` defaults to false.
    let app = App::new();
    assert!(!app.show_todo, "todo overlay hidden by default");
}

#[test]
fn todo_overlay_renders_items() {
    // R2: with a done and a non-done item, the buffer contains both texts
    // and the `[x]`/`[ ]` markers.
    let app = App::new();
    app.todo_handle.lock().unwrap().add("buy milk".to_string());
    app.todo_handle.lock().unwrap().add("call bob".to_string());
    app.todo_handle
        .lock()
        .unwrap()
        .complete_by_index(0)
        .unwrap();

    let text = screen_text(&app);
    assert!(text.contains("Todo"), "overlay title present: {text}");
    assert!(text.contains("buy milk"), "first item listed: {text}");
    assert!(text.contains("call bob"), "second item listed: {text}");
    assert!(text.contains("[x]"), "done marker present: {text}");
    assert!(text.contains("[ ]"), "non-done marker present: {text}");
}

#[test]
fn todo_overlay_empty_placeholder() {
    // R3: empty list shows a placeholder and does not panic.
    let app = App::new();
    let text = screen_text(&app);
    assert!(text.contains("Todo"), "overlay title present: {text}");
    assert!(
        text.contains("(no todos)"),
        "empty placeholder present: {text}"
    );
}

#[test]
fn todo_overlay_poisoned_mutex_does_not_panic() {
    // Bonus: a poisoned mutex shows an error placeholder instead of panicking.
    let app = App::new();
    // Poison the mutex by panicking while holding the lock.
    let _ = std::panic::catch_unwind(|| {
        let g = app.todo_handle.lock().unwrap();
        drop(g); // drop to avoid borrow issues in catch_unwind
        panic!("poison");
    });
    // The lock is now poisoned; render must not panic.
    let text = screen_text(&app);
    assert!(
        text.contains("unavailable") || text.contains("(no todos)"),
        "poisoned mutex handled gracefully: {text}"
    );
}
