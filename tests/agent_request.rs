//! Agent request-body tests (MT-3): the `tools` payload sent to the model
//! is `registry.manifest_for(task)` — the automatic per-task selection
//! (Core tools + keyword match) union the manually enabled tools. The
//! manual `enabled` set starts empty.

use nxm_tui::agent::Agent;
use nxm_tui::app::{Message, Role};
use nxm_tui::plan_mode::plan_mode_handle;
use nxm_tui_tools::{default_registry, Registry};
use reqwest::Client;

/// Build an agent over a caller-supplied registry with an empty transcript.
fn agent_over(reg: Registry, client: &Client) -> Agent<'_> {
    Agent::new(
        client,
        "http://localhost:1",
        "test-model",
        None,
        Vec::new(),
        None,
        reg,
        plan_mode_handle(),
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    )
}

/// Build an agent whose last user message is `task`.
fn agent_with_task<'a>(reg: Registry, task: &str, client: &'a Client) -> Agent<'a> {
    Agent::new(
        client,
        "http://localhost:1",
        "test-model",
        None,
        vec![Message::new(Role::User, task.to_string())],
        None,
        reg,
        plan_mode_handle(),
        nxm_tui::bounded::DEFAULT_MAX_STEPS,
        nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD,
    )
}

fn tool_names(body: &serde_json::Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t["function"]["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn request_body_failsafe_core_only() {
    // R3: no user message → fail-safe = Core tools only.
    let client = Client::new();
    let agent = agent_over(default_registry(), &client);
    let names = tool_names(&agent.request_body());
    assert_eq!(names, vec!["read_file", "list_resources"]);
}

#[test]
fn request_body_uses_per_task_selection() {
    // R3: the last user message drives the automatic selection.
    let client = Client::new();
    let agent = agent_with_task(default_registry(), "edit this file", &client);
    let names = tool_names(&agent.request_body());
    assert!(names.iter().any(|n| n == "search_replace")); // keyword "edit"
    assert!(names.iter().any(|n| n == "read_file")); // Core
    assert!(names.iter().any(|n| n == "list_resources")); // Core
    assert_eq!(names.len(), 3);
}

#[test]
fn request_body_includes_manual_addition() {
    // R4: a manually enabled tool is added on top of the automatic selection
    // even when the task does not match it.
    let mut reg = default_registry();
    reg.set_enabled("web_fetch", true);
    let client = Client::new();
    let agent = agent_with_task(reg, "hello how are you", &client); // no keyword match
    let names = tool_names(&agent.request_body());
    assert!(names.iter().any(|n| n == "web_fetch")); // manual addition
    assert!(names.iter().any(|n| n == "read_file")); // Core fail-safe
    assert_eq!(names.len(), 3);
}
