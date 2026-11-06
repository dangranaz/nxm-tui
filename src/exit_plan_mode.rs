//! Exit-plan-mode tool (MT-2).

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use crate::plan_mode::PlanModeHandle;
use nxm_tui_tools::ToolKind;
use nxm_tui_tools::{Tool, ToolResult};

const TARGET: &str = "nexum::plan_mode::exit";

/// Exit-plan-mode tool: exits plan mode.
///
/// Returns to normal operation where write tools (Edit, Web) are available
/// again according to the normal per-task selection.
///
/// The tool carries a shared `PlanModeHandle` (`Arc<AtomicBool>`), so `App`
/// and this tool observe the same plan mode state. Stateless w.r.t. data —
/// all state is in the handle.
///
/// ```
/// use nxm_tui::plan_mode::plan_mode_handle;
/// use nxm_tui::exit_plan_mode::ExitPlanMode;
/// use nxm_tui_tools::Tool;
///
/// let handle = plan_mode_handle();
/// let tool = ExitPlanMode { goal: handle.clone() };
/// assert_eq!(tool.name(), "exit_plan_mode");
/// let res = tool.run(&serde_json::json!({})).unwrap();
/// assert!(res.ok);
/// assert_eq!(handle.load(std::sync::atomic::Ordering::Relaxed), false);
//
pub struct ExitPlanMode {
    pub goal: PlanModeHandle,
}

impl Tool for ExitPlanMode {
    /// Name of the tool as called by the model (`"exit_plan_mode"`).
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    /// Human-readable description of the tool.
    fn description(&self) -> &str {
        "Exit plan mode: return to normal operation with write tools available."
    }

    /// JSON schema: no arguments required.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    /// Set the plan mode flag to `false` and return a confirmation.
    fn run(&self, _args: &Value) -> Result<ToolResult> {
        info!(target: TARGET, "exit_plan_mode run");
        self.goal.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(ToolResult::ok("plan mode deactivated".to_string()))
    }

    /// Task kind of the tool (`ToolKind::Other`).
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    /// Task keywords that pull the tool into automatic per-task selection.
    fn keywords(&self) -> &[&str] {
        &["plan", "exit", "normal"]
    }
}
