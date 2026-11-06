//! Update-goal tool tests (MT-1): `UpdateGoal` mutates the shared
//! `Arc<Mutex<Option<String>>>` goal state and returns a confirmation.
//! No UI, no overlay — just the tool + shared state, testable in isolation.

use nxm_tui::app::App;
use nxm_tui::goal::goal_handle;
use nxm_tui::mode_bar::render as render_mode_bar;
use nxm_tui::update_goal::UpdateGoal;
use nxm_tui_tools::{Registry, Tool};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use serde_json::json;

fn screen_text_mode_bar(app: &App) -> String {
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render_mode_bar(f, app, Rect::new(0, 0, 80, 1)))
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
fn update_goal_schema() {
    // R1: name == "update_goal", non-empty description, schema exposes `goal`
    // (string, required).
    let handle = goal_handle();
    let tool = UpdateGoal { goal: handle };
    assert_eq!(tool.name(), "update_goal");
    assert!(!tool.description().is_empty());
    let schema = tool.schema();
    assert_eq!(schema["required"], json!(["goal"]));
    assert!(schema["properties"]["goal"].is_object());
    assert_eq!(schema["properties"]["goal"]["type"], "string");
}

#[test]
fn update_goal_sets() {
    // R2: run(set) -> ok, handle sees Some(text), content confirms.
    let handle = goal_handle();
    let tool = UpdateGoal {
        goal: handle.clone(),
    };
    let res = tool.run(&json!({ "goal": "ship v1" })).unwrap();
    assert!(res.ok, "set should succeed, got: {:?}", res.content);
    assert!(res.content.contains("ship v1"));
    let guard = handle.lock().unwrap();
    assert_eq!(*guard, Some("ship v1".to_string()));
}

#[test]
fn update_goal_empty() {
    // R3: empty goal -> ok==false, state unchanged.
    let handle = goal_handle();
    let tool = UpdateGoal {
        goal: handle.clone(),
    };
    let res = tool.run(&json!({ "goal": "" })).unwrap();
    assert!(!res.ok, "empty goal should fail");
    let guard = handle.lock().unwrap();
    assert_eq!(*guard, None);
}

#[test]
fn update_goal_missing() {
    // R3: missing goal -> ok==false, state unchanged.
    let handle = goal_handle();
    let tool = UpdateGoal {
        goal: handle.clone(),
    };
    let res = tool.run(&json!({})).unwrap();
    assert!(!res.ok, "missing goal should fail");
    let guard = handle.lock().unwrap();
    assert_eq!(*guard, None);
}

#[test]
fn update_goal_shares_state() {
    // R4: registering UpdateGoal { goal: handle.clone() } and mutating via
    // registry.call("update_goal", ...) reflects in the same handle.
    let handle = goal_handle();
    let mut reg = Registry::new();
    reg.register(UpdateGoal {
        goal: handle.clone(),
    });
    let res = reg
        .call("update_goal", &json!({ "goal": "shared goal" }))
        .unwrap();
    assert!(res.ok, "registry call should succeed");
    let guard = handle.lock().unwrap();
    assert_eq!(*guard, Some("shared goal".to_string()));
}

#[test]
fn mode_bar_shows_goal() {
    // R5: with app.goal_handle = Some("ship v1"), mode_bar contains the goal.
    let app = App::new();
    app.goal_handle
        .lock()
        .unwrap()
        .replace("ship v1".to_string());
    let text = screen_text_mode_bar(&app);
    assert!(
        text.contains("ship v1"),
        "mode_bar should show goal: {text}"
    );
}

#[test]
fn mode_bar_no_goal_when_none() {
    // R5: with None goal, mode_bar contains no goal marker.
    let app = App::new();
    let text = screen_text_mode_bar(&app);
    assert!(
        !text.contains("goal"),
        "mode_bar should not show goal when None: {text}"
    );
    assert!(
        !text.contains("🎯"),
        "mode_bar should not show goal emoji when None: {text}"
    );
}
