//! Shared goal state: a single optional string, pure types + handle factory.
//!
//! The state lives as [`Arc<Mutex<Option<String>>>`] so [`crate::app::App`]
//! (mode bar) and the [`crate::update_goal::UpdateGoal`] tool can observe the
//! same current goal. Stateless w.r.t. data — state is in the shared handle,
//! not embedded in a tool.

use std::sync::{Arc, Mutex};

use tracing::debug;

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
pub const TARGET: &str = "nexum::goal::state";

/// Shared goal handle: cloneable, `Send + Sync`, shared between `App` and the
/// `update_goal` tool. `None` means no goal has been set.
pub type GoalHandle = Arc<Mutex<Option<String>>>;

/// Create a new shared goal handle with no goal set (`None`).
///
/// ```
/// use nxm_tui::goal::goal_handle;
///
/// let handle = goal_handle();
/// assert_eq!(*handle.lock().unwrap(), None);
/// ```
pub fn goal_handle() -> GoalHandle {
    debug!(target: TARGET, "goal handle created");
    Arc::new(Mutex::new(None))
}
