//! Circuit breaker on loop stagnation (loopeng-01-02).
//!
//! `Agent::run` re-queries the model until it stops calling tools. If the
//! model repeats the **same** tool call (name + args) for N consecutive
//! iterations without progress, the loop is stuck and would burn the whole
//! iteration budget. [`StallDetector`] flags that condition: it is pure —
//! zero I/O, no clock, no network — so the stall policy is testable in
//! isolation. What counts is consecutive identical iterations, not tool
//! calls or tokens.

/// Default consecutive-repeat threshold before the breaker trips.
pub const DEFAULT_STALL_THRESHOLD: u32 = 3;

/// Outcome of observing one iteration's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stall {
    /// Signature changed (or first observation): the loop is making
    /// progress. `repeats` is the current run length of the repeated
    /// signature (1 after a change).
    ///
    /// ```
    /// use nxm_tui::circuit_breaker::{Stall, StallDetector};
    /// let mut d = StallDetector::new(3);
    /// assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    /// ```
    Ok { repeats: u32 },
    /// The same signature has been observed `repeats` times in a row,
    /// reaching `threshold`: the loop is stalled.
    Tripped { repeats: u32, signature: String },
}

/// Detects stagnation: the same tool-call signature repeated N consecutive
/// iterations.
///
/// A signature change resets the counter to 1 (progress was made). When the
/// run length of identical signatures reaches `threshold`, [`Stall::Tripped`]
/// is returned and keeps being reported on further identical observations.
///
/// Zero I/O, zero clock, zero network.
#[derive(Debug)]
pub struct StallDetector {
    threshold: u32,
    repeats: u32,
    previous: Option<String>,
}

impl StallDetector {
    /// New detector tripping after `threshold` consecutive identical
    /// signatures.
    ///
    /// ```
    /// use nxm_tui::circuit_breaker::{Stall, StallDetector};
    /// let mut d = StallDetector::new(3);
    /// assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    /// assert_eq!(d.observe("A"), Stall::Ok { repeats: 2 });
    /// assert_eq!(d.observe("A"), Stall::Tripped { repeats: 3, signature: "A".into() });
    /// ```
    pub fn new(threshold: u32) -> Self {
        tracing::debug!(
            target: "nexum::circuit_breaker::detector",
            threshold,
            "stall detector created"
        );
        Self {
            threshold,
            repeats: 0,
            previous: None,
        }
    }

    /// Record the signature of one iteration.
    ///
    /// - Same signature as the previous iteration → `repeats + 1`.
    /// - Different signature (or first observation) → reset to 1.
    /// - When `repeats >= threshold` → [`Stall::Tripped`].
    ///
    /// With `threshold = 1` the first observation already trips (every cycle
    /// is a run of one).
    ///
    /// ```
    /// use nxm_tui::circuit_breaker::{Stall, StallDetector};
    /// let mut d = StallDetector::new(2);
    /// assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    /// assert_eq!(d.observe("B"), Stall::Ok { repeats: 1 }); // progress resets
    /// assert_eq!(d.observe("A"), Stall::Ok { repeats: 1 });
    /// assert_eq!(d.observe("A"), Stall::Tripped { repeats: 2, signature: "A".into() });
    /// ```
    pub fn observe(&mut self, signature: &str) -> Stall {
        if self.previous.as_deref() == Some(signature) {
            self.repeats += 1;
        } else {
            self.repeats = 1;
            self.previous = Some(signature.to_string());
        }
        tracing::debug!(
            target: "nexum::circuit_breaker::observe",
            repeats = self.repeats,
            threshold = self.threshold,
            signature = %signature,
            "iteration signature observed"
        );
        if self.repeats >= self.threshold {
            return Stall::Tripped {
                repeats: self.repeats,
                signature: signature.to_string(),
            };
        }
        Stall::Ok {
            repeats: self.repeats,
        }
    }
}

/// Deterministic, order-independent signature of one iteration's tool calls.
///
/// Each call contributes `name` + `args`; entries are sorted before
/// concatenation so two cycles with the same calls in a different order
/// collide (same signature), while any change to a name or an argument
/// produces a different signature. A NUL separator prevents ambiguous
/// concatenations (e.g. `("a","b")` vs `("ab","")`).
///
/// ```
/// use nxm_tui::circuit_breaker::signature;
/// let a = signature(&[
///     ("read_file".to_string(), "{\"path\":\"a\"}".to_string()),
///     ("list_resources".to_string(), "{}".to_string()),
/// ]);
/// let b = signature(&[
///     ("list_resources".to_string(), "{}".to_string()),
///     ("read_file".to_string(), "{\"path\":\"a\"}".to_string()),
/// ]);
/// assert_eq!(a, b); // same calls, different order → same signature
/// let c = signature(&[("read_file".to_string(), "{\"path\":\"b\"}".to_string())]);
/// assert_ne!(a, c); // different arg → different signature
/// ```
pub fn signature(calls: &[(String, String)]) -> String {
    let mut entries: Vec<(String, String)> = calls.to_vec();
    entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let joined: String = entries
        .into_iter()
        .map(|(name, args)| format!("{name}\u{0000}{args}"))
        .collect::<Vec<_>>()
        .join("\u{0001}");
    tracing::debug!(
        target: "nexum::circuit_breaker::signature",
        call_count = calls.len(),
        signature_len = joined.len(),
        "tool-call signature computed"
    );
    joined
}
