//! Update-goal tool (MT-1): mutates the shared `Arc<Mutex<Option<String>>>` goal
//! state and returns a confirmation to the model. Stateless w.r.t. data —
//! state lives in the shared handle, same pattern as `todo`.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use crate::goal::GoalHandle;
use nxm_tui_tools::{Tool, ToolKind, ToolResult};

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
const RUN_TARGET: &str = "nexum::update_goal::run";

/// `update_goal` tool: sets the agent's current goal (a single string).
///
/// The tool carries a shared `GoalHandle` (`Arc<Mutex<Option<String>>>`),
/// so `App` (mode bar) and this tool observe the same goal. Stateless
/// w.r.t. data — all state is in the handle.
///
/// Setting an empty goal is rejected with `ToolResult::err` and leaves the
/// state unchanged. Mutex lock poisoning is handled without panic.
///
/// ```
/// use nxm_tui::goal::goal_handle;
/// use nxm_tui::update_goal::UpdateGoal;
/// use nxm_tui_tools::Tool;
///
/// let handle = goal_handle();
/// let tool = UpdateGoal { goal: handle.clone() };
/// assert_eq!(tool.name(), "update_goal");
/// let res = tool.run(&serde_json::json!({ "goal": "ship v1" })).unwrap();
/// assert!(res.ok);
/// assert_eq!(*handle.lock().unwrap(), Some("ship v1".to_string()));
/// ```
pub struct UpdateGoal {
    pub goal: GoalHandle,
}

impl Tool for UpdateGoal {
    /// Name of the tool as called by the model (`"update_goal"`).
    ///
    /// ```
    /// use nxm_tui::goal::goal_handle;
    /// use nxm_tui::update_goal::UpdateGoal;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = UpdateGoal { goal: goal_handle() };
    /// assert_eq!(tool.name(), "update_goal");
    /// ```
    fn name(&self) -> &str {
        "update_goal"
    }

    /// Human-readable description of the tool.
    ///
    /// ```
    /// use nxm_tui::goal::goal_handle;
    /// use nxm_tui::update_goal::UpdateGoal;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = UpdateGoal { goal: goal_handle() };
    /// assert!(!tool.description().is_empty());
    /// ```
    fn description(&self) -> &str {
        "Set the agent's current goal (a single string shown in the mode bar)."
    }

    /// JSON schema: `goal` (required, string).
    ///
    /// ```
    /// use nxm_tui::goal::goal_handle;
    /// use nxm_tui::update_goal::UpdateGoal;
    /// use nxm_tui_tools::Tool;
    ///
    /// let tool = UpdateGoal { goal: goal_handle() };
    /// let schema = tool.schema();
    /// assert_eq!(schema["required"], serde_json::json!(["goal"]));
    /// assert!(schema["properties"]["goal"].is_object());
    /// assert_eq!(schema["properties"]["goal"]["type"], "string");
    /// ```
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "goal": {
                    "type": "string",
                    "description": "Current goal for the agent (shown in the mode bar)."
                }
            },
            "required": ["goal"]
        })
    }

    /// Set the current goal and return a confirmation. Never panics on a
    /// poisoned mutex — returns `ToolResult::err` instead.
    ///
    /// ```
    /// use nxm_tui::goal::goal_handle;
    /// use nxm_tui::update_goal::UpdateGoal;
    /// use nxm_tui_tools::Tool;
    ///
    /// let handle = goal_handle();
    /// let tool = UpdateGoal { goal: handle.clone() };
    /// let res = tool.run(&serde_json::json!({ "goal": "ship v1" })).unwrap();
    /// assert!(res.ok);
    /// assert_eq!(*handle.lock().unwrap(), Some("ship v1".to_string()));
    /// ```
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let text = match args.get("goal").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t.to_string(),
            Some(_) => return Ok(ToolResult::err("goal must not be empty".to_string())),
            None => return Ok(ToolResult::err("missing `goal` argument".to_string())),
        };

        info!(
            target: RUN_TARGET,
            goal_len = text.len(),
            "update_goal run"
        );

        let mut guard = match self.goal.lock() {
            Ok(g) => g,
            Err(e) => return Ok(ToolResult::err(format!("mutex poisoned: {e}"))),
        };
        *guard = Some(text.clone());
        Ok(ToolResult::ok(format!("goal set: {text}")))
    }

    /// Task kind of the tool (`ToolKind::Other`; `update_goal` is not Core/Web/Edit/Search/Read).
    fn kind(&self) -> nxm_tui_tools::ToolKind {
        ToolKind::Other
    }

    /// Task keywords that pull the tool into automatic per-task selection.
    fn keywords(&self) -> &[&str] {
        &["goal", "objective", "obiettivo", "target", "aim", "plan"]
    }
}
