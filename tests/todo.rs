//! Todo-list shared state tests (MT-1): `TodoItem`/`TodoList` types + pure
//! mutation logic. No tool, no UI — just types and logic, testable in
//! isolation. The shared state lives as `Arc<Mutex<TodoList>>` so `App` (UI,
//! MT-3) and `Agent` (tool, MT-2) can observe the same list.

use std::sync::Arc;
use std::sync::Mutex;

use nxm_tui::todo::TodoList;

#[test]
fn todo_add_appends() {
    // R1: add appends items in insertion order, all not done.
    let mut list = TodoList::new();
    list.add("x".to_string());
    list.add("y".to_string());
    let items = list.list();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].text, "x");
    assert!(!items[0].done);
    assert_eq!(items[1].text, "y");
    assert!(!items[1].done);
}

#[test]
fn todo_complete_by_index() {
    // R2: completing a valid index marks only that item done.
    let mut list = TodoList::new();
    list.add("a".to_string());
    list.add("b".to_string());
    assert!(list.complete_by_index(1).is_ok());
    let items = list.list();
    assert!(!items[0].done);
    assert!(items[1].done);
}

#[test]
fn todo_complete_out_of_range() {
    // R2: out-of-range index is a domain error (no panic), list unchanged.
    let mut list = TodoList::new();
    list.add("a".to_string());
    let err = list.complete_by_index(5).unwrap_err();
    assert!(err.to_string().contains("out of range"));
    let items = list.list();
    assert_eq!(items.len(), 1);
    assert!(!items[0].done);
}

#[test]
fn todo_complete_idempotent() {
    // Spec edge case: completing an already-done item twice is idempotent,
    // no error, list unchanged.
    let mut list = TodoList::new();
    list.add("a".to_string());
    assert!(list.complete_by_index(0).is_ok());
    assert!(list.complete_by_index(0).is_ok());
    let items = list.list();
    assert_eq!(items.len(), 1);
    assert!(items[0].done);
}

#[test]
fn todo_complete_by_text() {
    // R3: completing exact text marks it done; absent text is an error.
    let mut list = TodoList::new();
    list.add("buy milk".to_string());
    list.add("call bob".to_string());
    assert!(list.complete_by_text("buy milk").is_ok());
    let items = list.list();
    assert!(items[0].done);
    assert!(!items[1].done);
    let err = list.complete_by_text("no such task").unwrap_err();
    assert!(err.to_string().contains("not found"));
    let items = list.list();
    assert_eq!(items.len(), 2);
    assert!(items[0].done);
    assert!(!items[1].done);
}

#[test]
fn todo_render_format() {
    // R4: deterministic render, one line per item, `- [x]` / `- [ ]`.
    let mut list = TodoList::new();
    list.add("first".to_string());
    list.add("second".to_string());
    assert!(list.complete_by_index(0).is_ok());
    let out = list.render();
    assert_eq!(out, "- [x] first\n- [ ] second");
}

#[test]
fn todo_shared_state_via_arc_mutex() {
    // R5: two clones of the same Arc<Mutex<TodoList>> observe the same list.
    let list = Arc::new(Mutex::new(TodoList::new()));
    let clone = Arc::clone(&list);
    list.lock().unwrap().add("shared".to_string());
    let guard = clone.lock().unwrap();
    let items = guard.list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "shared");
}
