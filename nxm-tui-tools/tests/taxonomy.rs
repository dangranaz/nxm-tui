//! Integration tests for the `ToolKind` taxonomy + `Tool::kind()`/`keywords()`
//! (RULES: tests under `tests/`, not inline). MT-1 is pure groundwork: no
//! selection behavior here — R5 (identical `manifest()`/`manifest_selected()`)
//! is covered by the untouched `tools`/`selection`/`agent_request` tests.

use anyhow::Result;
use nxm_tui_tools::{
    default_registry, Grep, ListResources, ReadFile, SearchReplace, Tool, ToolKind, WebFetch,
    WebSearch,
};
use serde_json::{json, Value};

/// Test-only Tool that does NOT override `kind()`/`keywords()` — exercises the
/// trait defaults (`ToolKind::Other`, empty keywords).
struct Dummy;

impl Tool for Dummy {
    fn name(&self) -> &str {
        "dummy"
    }
    fn description(&self) -> &str {
        "Test-only tool exercising the trait defaults."
    }
    fn schema(&self) -> Value {
        json!({})
    }
    fn run(&self, _args: &Value) -> Result<nxm_tui_tools::ToolResult> {
        Ok(nxm_tui_tools::ToolResult::ok("ok".to_string()))
    }
}

/// R1 — trait defaults: no override → `Other`, empty keywords.
#[test]
fn kind_defaults() {
    assert_eq!(Dummy.kind(), ToolKind::Other);
    assert!(Dummy.keywords().is_empty());
}

/// R2 — the two Core tools (fail-safe set for MT-2).
#[test]
fn core_tools_classified() {
    assert_eq!(ReadFile.kind(), ToolKind::Core);
    assert_eq!(ListResources.kind(), ToolKind::Core);
}

/// R3 — per-tool classification.
#[test]
fn tools_kinds() {
    assert_eq!(Grep.kind(), ToolKind::Search);
    assert_eq!(SearchReplace.kind(), ToolKind::Edit);
    assert_eq!(WebFetch.kind(), ToolKind::Web);
    assert_eq!(WebSearch::default().kind(), ToolKind::Web);
}

/// R4 — non-Core tools carry keywords (EN + IT, case handled by the select in
/// MT-2); Core tools may stay empty (they bypass keyword matching).
#[test]
fn keyword_tools_have_keywords() {
    assert!(Grep.keywords().contains(&"grep"));
    assert!(SearchReplace.keywords().contains(&"replace"));
    assert!(WebFetch.keywords().contains(&"fetch"));
    assert!(WebSearch::default().keywords().contains(&"web search"));
    assert!(ReadFile.keywords().is_empty());
    assert!(ListResources.keywords().is_empty());
}

/// Sanity: every default-registry tool exposes a kind (taxonomy complete).
#[test]
fn registry_tools_all_kinds_known() {
    let reg = default_registry();
    for name in reg.tool_names() {
        let kind = reg.get(name).expect("registered").kind();
        assert_ne!(kind, ToolKind::Other, "`{name}` left as Other");
    }
}
