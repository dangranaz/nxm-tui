//! Enter-plan-mode tool (MT-2).

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use crate::plan_mode::PlanModeHandle;
use nxm_tui_tools::ToolKind;
use nxm_tui_tools::{Tool, ToolResult};

const TARGET: &str = "nexum::plan_mode::enter";

/// Enter-plan-mode tool: enters plan mode.
///
/// In plan mode the Agent abstains from write tools (Edit, Web) and can only
/// analyze and produce a task list via the existing `todo` tool. No overlay/tasto.
///
/// The tool carries a shared `PlanModeHandle` (`Arc<AtomicBool>`), so `App`
/// and this tool observe the same plan mode state. Stateless w.r.t. data —
/// all state is in the handle.
///
/// ```
/// use nxm_tui::plan_mode::plan_mode_handle;
/// use nxm_tui::enter_plan_mode::EnterPlanMode;
/// use nxm_tui_tools::Tool;
///
/// let handle = plan_mode_handle();
/// let tool = EnterPlanMode { goal: handle.clone() };
/// assert_eq!(tool.name(), "enter_plan_mode");
/// let res = tool.run(&serde_json::json!({})).unwrap();
/// assert!(res.ok);
/// assert_eq!(handle.load(std::sync::atomic::Ordering::Relaxed), true);
//
pub struct EnterPlanMode {
    pub goal: PlanModeHandle,
}

impl Tool for EnterPlanMode {
    /// Name of the tool as called by the model (`"enter_plan_mode"`).
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    /// Human-readable description of the tool.
    fn description(&self) -> &str {
        "Enter plan mode: Agent abstains from write tools (Edit, Web) and can only analyze + produce task list via `todo`."
    }

    /// JSON schema: no arguments required.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    /// Set the plan mode flag to `true` and return a confirmation.
    fn run(&self, _args: &Value) -> Result<ToolResult> {
        info!(target: TARGET, "enter_plan_mode run");
        self.goal.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(ToolResult::ok("plan mode activated".to_string()))
    }

    /// Task kind of the tool (`ToolKind::Other`).
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    /// Task keywords that pull the tool into automatic per-task selection.
    fn keywords(&self) -> &[&str] {
        &["plan", "analyze", "mode"]
    }
}
