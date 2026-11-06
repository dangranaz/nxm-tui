//! `nxm-tui-tools` — pure-Rust actuator set for the Nexum agentic TUI.
//!
//! A single crate holding the tools the OpenAI agent calls (`web_search`,
//! `read_file`, `list_resources`). `nxm-tui`'s `Agent` (src/agent.rs) depends on
//! this crate's [`Tool`] trait + [`Registry`] (P2-proper wiring). v0.1 scaffold:
//! blocking I/O is intentional — P2-proper makes `Tool::run` async.
//!
//! Run: `cargo test --workspace` (lib + integration tests under `tests/`).

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

mod builtins;
mod grep;
mod search_replace;
mod selection;
mod ssrf;
mod taxonomy;
pub use builtins::{ListResources, ReadFile, WebFetch, WebSearch};
pub use grep::Grep;
pub use search_replace::SearchReplace;
pub use ssrf::{
    fetch_with_ssrf_guard, is_allowed_scheme, is_blocked_addr, validate_host, validate_url,
};
pub use taxonomy::ToolKind;

/// Tracing target per `RULES.md` (`nexum::<module>::<feature>`).
pub const TARGET: &str = "nexum::tools::runtime";

/// Outcome of a tool run, returned to the model as a `role: "tool"` result.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the tool succeeded.
    pub ok: bool,
    /// Text content handed back to the model.
    pub content: String,
}

impl ToolResult {
    pub fn ok(content: String) -> Self {
        Self { ok: true, content }
    }
    pub fn err(msg: String) -> Self {
        Self {
            ok: false,
            content: msg,
        }
    }
}

/// A tool callable by the OpenAI agent. Implementations are stateless and
/// `Send + Sync` so the [`Registry`] can hold `Arc<dyn Tool>`.
pub trait Tool: Send + Sync {
    /// Canonical, kebab-case name the model calls (e.g. `read_file`).
    fn name(&self) -> &str;
    /// Human-readable description given to the model.
    fn description(&self) -> &str;
    /// JSON schema (`function.parameters`) for the tool's arguments.
    fn schema(&self) -> Value;
    /// Execute the tool. Synchronous in v0.1 (P2-proper: make `async`).
    fn run(&self, args: &Value) -> Result<ToolResult>;
    /// Task kind of the tool ([`ToolKind::Other`] by default). MT-2's
    /// automatic selection groups the manifest on this; `Core` tools are
    /// always included and also form the fail-safe set.
    ///
    /// ```
    /// use nxm_tui_tools::{Tool, ToolKind, WebFetch};
    ///
    /// assert_eq!(WebFetch.kind(), ToolKind::Web);
    /// ```
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }
    /// Lower-case English/Italian task keywords that pull the tool into the
    /// automatic per-task selection (MT-2). Matching is case-insensitive at
    /// the call site; empty here, so defaulted tools are only selected via
    /// manual toggle.
    ///
    /// ```
    /// use nxm_tui_tools::{Grep, Tool};
    ///
    /// assert!(Grep.keywords().contains(&"grep"));
    /// assert!(nxm_tui_tools::ReadFile.keywords().is_empty());
    /// ```
    fn keywords(&self) -> &[&str] {
        &[]
    }
}

