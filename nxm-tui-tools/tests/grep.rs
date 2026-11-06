//! Tests for the `grep` tool (R1-R6).
//!
//! Uses `tempfile` (already a dev-dependency) for temporary file trees.
//! No new dependencies.

use std::fs;
use std::path::Path;

use nxm_tui_tools::default_registry;
use serde_json::json;

fn write_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).expect("write test file");
    path
}

#[test]
fn grep_schema() {
    let reg = default_registry();
    let tool = reg.get("grep").expect("grep registered");
    assert_eq!(tool.name(), "grep");
    assert!(!tool.description().is_empty());
    let schema = tool.schema();
    let required = schema["required"].as_array().expect("required array");
    assert!(required.iter().any(|v| v.as_str() == Some("pattern")));
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
    assert_eq!(
        schema["properties"]["pattern"]["type"].as_str(),
        Some("string")
    );
    assert_eq!(
        schema["properties"]["path"]["type"].as_str(),
        Some("string")
    );
}

#[test]
fn grep_missing_args() {
    let reg = default_registry();
    assert!(reg.call("grep", &json!({})).is_err());
    assert!(reg.call("grep", &json!({ "pattern": "x" })).is_err());
    assert!(reg.call("grep", &json!({ "path": "/" })).is_err());
}

#[test]
fn grep_invalid_regex() {
    let reg = default_registry();
    let args = json!({ "pattern": "(", "path": "/" });
    let res = reg.call("grep", &args).expect("run grep");
    assert!(!res.ok);
}

#[test]
fn grep_single_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_file(dir.path(), "note.txt", "alpha\nbeta gamma\nalpha\n");
    let reg = default_registry();
    let args = json!({ "pattern": "gamma", "path": path.to_string_lossy() });
    let res = reg.call("grep", &args).expect("run grep");
    assert!(res.ok);
    assert!(res
        .content
        .contains(&format!("{}:2:beta gamma", path.display())));
}

#[test]
fn grep_directory_excludes() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(dir.path(), "a.txt", "needle\nhay\n");
    write_file(dir.path(), "b.txt", "needle\n");
    let target = dir.path().join("target");
    fs::create_dir(&target).expect("create target dir");
    write_file(&target, "c.txt", "needle\n");
    let reg = default_registry();
    let args = json!({ "pattern": "needle", "path": dir.path().to_string_lossy() });
    let res = reg.call("grep", &args).expect("run grep");
    assert!(res.ok);
    assert!(res.content.contains("a.txt:1:needle"));
    assert!(res.content.contains("b.txt:1:needle"));
    assert!(!res.content.contains("target/c.txt:1:needle"));
    // Ordering: path then line number.
    let a_idx = res.content.find("a.txt:1:needle").expect("a.txt match");
    let b_idx = res.content.find("b.txt:1:needle").expect("b.txt match");
    assert!(a_idx < b_idx);
}

#[test]
fn grep_in_default_registry() {
    let reg = default_registry();
    // MT-3: registration no longer auto-enables; verify registration only.
    assert!(reg.tool_names().contains(&"grep"));
}
