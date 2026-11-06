//! Plan mode shared state: a simple atomic flag.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Plan mode handle: cloneable, `Send + Sync`, shared between `App` and the
/// `enter_plan_mode`/`exit_plan_mode` tools. `false` = plan mode off (normal
/// operation), `true` = plan mode on (write-abstaining analysis mode).
pub type PlanModeHandle = Arc<AtomicBool>;

/// Create a new plan mode handle with plan mode off (`false`).
///
/// ```
/// use nxm_tui::plan_mode::plan_mode_handle;
///
/// let handle = plan_mode_handle();
/// assert_eq!(handle.load(std::sync::atomic::Ordering::Relaxed), false);
/// ```
pub fn plan_mode_handle() -> PlanModeHandle {
    Arc::new(AtomicBool::new(false))
}
