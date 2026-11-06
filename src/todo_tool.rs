//! Todo tool (MT-2): mutates the shared `Arc<Mutex<TodoList>>` and returns
//! `render()` to the model. Stateless w.r.t. data — state lives in the
//! shared handle, same pattern as `tool_registry`.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use crate::todo::{TodoList, TodoListHandle};
use nxm_tui_tools::{Tool, ToolKind, ToolResult};

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
const TARGET: &str = "nexum::todo::tool";

/// `todo` tool: add / complete / list items on the shared todo list.
///
/// The tool carries a shared `TodoListHandle` (`Arc<Mutex<TodoList>>`),
/// so `App` (UI, MT-3) and this tool observe the same list. Stateless
/// w.r.t. data — all state is in the handle.
///
/// ```
/// use nxm_tui::todo::{todo_list_handle, TodoListHandle};
/// use nxm_tui::todo_tool::Todo;
/// use nxm_tui_tools::Tool;
///
/// let handle: TodoListHandle = todo_list_handle();
/// let tool = Todo { todos: handle.clone() };
/// assert_eq!(tool.name(), "todo");
/// let res = tool.run(&serde_json::json!({ "action": "add", "text": "buy milk" })).unwrap();
/// assert!(res.ok);
/// assert!(res.content.contains("- [ ] buy milk"));
/// ```
pub struct Todo {
    pub todos: TodoListHandle,
}

impl Tool for Todo {
    /// Name of the tool as called by the model (`"todo"`).
    ///
    /// ```
    /// use nxm_tui::todo::todo_list_handle;
    /// use nxm_tui::todo_tool::Todo;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = Todo { todos: todo_list_handle() };
    /// assert_eq!(tool.name(), "todo");
    /// ```
    fn name(&self) -> &str {
        "todo"
    }

    /// Human-readable description of the tool.
    ///
    /// ```
    /// use nxm_tui::todo::todo_list_handle;
    /// use nxm_tui::todo_tool::Todo;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = Todo { todos: todo_list_handle() };
    /// assert!(!tool.description().is_empty());
    /// ```
    fn description(&self) -> &str {
        "Manage the shared todo list: add, complete, or list items."
    }

    /// JSON schema: `action` (required, string) + optional `text` (string)
    /// and `index` (integer).
    ///
    /// ```
    /// use nxm_tui::todo::todo_list_handle;
    /// use nxm_tui::todo_tool::Todo;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = Todo { todos: todo_list_handle() };
    /// let schema = tool.schema();
    /// assert_eq!(schema["required"], serde_json::json!(["action"]));
    /// assert!(schema["properties"]["action"].is_object());
    /// assert!(schema["properties"]["text"].is_object());
    /// assert!(schema["properties"]["index"].is_object());
    /// ```
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action to perform: add, complete, or list."
                },
                "text": {
                    "type": "string",
                    "description": "Todo text to add, or to complete by exact text match."
                },
                "index": {
                    "type": "integer",
                    "description": "Zero-based index of the todo to complete."
                }
            },
            "required": ["action"]
        })
    }

    /// Execute one todo action on the shared list and return `render()`.
    /// Never panics on a poisoned mutex — returns `ToolResult::err` instead.
    ///
    /// ```
    /// use nxm_tui::todo::todo_list_handle;
    /// use nxm_tui::todo_tool::Todo;
    /// use nxm_tui_tools::Tool;
    ///
    /// let handle = todo_list_handle();
    /// let tool = Todo { todos: handle.clone() };
    /// let res = match tool.run(&serde_json::json!({ "action": "add", "text": "buy milk" })) {
    ///     Ok(r) => r,
    ///     Err(e) => panic!("add failed: {e}"),
    /// };
    /// assert!(res.ok);
    /// assert!(res.content.contains("- [ ] buy milk"));
    /// let res = match tool.run(&serde_json::json!({ "action": "list" })) {
    ///     Ok(r) => r,
    ///     Err(e) => panic!("list failed: {e}"),
    /// };
    /// assert!(res.ok);
    /// assert_eq!(res.content, "- [ ] buy milk");
    /// ```
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let action = match args.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return Ok(ToolResult::err("missing `action` argument".to_string())),
        };

        info!(
            target: TARGET,
            action = %action,
            "todo tool run"
        );

        let result = match action {
            "add" => self.do_add(args),
            "complete" => self.do_complete(args),
            "list" => self.do_list(),
            other => Ok(ToolResult::err(format!(
                "unknown action `{other}` (expected add, complete, or list)"
            ))),
        };

        match &result {
            Ok(r) if r.ok => info!(target: TARGET, action = %action, "todo tool done"),
            Ok(r) => {
                info!(target: TARGET, action = %action, error = %r.content, "todo tool failed")
            }
            Err(e) => info!(target: TARGET, action = %action, error = %e, "todo tool error"),
        }
        result
    }

    /// Task kind of the tool (`ToolKind::Other`; `todo` is not Core/Web/Edit/Search/Read).
    fn kind(&self) -> nxm_tui_tools::ToolKind {
        ToolKind::Other
    }

    /// Task keywords that pull the tool into automatic per-task selection.
    fn keywords(&self) -> &[&str] {
        &["todo", "task", "plan", "checklist", "lista", "piano"]
    }
}

impl Todo {
    /// Lock the shared todo list, converting a poisoned mutex into a
    /// `ToolResult::err` (never a panic).
    fn lock_or_err(&self) -> Result<std::sync::MutexGuard<'_, TodoList>> {
        self.todos
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))
    }

    fn do_add(&self, args: &Value) -> Result<ToolResult> {
        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => {
                return Ok(ToolResult::err(
                    "missing `text` argument for add".to_string(),
                ))
            }
        };
        let mut list = match self.lock_or_err() {
            Ok(g) => g,
            Err(e) => return Ok(ToolResult::err(format!("mutex poisoned: {e}"))),
        };
        list.add(text.to_string());
        let rendered = list.render();
        Ok(ToolResult::ok(rendered))
    }

    fn do_complete(&self, args: &Value) -> Result<ToolResult> {
        let mut list = match self.lock_or_err() {
            Ok(g) => g,
            Err(e) => return Ok(ToolResult::err(format!("mutex poisoned: {e}"))),
        };
        if let Some(index) = args.get("index").and_then(|v| v.as_u64()) {
            if let Err(e) = list.complete_by_index(index as usize) {
                return Ok(ToolResult::err(e.to_string()));
            }
        } else if let Some(text) = args.get("text").and_then(|v| v.as_str()) {
            if let Err(e) = list.complete_by_text(text) {
                return Ok(ToolResult::err(e.to_string()));
            }
        } else {
            return Ok(ToolResult::err(
                "missing `index` or `text` argument for complete".to_string(),
            ));
        }
        let rendered = list.render();
        Ok(ToolResult::ok(rendered))
    }

    fn do_list(&self) -> Result<ToolResult> {
        let list = match self.lock_or_err() {
            Ok(g) => g,
            Err(e) => return Ok(ToolResult::err(format!("mutex poisoned: {e}"))),
        };
        let rendered = list.render();
        Ok(ToolResult::ok(rendered))
    }
}