/// Registry of available tools + OpenAI manifest builder.
///
/// Holds the T1 in-memory selection: the **manual** `enabled` set starts
/// empty (MT-3), and a tool reaches the model only via
/// [`manifest_for`](Registry::manifest_for) — the automatic per-task
/// selection ([`ToolKind::Core`] + keyword match) union the tools the user
/// explicitly enabled. Selection is orthogonal to approval gating — a
/// selected tool can still be gated in `tool_types::needs_approval`.
#[derive(Clone)]
pub struct Registry {
    tools: Vec<Arc<dyn Tool>>,
    /// Names of the tools currently sent to the model (T1 selection).
    enabled: HashSet<String>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            enabled: HashSet::new(),
        }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        info!(target: TARGET, "register tool `{name}`", name = tool.name());
        // MT-3: registration no longer auto-enables. The manual `enabled` set
        // starts empty; visibility comes from `select(task)` (Core + keyword)
        // plus what the user explicitly enables via the overlay.
        self.tools.push(Arc::new(tool));
    }

    /// Enable or disable a tool for the model-visible manifest (T1).
    ///
    /// ```
    /// use nxm_tui_tools::default_registry;
    ///
    /// let mut reg = default_registry();
    /// reg.set_enabled("web_search", false);
    /// assert!(!reg.is_enabled("web_search"));
    /// ```
    pub fn set_enabled(&mut self, name: &str, on: bool) {
        tracing::debug!(target: TARGET, "set tool `{name}` enabled={on}");
        if on {
            self.enabled.insert(name.to_string());
        } else {
            self.enabled.remove(name);
        }
    }

    /// Whether a tool is part of the model-visible selection (manual
    /// `enabled` set; MT-3 starts empty — nothing is enabled by default).
    ///
    /// ```
    /// use nxm_tui_tools::default_registry;
    ///
    /// // Default: nothing manually enabled.
    /// assert!(!default_registry().is_enabled("read_file"));
    /// ```
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }

    /// Enable every registered tool (restore the full selection).
    ///
    /// ```
    /// use nxm_tui_tools::default_registry;
    ///
    /// let mut reg = default_registry();
    /// reg.disable_all();
    /// reg.enable_all();
    /// assert_eq!(reg.manifest_selected(), reg.manifest());
    /// ```
    pub fn enable_all(&mut self) {
        tracing::debug!(target: TARGET, "enable all tools");
        self.enabled = self.tools.iter().map(|t| t.name().to_string()).collect();
    }

    /// Disable every registered tool (the model sees an empty `tools` list).
    ///
    /// ```
    /// use nxm_tui_tools::default_registry;
    ///
    /// let mut reg = default_registry();
    /// reg.disable_all();
    /// assert!(reg.manifest_selected().is_empty());
    /// ```
    pub fn disable_all(&mut self) {
        tracing::debug!(target: TARGET, "disable all tools");
        self.enabled.clear();
    }

    /// Registered tool names, in registration order (for the Tools overlay UI).
    ///
    /// ```
    /// use nxm_tui_tools::Registry;
    ///
    /// let mut reg = Registry::new();
    /// reg.register(nxm_tui_tools::ReadFile);
    /// assert_eq!(reg.tool_names(), vec!["read_file"]);
    /// ```
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// Look up a tool by name (clones the `Arc`, no cloning of the tool itself).
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    /// Execute a named tool. Returns an `Err` only if the tool is unknown or
    /// the binary throws; tool-level failures come back as `ToolResult::err`.
    pub fn call(&self, name: &str, args: &Value) -> Result<ToolResult> {
        let tool = self
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown tool `{name}`"))?;
        tool.run(args)
    }

    /// Build the OpenAI `tools` manifest sent in the chat request.
    pub fn manifest(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| manifest_entry(t.as_ref()))
            .collect()
    }

    /// Like [`manifest`](Registry::manifest) but filtered to the enabled
    /// subset — what the agent puts in the request's `tools` body (T1).
    ///
    /// ```
    /// use nxm_tui_tools::default_registry;
    ///
    /// let mut reg = default_registry();
    /// reg.set_enabled("web_search", false);
    /// let selected = reg.manifest_selected();
    /// let names: Vec<&str> = selected
    ///     .iter()
    ///     .filter_map(|v| v["function"]["name"].as_str())
    ///     .collect();
    /// assert!(!names.contains(&"web_search"));
    /// ```
    pub fn manifest_selected(&self) -> Vec<Value> {
        self.tools
            .iter()
            .filter(|t| self.is_enabled(t.name()))
            .map(|t| manifest_entry(t.as_ref()))
            .collect()
    }

    /// Automatic per-task selection of tools for `task` (MT-2, pure logic —
    /// no runtime behavior change yet; `agent.rs` is wired up in MT-3).
    ///
    /// Always includes every [`ToolKind::Core`] tool (the fail-safe set),
    /// plus any tool for which at least one of [`Tool::keywords`] is a
    /// case-insensitive substring of `task`
    /// (`task.to_lowercase().contains(&kw.to_lowercase())`). If no non-Core
    /// tool matches, returns **only** the Core tools (fail-safe, not all).
    ///
    /// Output order follows tool registration order (deterministic).
    ///
    /// ```
    /// use nxm_tui_tools::{default_registry, Registry};
    ///
    /// let mut reg = default_registry();
    /// reg.disable_all(); // isolate the automatic part until MT-3
    /// let sel = reg.select("Please GREP for foo");
    /// let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    /// assert!(names.contains(&"read_file")); // Core, always
    /// assert!(names.contains(&"grep"));      // keyword match
    /// ```
    pub fn select(&self, task: &str) -> Vec<Arc<dyn Tool>> {
        selection::select(self, task)
    }

    /// OpenAI manifest for `task`: union of
    /// [`select`](Registry::select) and the manually enabled tools, by name,
    /// without duplicates, in registration order.
    ///
    /// Until MT-3 flips the default of `enabled`, every tool is enabled, so
    /// `manifest_for` on a default registry still returns all tools — test
    /// the automatic part with `disable_all()` + targeted `set_enabled(...)`.
    ///
    /// ```
    /// use nxm_tui_tools::{default_registry, Registry};
    ///
    /// let mut reg = default_registry();
    /// reg.disable_all();
    /// reg.set_enabled("web_fetch", true);
    /// let man = reg.manifest_for("edit this file");
    /// let names: Vec<&str> = man
    ///     .iter()
    ///     .filter_map(|v| v["function"]["name"].as_str())
    ///     .collect();
    /// assert!(names.contains(&"search_replace")); // auto, keyword "edit"
    /// assert!(names.contains(&"read_file"));      // Core
    /// assert!(names.contains(&"web_fetch"));      // manual
    /// assert_eq!(
    ///     names.iter().filter(|n| **n == "search_replace").count(),
    ///     1,
    ///     "no duplicates"
    /// );
    /// ```
    pub fn manifest_for(&self, task: &str) -> Vec<Value> {
        selection::manifest_for(self, task)
    }
}

/// One OpenAI `tools` entry for `t` — shared by [`Registry::manifest`] and
/// [`Registry::manifest_selected`].
fn manifest_entry(t: &dyn Tool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": t.name(),
            "description": t.description(),
            "parameters": t.schema(),
        }
    })
}

/// Build the default MVP registry (read_file, list_resources, web_search).
pub fn default_registry() -> Registry {
    let mut reg = Registry::new();
    reg.register(ReadFile);
    reg.register(ListResources);
    reg.register(WebSearch::default());
    reg.register(WebFetch);
    reg.register(Grep);
    reg.register(SearchReplace);
    reg
}
