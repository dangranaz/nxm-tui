//! Tools selection overlay render test (T1, TestBackend — no TTY).
//!
//! Proves `render_tools` lists every registered tool with the correct
//! `[x]`/`[ ]` marker and the cursor row highlighted.

use nxm_tui::app::App;
use nxm_tui::tools_overlay::render_tools;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen_text(app: &App) -> String {
    let backend = TestBackend::new(60, 16);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render_tools(f, app))
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
fn tools_overlay_marks_all_registered_tools_inactive_by_default() {
    let app = App::new();
    let text = screen_text(&app);
    assert!(text.contains("Tools"), "overlay title present");
    // MT-3: default manual set is empty, so each shows [ ].
    assert!(text.contains("[ ]"), "inactive marker: {text}");
    assert!(text.contains("read_file"), "read_file listed");
    assert!(text.contains("list_resources"), "list_resources listed");
    assert!(text.contains("web_search"), "web_search listed");
}

#[test]
fn tools_overlay_marks_an_enabled_tool_active() {
    let mut app = App::new();
    // MT-3: default is empty; explicitly enable web_search.
    app.tool_registry.set_enabled("web_search", true);
    let text = screen_text(&app);
    let web_row = text
        .lines()
        .find(|l| l.contains("web_search"))
        .expect("web_search row present");
    assert!(
        web_row.contains("[x]"),
        "active marker on web_search row: {web_row}"
    );
    let read_row = text
        .lines()
        .find(|l| l.contains("read_file"))
        .expect("read_file row present");
    assert!(
        read_row.contains("[ ]"),
        "inactive marker on read_file row (default): {read_row}"
    );
}

#[test]
fn tools_overlay_shows_cursor_row() {
    let mut app = App::new();
    app.tools_cursor = 1; // second tool
    let text = screen_text(&app);
    // Row 1 = list_resources (registration order: read_file, list_resources, web_search).
    let cursor_row = text
        .lines()
        .find(|l| l.contains("list_resources"))
        .expect("list_resources row present");
    assert!(
        cursor_row.contains(">"),
        "cursor marker on row 1: {cursor_row}"
    );
}

#[test]
fn tools_overlay_renders_in_a_tiny_terminal_without_panicking() {
    // Degenerate case: fewer than the overlay's 8-row minimum. The height
    // clamp must cap at the frame, not overshoot it (ratatui clips, but the
    // rect itself should stay inside the area).
    let app = App::new();
    let backend = TestBackend::new(60, 4);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render_tools(f, &app))
        .expect("draw must not panic");
}
