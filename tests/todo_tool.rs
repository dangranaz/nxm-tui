//! Todo tool tests (MT-2): the `todo` tool mutates the shared
//! `Arc<Mutex<TodoList>>` and returns `render()` to the model. No UI, no
//! overlay — just the tool + shared state, testable in isolation.

use nxm_tui::todo::todo_list_handle;
use nxm_tui::todo_tool::Todo;
use nxm_tui_tools::{Registry, Tool};
use serde_json::json;

#[test]
fn todo_tool_schema() {
    // R1: name == "todo", non-empty description, schema exposes `action`
    // (required) + optional `text`/`index`.
    let handle = todo_list_handle();
    let tool = Todo { todos: handle };
    assert_eq!(tool.name(), "todo");
    assert!(!tool.description().is_empty());
    let schema = tool.schema();
    assert_eq!(schema["required"], json!(["action"]));
    assert!(schema["properties"]["action"].is_object());
    assert!(schema["properties"]["text"].is_object());
    assert!(schema["properties"]["index"].is_object());
}

#[test]
fn todo_tool_add() {
    // R2: run(add) -> ok, handle sees 1 item, content contains "- [ ] x".
    let handle = todo_list_handle();
    let tool = Todo {
        todos: handle.clone(),
    };
    let res = tool.run(&json!({ "action": "add", "text": "x" })).unwrap();
    assert!(res.ok, "add should succeed, got: {:?}", res.content);
    assert!(res.content.contains("- [ ] x"));
    let guard = handle.lock().unwrap();
    let items = guard.list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "x");
    assert!(!items[0].done);
}

#[test]
fn todo_tool_complete() {
    // R3: complete by index -> ok, item done, content contains "- [x] x".
    let handle = todo_list_handle();
    handle.lock().unwrap().add("x".to_string());
    let tool = Todo {
        todos: handle.clone(),
    };
    let res = tool
        .run(&json!({ "action": "complete", "index": 0 }))
        .unwrap();
    assert!(res.ok, "complete should succeed, got: {:?}", res.content);
    assert!(res.content.contains("- [x] x"));
    let guard = handle.lock().unwrap();
    let items = guard.list();
    assert!(items[0].done);
}

#[test]
fn todo_tool_complete_invalid() {
    // R3: invalid index/text -> ok==false, list unchanged.
    let handle = todo_list_handle();
    handle.lock().unwrap().add("x".to_string());
    let tool = Todo {
        todos: handle.clone(),
    };

    let res = tool
        .run(&json!({ "action": "complete", "index": 5 }))
        .unwrap();
    assert!(!res.ok, "out-of-range index should fail");

    let res = tool
        .run(&json!({ "action": "complete", "text": "nope" }))
        .unwrap();
    assert!(!res.ok, "unknown text should fail");

    let guard = handle.lock().unwrap();
    let items = guard.list();
    assert_eq!(items.len(), 1);
    assert!(!items[0].done);
}

#[test]
fn todo_tool_list() {
    // R4: run(list) returns current render().
    let handle = todo_list_handle();
    handle.lock().unwrap().add("x".to_string());
    let tool = Todo {
        todos: handle.clone(),
    };
    let res = tool.run(&json!({ "action": "list" })).unwrap();
    assert!(res.ok);
    assert_eq!(res.content, "- [ ] x");
}

#[test]
fn todo_tool_bad_action() {
    // R5: missing/unknown action -> ok==false (ToolResult::err, not Result::Err).
    let tool = Todo {
        todos: todo_list_handle(),
    };
    let res = tool.run(&json!({})).unwrap();
    assert!(!res.ok, "missing action should fail");
    let res = tool.run(&json!({ "action": "nope" })).unwrap();
    assert!(!res.ok, "unknown action should fail");
}

#[test]
fn todo_tool_shares_state_with_handle() {
    // R6: registering Todo { todos: handle.clone() } and mutating via
    // registry.call("todo", ...) reflects in the same handle.
    let handle = todo_list_handle();
    let mut reg = Registry::new();
    reg.register(Todo {
        todos: handle.clone(),
    });
    let res = reg
        .call("todo", &json!({ "action": "add", "text": "shared" }))
        .unwrap();
    assert!(res.ok, "registry call should succeed");
    let guard = handle.lock().unwrap();
    let items = guard.list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "shared");
    assert!(res.content.contains("- [ ] shared"));
}
