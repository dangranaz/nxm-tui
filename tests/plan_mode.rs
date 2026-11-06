//! Plan mode tests (MT-2 follow-up): `enter_plan_mode`/`exit_plan_mode` tools,
//! shared `PlanModeHandle`, mechanical write-abstaining manifest filter, and
//! mode_bar indicator. TDD: tests written first, then the filter is implemented
//! until all pass.

use std::sync::atomic::Ordering;

use nxm_tui::agent::Agent;
use nxm_tui::app::App;
use nxm_tui::enter_plan_mode::EnterPlanMode;
use nxm_tui::exit_plan_mode::ExitPlanMode;
use nxm_tui::goal::goal_handle;
use nxm_tui::mode_bar::render as render_mode_bar;
use nxm_tui::plan_mode::plan_mode_handle;
use nxm_tui::todo::todo_list_handle;
use nxm_tui::todo_tool::Todo;
use nxm_tui::update_goal::UpdateGoal;
use nxm_tui_tools::{default_registry, Registry, Tool};
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

fn tool_names_in_body(body: &serde_json::Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t["function"]["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn plan_mode_tools_schema() {
    // R1: enter_plan_mode / exit_plan_mode exist, name/description/schema correct.
    let handle = plan_mode_handle();
    let enter = EnterPlanMode {
        goal: handle.clone(),
    };
    assert_eq!(enter.name(), "enter_plan_mode");
    assert!(!enter.description().is_empty());
    let schema = enter.schema();
    assert_eq!(schema["required"], json!([]));
    assert!(schema["properties"].is_object());

    let handle2 = plan_mode_handle();
    let exit = ExitPlanMode {
        goal: handle2.clone(),
    };
    assert_eq!(exit.name(), "exit_plan_mode");
    assert!(!exit.description().is_empty());
    let schema = exit.schema();
    assert_eq!(schema["required"], json!([]));
    assert!(schema["properties"].is_object());
}

#[test]
fn plan_mode_enter_exit() {
    // R2: enter → handle true; exit → handle false.
    let handle = plan_mode_handle();
    assert!(!handle.load(Ordering::Relaxed));

    let tool = EnterPlanMode {
        goal: handle.clone(),
    };
    let res = tool.run(&json!({})).unwrap();
    assert!(res.ok, "enter should succeed, got: {:?}", res.content);
    assert!(handle.load(Ordering::Relaxed));

    let tool = ExitPlanMode {
        goal: handle.clone(),
    };
    let res = tool.run(&json!({})).unwrap();
    assert!(res.ok, "exit should succeed, got: {:?}", res.content);
    assert!(!handle.load(Ordering::Relaxed));
}

#[test]
fn plan_mode_shares_state() {
    // R3: registering the tools with handle.clone() and calling via
    // registry.call makes the same handle (the app's one) reflect on/off.
    let handle = plan_mode_handle();
    let mut reg = Registry::new();
    reg.register(EnterPlanMode {
        goal: handle.clone(),
    });
    reg.register(ExitPlanMode {
        goal: handle.clone(),
    });

    let res = reg.call("enter_plan_mode", &json!({})).unwrap();
    assert!(res.ok, "registry call enter should succeed");
    assert!(handle.load(Ordering::Relaxed));

    let res = reg.call("exit_plan_mode", &json!({})).unwrap();
    assert!(res.ok, "registry call exit should succeed");
    assert!(!handle.load(Ordering::Relaxed));
}

#[test]
fn request_body_plan_mode_excludes_writes() {
    // R4: plan mode ON → body does NOT contain search_replace/web_fetch/web_search,
    // but DOES contain Core/grep/todo/update_goal when selected by the task.
    let client = reqwest::Client::new();
    let plan_handle = plan_mode_handle();
    plan_handle.store(true, Ordering::Relaxed);

    let mut reg = default_registry();
    reg.register(Todo {
        todos: todo_list_handle(),
    });
    reg.register(UpdateGoal {
        goal: goal_handle(),
    });
    // Re-register with the actual plan handle for enter/exit.
    reg.register(EnterPlanMode {
        goal: plan_handle.clone(),
    });
    reg.register(ExitPlanMode {
        goal: plan_handle.clone(),
    });

    let agent = Agent::new(
        &client,
        "http://localhost",
        "test-model",
        None,
        vec![nxm_tui::app::Message::new(
            nxm_tui::app::Role::User,
            "search edit fetch todo goal plan".into(),
        )],
        None,
        reg,
        plan_handle,
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    );

    let body = agent.request_body();
    let names = tool_names_in_body(&body);

    // Write tools must be excluded.
    assert!(
        !names.contains(&"search_replace".to_string()),
        "search_replace must be excluded in plan mode: {names:?}"
    );
    assert!(
        !names.contains(&"web_fetch".to_string()),
        "web_fetch must be excluded in plan mode: {names:?}"
    );
    assert!(
        !names.contains(&"web_search".to_string()),
        "web_search must be excluded in plan mode: {names:?}"
    );

    // Core / Search / Other must remain.
    assert!(
        names.contains(&"read_file".to_string()),
        "read_file must be present in plan mode: {names:?}"
    );
    assert!(
        names.contains(&"grep".to_string()),
        "grep must be present in plan mode: {names:?}"
    );
    assert!(
        names.contains(&"todo".to_string()),
        "todo must be present in plan mode: {names:?}"
    );
    assert!(
        names.contains(&"update_goal".to_string()),
        "update_goal must be present in plan mode: {names:?}"
    );
    assert!(
        names.contains(&"enter_plan_mode".to_string()),
        "enter_plan_mode must be present in plan mode: {names:?}"
    );
    assert!(
        names.contains(&"exit_plan_mode".to_string()),
        "exit_plan_mode must be present in plan mode: {names:?}"
    );
}

#[test]
fn request_body_plan_mode_off_unchanged() {
    // R5: plan mode OFF → body identical to today (write tools available per
    // normal per-task selection).
    let client = reqwest::Client::new();
    let plan_handle = plan_mode_handle();
    plan_handle.store(false, Ordering::Relaxed);

    let mut reg = default_registry();
    reg.register(EnterPlanMode {
        goal: plan_handle.clone(),
    });
    reg.register(ExitPlanMode {
        goal: plan_handle.clone(),
    });

    let agent = Agent::new(
        &client,
        "http://localhost",
        "test-model",
        None,
        vec![nxm_tui::app::Message::new(
            nxm_tui::app::Role::User,
            "edit fetch google".into(),
        )],
        None,
        reg,
        plan_handle,
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    );

    let body = agent.request_body();
    let names = tool_names_in_body(&body);

    // Write tools must be present when plan mode is OFF.
    assert!(
        names.contains(&"search_replace".to_string()),
        "search_replace must be present when plan mode OFF: {names:?}"
    );
    assert!(
        names.contains(&"web_fetch".to_string()),
        "web_fetch must be present when plan mode OFF: {names:?}"
    );
    assert!(
        names.contains(&"web_search".to_string()),
        "web_search must be present when plan mode OFF: {names:?}"
    );

    // Core tools always present.
    assert!(
        names.contains(&"read_file".to_string()),
        "read_file must be present: {names:?}"
    );
}

#[test]
fn mode_bar_shows_plan() {
    // R6: handle ON → mode_bar contains "PLAN".
    let app = App::new();
    app.plan_mode_handle.store(true, Ordering::Relaxed);
    let text = screen_text_mode_bar(&app);
    assert!(text.contains("PLAN"), "mode_bar should show PLAN: {text}");
}

#[test]
fn mode_bar_no_plan_when_off() {
    // R6: handle OFF → mode_bar does NOT contain "PLAN".
    let app = App::new();
    let text = screen_text_mode_bar(&app);
    assert!(
        !text.contains("PLAN"),
        "mode_bar should not show PLAN when off: {text}"
    );
}
