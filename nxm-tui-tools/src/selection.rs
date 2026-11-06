//! Automatic per-task selection: `Registry::select(task)` and
//! `Registry::manifest_for(task)`.
//!
//! Pure logic, no runtime behavior change yet (MT-2): `agent.rs` and the
//! overlay are touched only in MT-3. `manifest()` / `manifest_selected()`
//! are not modified here.

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::Value;
use tracing::debug;

use crate::{manifest_entry, Tool, ToolKind, TARGET};

/// Automatic per-task selection of tools for `task`.
///
/// Always includes every [`ToolKind::Core`] tool, plus any tool for which at
/// least one of [`Tool::keywords`] is a case-insensitive substring of `task`
/// (`task.to_lowercase().contains(&kw.to_lowercase())`).
///
/// Fail-safe: if no non-Core tool matches, returns **only** the Core tools
/// (not all of them).
///
/// Output order follows tool registration order (deterministic).
pub fn select(registry: &crate::Registry, task: &str) -> Vec<Arc<dyn Tool>> {
    let task_lower = task.to_lowercase();
    let mut selected: Vec<Arc<dyn Tool>> = Vec::new();
    let mut non_core_matched = false;

    for tool in &registry.tools {
        if tool.kind() == ToolKind::Core {
            selected.push(tool.clone());
            continue;
        }
        let matched = tool
            .keywords()
            .iter()
            .any(|kw| task_lower.contains(&kw.to_lowercase()));
        if matched {
            non_core_matched = true;
            selected.push(tool.clone());
        }
    }

    let fail_safe = !non_core_matched;
    if fail_safe {
        selected.retain(|tool| tool.kind() == ToolKind::Core);
    }

    let names: Vec<String> = selected.iter().map(|t| t.name().to_string()).collect();
    debug!(
        target: TARGET,
        "select(task={task:?}) -> {} tool(s) [{}]{}",
        selected.len(),
        names.join(", "),
        if fail_safe {
            " (fail-safe: Core only)"
        } else {
            ""
        }
    );
    selected
}

/// OpenAI manifest for `task`: union of [`select`] and the manually enabled
/// tools, by name, without duplicates, in registration order.
pub fn manifest_for(registry: &crate::Registry, task: &str) -> Vec<Value> {
    let auto: HashSet<String> = select(registry, task)
        .iter()
        .map(|t| t.name().to_string())
        .collect();

    let out: Vec<Value> = registry
        .tools
        .iter()
        .filter(|t| auto.contains(t.name()) || registry.is_enabled(t.name()))
        .map(|t| manifest_entry(t.as_ref()))
        .collect();

    debug!(target: TARGET, "manifest_for(task={task:?}) -> {} tool(s)", out.len());
    out
}
