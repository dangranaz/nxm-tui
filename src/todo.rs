//! Shared todo-list state: pure types + mutation logic, no tool, no UI.
//!
//! The state lives as [`Arc<Mutex<TodoList>>`] so [`crate::app::App`] (UI,
//! MT-3) and [`crate::agent::Agent`] (tool, MT-2) can observe the same list.
//! Tools remain stateless — the todo state is shared, not embedded in a tool.

use std::fmt;
use std::sync::{Arc, Mutex};

use tracing::debug;

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
const TARGET: &str = "nexum::todo::state";

/// One todo item: text + done flag.
///
/// ```
/// use nxm_tui::todo::{TodoItem, TodoList};
///
/// let mut list = TodoList::new();
/// list.add("buy milk".to_string());
/// let item = TodoItem { text: "buy milk".to_string(), done: false };
/// assert_eq!(item.text, "buy milk");
/// assert!(!item.done);
/// ```
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

/// Todo list: a wrapper over `Vec<TodoItem>` with deterministic, registration
/// order. The manual/shared state is `Arc<Mutex<TodoList>>`.
///
/// ```
/// use nxm_tui::todo::TodoList;
///
/// let mut list = TodoList::new();
/// list.add("buy milk".to_string());
/// assert_eq!(list.list().len(), 1);
/// assert!(!list.list()[0].done);
/// ```
#[derive(Debug, Clone, Default)]
pub struct TodoList(Vec<TodoItem>);

/// Domain errors for todo mutations — typed variants, no `thiserror`
/// dependency (not in the workspace; `std::error::Error` implemented by hand).
///
/// ```
/// use nxm_tui::todo::TodoError;
///
/// let e = TodoError::NotFound { text: "x".to_string() };
/// assert_eq!(e.to_string(), "todo not found: x");
/// ```
#[derive(Debug, Clone)]
pub enum TodoError {
    /// Requested index is beyond the current list length.
    ///
    /// ```
    /// use nxm_tui::todo::TodoError;
    ///
    /// let e = TodoError::IndexOutOfRange { index: 5, len: 2 };
    /// assert_eq!(e.to_string(), "todo index 5 out of range (list has 2 items)");
    /// ```
    IndexOutOfRange { index: usize, len: usize },
    /// Requested text does not match any item.
    ///
    /// ```
    /// use nxm_tui::todo::TodoError;
    ///
    /// let e = TodoError::NotFound { text: "x".to_string() };
    /// assert_eq!(e.to_string(), "todo not found: x");
    /// ```
    NotFound { text: String },
}

impl fmt::Display for TodoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TodoError::IndexOutOfRange { index, len } => {
                write!(f, "todo index {index} out of range (list has {len} items)")
            }
            TodoError::NotFound { text } => {
                write!(f, "todo not found: {text}")
            }
        }
    }
}

impl std::error::Error for TodoError {}

impl TodoList {
    /// Create an empty todo list.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let list = TodoList::new();
    /// assert!(list.list().is_empty());
    /// ```
    pub fn new() -> Self {
        debug!(target: TARGET, "todo new");
        Self(Vec::new())
    }

    /// Append a new (not done) item to the end of the list.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let mut list = TodoList::new();
    /// list.add("buy milk".to_string());
    /// assert_eq!(list.list().len(), 1);
    /// assert!(!list.list()[0].done);
    /// ```
    pub fn add(&mut self, text: String) {
        debug!(target: TARGET, "todo add text={text:?}");
        self.0.push(TodoItem { text, done: false });
    }

    /// Mark the item at `index` as done. Returns a domain error (no panic)
    /// when the index is out of range; the list is left unchanged.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let mut list = TodoList::new();
    /// list.add("a".to_string());
    /// list.add("b".to_string());
    /// assert!(list.complete_by_index(1).is_ok());
    /// assert!(list.list()[1].done);
    /// assert!(!list.list()[0].done);
    /// ```
    pub fn complete_by_index(&mut self, index: usize) -> Result<(), TodoError> {
        if index >= self.0.len() {
            debug!(
                target: TARGET,
                "todo complete_by_index index={index} out of range len={}",
                self.0.len()
            );
            return Err(TodoError::IndexOutOfRange {
                index,
                len: self.0.len(),
            });
        }
        self.0[index].done = true;
        debug!(target: TARGET, "todo complete_by_index index={index}");
        Ok(())
    }

    /// Mark the first item whose `text` equals `text` as done. Returns a
    /// domain error (no panic) when no item matches; the list is unchanged.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let mut list = TodoList::new();
    /// list.add("buy milk".to_string());
    /// assert!(list.complete_by_text("buy milk").is_ok());
    /// assert!(list.list()[0].done);
    /// ```
    pub fn complete_by_text(&mut self, text: &str) -> Result<(), TodoError> {
        match self.0.iter().position(|i| i.text == text) {
            Some(index) => {
                self.0[index].done = true;
                debug!(target: TARGET, "todo complete_by_text text={text:?}");
                Ok(())
            }
            None => {
                debug!(target: TARGET, "todo complete_by_text text={text:?} not found");
                Err(TodoError::NotFound {
                    text: text.to_string(),
                })
            }
        }
    }

    /// Borrow the items in registration order.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let mut list = TodoList::new();
    /// list.add("x".to_string());
    /// assert_eq!(list.list().len(), 1);
    /// ```
    pub fn list(&self) -> &[TodoItem] {
        debug!(target: TARGET, "todo list len={}", self.0.len());
        &self.0
    }

    /// Deterministic textual rendering: one `- [ ] text` / `- [x] text` line
    /// per item, registration order, joined with `\n`. Empty list → empty
    /// string.
    ///
    /// ```
    /// use nxm_tui::todo::TodoList;
    ///
    /// let mut list = TodoList::new();
    /// list.add("first".to_string());
    /// list.add("second".to_string());
    /// assert!(list.complete_by_index(0).is_ok());
    /// assert_eq!(list.render(), "- [x] first\n- [ ] second");
    /// ```
    pub fn render(&self) -> String {
        debug!(target: TARGET, "todo render");
        self.0
            .iter()
            .map(|i| {
                if i.done {
                    format!("- [x] {}", i.text)
                } else {
                    format!("- [ ] {}", i.text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Shared todo-list handle: cloneable, `Send + Sync`, shared between `App`
/// (UI, MT-3) and `Agent` (tool, MT-2) — same pattern as `tool_registry`.
///
/// ```
/// use nxm_tui::todo::{TodoListHandle, todo_list_handle};
///
/// let handle: TodoListHandle = todo_list_handle();
/// handle.lock().expect("lock poisoned").add("buy milk".to_string());
/// assert_eq!(handle.lock().expect("lock poisoned").list().len(), 1);
/// ```
pub type TodoListHandle = Arc<Mutex<TodoList>>;

/// Create a new shared todo-list handle.
///
/// ```
/// use nxm_tui::todo::todo_list_handle;
///
/// let handle = todo_list_handle();
/// handle.lock().unwrap().add("a".to_string());
/// assert_eq!(handle.lock().unwrap().list().len(), 1);
/// ```
pub fn todo_list_handle() -> TodoListHandle {
    debug!(target: TARGET, "todo_list_handle");
    Arc::new(Mutex::new(TodoList::new()))
}
