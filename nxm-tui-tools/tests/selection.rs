//! Selection tests for `Registry` (T1 + MT-3): in-memory `enabled` state +
//! the filtered `manifest_selected()` the agent sends to the model.
//!
//! Contract (MT-3): the manual `enabled` set starts empty — registration no
//! longer auto-enables. `manifest()` stays the full compat manifest;
//! `manifest_selected()` reflects the empty manual set by default.

use nxm_tui_tools::{default_registry, Registry};

#[test]
fn default_manual_set_is_empty() {
    // R1: manual `enabled` set starts empty; tools are still registered.
    let reg = default_registry();
    let names = reg.tool_names();
    assert!(!names.is_empty(), "default registry has tools");
    for name in names {
        assert!(
            !reg.is_enabled(name),
            "`{name}` not manually enabled by default"
        );
    }
    // manifest_selected() reflects the empty manual set.
    assert!(reg.manifest_selected().is_empty());
    // manifest() (full compat manifest) is untouched.
    assert_eq!(reg.manifest().len(), 6);
}

#[test]
fn registering_a_tool_no_longer_auto_enables_it() {
    // MT-3: register() does not insert into the manual `enabled` set.
    let mut reg = Registry::new();
    reg.register(nxm_tui_tools::ReadFile);
    assert!(
        !reg.is_enabled("read_file"),
        "registration no longer auto-enables"
    );
    reg.register(nxm_tui_tools::ListResources);
    assert!(!reg.is_enabled("list_resources"));
}

#[test]
fn set_enabled_false_empties_manual_selected_manifest() {
    // MT-3: default manual set is empty — enable first, then disable.
    // `manifest_selected()` reflects only the manual set, so it empties.
    let mut reg = default_registry();
    reg.set_enabled("read_file", true);
    reg.set_enabled("read_file", false);

    assert!(!reg.is_enabled("read_file"));
    assert!(reg.manifest_selected().is_empty());
    // Compat: the unfiltered manifest still lists everything.
    assert_eq!(reg.manifest().len(), 6);
}

#[test]
fn set_enabled_true_re_enables_a_disabled_tool() {
    // MT-3: default manual set is empty — enable, then re-enable is a no-op.
    let mut reg = default_registry();
    reg.set_enabled("read_file", true);
    assert!(reg.is_enabled("read_file"));
    reg.set_enabled("read_file", true);
    assert!(reg.is_enabled("read_file"));
    assert_eq!(reg.manifest_selected().len(), 1);
}

#[test]
fn disable_all_yields_empty_selected_manifest() {
    let mut reg = default_registry();
    reg.disable_all();
    assert!(reg.manifest_selected().is_empty());
    // Compat manifest unaffected by the selection.
    assert_eq!(reg.manifest().len(), 6);
}

#[test]
fn enable_all_restores_full_selected_manifest() {
    let mut reg = default_registry();
    reg.disable_all();
    reg.enable_all();
    assert_eq!(reg.manifest_selected(), reg.manifest());
    for name in reg.tool_names() {
        assert!(reg.is_enabled(name));
    }
}

#[test]
fn tool_names_lists_registered_tools_in_registration_order() {
    let mut reg = Registry::new();
    reg.register(nxm_tui_tools::ReadFile);
    reg.register(nxm_tui_tools::ListResources);
    assert_eq!(reg.tool_names(), vec!["read_file", "list_resources"]);
}

// === MT-2: automatic per-task selection (`select` / `manifest_for`) ===
// The manual (enabled) set starts empty (MT-3), so `disable_all()` is a
// no-op here; targeted `set_enabled(...)` calls isolate the automatic part.

#[test]
fn select_always_includes_core() {
    let mut reg = default_registry();
    reg.disable_all();
    let sel = reg.select("something generic with no keyword");
    let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    assert_eq!(names, vec!["read_file", "list_resources"]);
}

#[test]
fn select_keyword_case_insensitive() {
    let mut reg = default_registry();
    reg.disable_all();

    let sel = reg.select("Please GREP for foo");
    let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"grep"));
    assert!(names.contains(&"read_file")); // Core still there

    let sel = reg.select("do a web SEARCH");
    let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"web_search")); // keyword "web search"

    let sel = reg.select("fammi una ricerca web");
    let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"web_search")); // keyword "ricerca web" (Italian)
}

#[test]
fn select_failsafe_core_only() {
    let mut reg = default_registry();
    reg.disable_all();
    let sel = reg.select("hello how are you");
    let names: Vec<&str> = sel.iter().map(|t| t.name()).collect();
    assert_eq!(names, vec!["read_file", "list_resources"]);
}

#[test]
fn manifest_for_unions_auto_and_manual() {
    let mut reg = default_registry();
    reg.disable_all();
    reg.set_enabled("web_fetch", true);

    let man = reg.manifest_for("edit this file");
    let names: Vec<&str> = man
        .iter()
        .filter_map(|v| v["function"]["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec!["read_file", "list_resources", "web_fetch", "search_replace"]
    );
    assert!(names.contains(&"search_replace")); // auto, keyword "edit"
    assert!(names.contains(&"read_file")); // Core
    assert!(names.contains(&"web_fetch")); // manual
    assert_eq!(
        names.iter().filter(|n| **n == "search_replace").count(),
        1,
        "no duplicates"
    );
}

#[test]
fn manifest_for_dedup_and_order() {
    let mut reg = default_registry();
    reg.disable_all();
    reg.set_enabled("search_replace", true); // both auto ("edit") and enabled
    reg.set_enabled("web_fetch", true); // manual only

    let man = reg.manifest_for("edit this file");
    let names: Vec<&str> = man
        .iter()
        .filter_map(|v| v["function"]["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec!["read_file", "list_resources", "web_fetch", "search_replace"]
    );
    assert_eq!(
        names.iter().filter(|n| **n == "search_replace").count(),
        1,
        "auto + enabled tool appears once"
    );
    // Registration order, not HashSet order.
    assert_eq!(names[0], "read_file");
    assert_eq!(names[3], "search_replace");
}

#[test]
fn manifest_for_default_registry_is_fail_safe_core_only() {
    // MT-3: manual set starts empty, so manifest_for on a default registry
    // reflects only the automatic selection — fail-safe = Core only when no
    // keyword matches.
    let reg = default_registry();
    let man = reg.manifest_for("hello how are you");
    let names: Vec<&str> = man
        .iter()
        .filter_map(|v| v["function"]["name"].as_str())
        .collect();
    assert_eq!(names, vec!["read_file", "list_resources"]);
}
