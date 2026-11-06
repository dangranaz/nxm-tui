//! Pure iteration-budget logic for the ReAct loop (loopeng-01).
//!
//! `Agent::run` is an unbounded `loop {}`: it exits only when the model stops
//! calling tools. [`BudgetTracker`] gives it a hard, configurable cap on the
//! number of model turns (iterations): when the budget is spent, the loop
//! must hard-stop and report it to the UI via `ToolPart::LoopHalted`. The
//! tracker is pure — zero I/O, no clock, no network — so the cap policy is
//! testable in isolation. What counts is loop iterations, not tool calls or
//! tokens (token budget is a later phase, out of scope here).

/// Default hard cap on `Agent::run` iterations (user-chosen: 15).
pub const DEFAULT_MAX_STEPS: u32 = 15;

/// Outcome of one loop iteration checked against the budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Budget {
    /// Budget not exhausted: the loop may run another iteration.
    Continue,
    /// Budget exhausted after `steps` completed iterations: hard stop.
    Halted { steps: u32 },
}

/// Counts `Agent::run` loop iterations against a hard cap.
#[derive(Debug)]
pub struct BudgetTracker {
    max_steps: u32,
    steps: u32,
}

impl BudgetTracker {
    /// New tracker allowing at most `max_steps` iterations.
    ///
    /// ```
    /// use nxm_tui::bounded::{Budget, BudgetTracker};
    /// let mut t = BudgetTracker::new(1);
    /// assert_eq!(t.tick(), Budget::Continue);
    /// assert_eq!(t.tick(), Budget::Halted { steps: 1 });
    /// ```
    pub fn new(max_steps: u32) -> Self {
        tracing::debug!(
            target: "nexum::bounded::tracker",
            max_steps,
            "iteration budget tracker created"
        );
        Self {
            max_steps,
            steps: 0,
        }
    }

    /// Record one loop iteration. Returns [`Budget::Continue`] while the loop
    /// may keep going; once the cap is hit, every further tick keeps reporting
    /// [`Budget::Halted`] (the budget never restarts).
    ///
    /// ```
    /// use nxm_tui::bounded::{Budget, BudgetTracker};
    /// let mut t = BudgetTracker::new(0);
    /// assert_eq!(t.tick(), Budget::Halted { steps: 0 });
    /// ```
    pub fn tick(&mut self) -> Budget {
        if self.steps >= self.max_steps {
            tracing::info!(
                target: "nexum::bounded::tick",
                max_steps = self.max_steps,
                steps = self.steps,
                "iteration budget exhausted, halting loop"
            );
            return Budget::Halted { steps: self.steps };
        }
        self.steps += 1;
        tracing::trace!(
            target: "nexum::bounded::tick",
            max_steps = self.max_steps,
            steps = self.steps,
            "iteration recorded"
        );
        Budget::Continue
    }

    /// Number of iterations recorded so far (the current step index).
    ///
    /// Exposed so the stagnation breaker (`circuit_breaker`) can report the
    /// current iteration index in `ToolPart::LoopHalted { steps }` without
    /// reaching into `BudgetTracker`'s private field — the two modules stay
    /// separate, per loopeng-01-02.
    ///
    /// ```
    /// use nxm_tui::bounded::BudgetTracker;
    /// let mut t = BudgetTracker::new(3);
    /// t.tick();
    /// assert_eq!(t.steps(), 1);
    /// ```
    pub fn steps(&self) -> u32 {
        tracing::debug!(
            target: "nexum::bounded::steps",
            steps = self.steps,
            "iteration step index read"
        );
        self.steps
    }
}
