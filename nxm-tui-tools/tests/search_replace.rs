//! Tests for the `search_replace` tool (R1-R8).
//!
//! Uses `tempfile` (already a dev-dependency) for temporary files.
//! No new dependencies.

use std::fs;

use nxm_tui_tools::default_registry;
use serde_json::json;

fn write_temp_file(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("write test file");
    path
}

#[test]
fn sr_schema() {
    let reg = default_registry();
    let tool = reg
        .get("search_replace")
        .expect("search_replace registered");
    assert_eq!(tool.name(), "search_replace");
    assert!(!tool.description().is_empty());
    let schema = tool.schema();
    let required = schema["required"].as_array().expect("required array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
    assert!(required.iter().any(|v| v.as_str() == Some("old")));
    assert!(required.iter().any(|v| v.as_str() == Some("new")));
    assert_eq!(
        schema["properties"]["path"]["type"].as_str(),
        Some("string")
    );
    assert_eq!(schema["properties"]["old"]["type"].as_str(), Some("string"));
    assert_eq!(schema["properties"]["new"]["type"].as_str(), Some("string"));
}

#[test]
fn sr_missing_args() {
    let reg = default_registry();
    assert!(reg.call("search_replace", &json!({})).is_err());
    assert!(reg
        .call("search_replace", &json!({ "path": "/tmp/x" }))
        .is_err());
    assert!(reg
        .call("search_replace", &json!({ "old": "foo" }))
        .is_err());
    assert!(reg
        .call("search_replace", &json!({ "new": "bar" }))
        .is_err());
    assert!(reg
        .call("search_replace", &json!({ "path": "/tmp/x", "old": "foo" }))
        .is_err());
    assert!(reg
        .call("search_replace", &json!({ "path": "/tmp/x", "new": "bar" }))
        .is_err());
    assert!(reg
        .call("search_replace", &json!({ "old": "foo", "new": "bar" }))
        .is_err());
}

#[test]
fn sr_unique_replace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_temp_file(&dir, "test.txt", "hello world\nfoo bar\n");
    let original = fs::read_to_string(&path).expect("read original");

    let reg = default_registry();
    let args = json!({
        "path": path.to_string_lossy(),
        "old": "foo bar",
        "new": "baz qux"
    });
    let res = reg
        .call("search_replace", &args)
        .expect("run search_replace");
    assert!(res.ok);
    assert!(res.content.contains("1 replacement"));

    // Verify file content was changed
    let updated = fs::read_to_string(&path).expect("read updated");
    assert_eq!(updated, "hello world\nbaz qux\n");
    assert_ne!(updated, original);
}

#[test]
fn sr_no_match_no_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_temp_file(&dir, "test.txt", "hello world\nfoo bar\n");
    let original = fs::read_to_string(&path).expect("read original");

    let reg = default_registry();
    let args = json!({
        "path": path.to_string_lossy(),
        "old": "nonexistent",
        "new": "something"
    });
    let res = reg
        .call("search_replace", &args)
        .expect("run search_replace");
    assert!(!res.ok);
    assert!(res.content.to_lowercase().contains("not found"));

    // Verify file was NOT modified
    let after = fs::read_to_string(&path).expect("read after");
    assert_eq!(after, original);
}

#[test]
fn sr_ambiguous_no_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_temp_file(&dir, "test.txt", "foo\nfoo\nbar\n");
    let original = fs::read_to_string(&path).expect("read original");

    let reg = default_registry();
    let args = json!({
        "path": path.to_string_lossy(),
        "old": "foo",
        "new": "baz"
    });
    let res = reg
        .call("search_replace", &args)
        .expect("run search_replace");
    assert!(!res.ok);
    assert!(
        res.content.to_lowercase().contains("ambiguous")
            || res.content.to_lowercase().contains("multiple")
    );

    // Verify file was NOT modified
    let after = fs::read_to_string(&path).expect("read after");
    assert_eq!(after, original);
}

#[test]
fn sr_empty_old() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_temp_file(&dir, "test.txt", "hello world\n");
    let original = fs::read_to_string(&path).expect("read original");

    let reg = default_registry();
    let args = json!({
        "path": path.to_string_lossy(),
        "old": "",
        "new": "something"
    });
    let res = reg
        .call("search_replace", &args)
        .expect("run search_replace");
    assert!(!res.ok);
    assert!(res.content.to_lowercase().contains("empty"));

    // Verify file was NOT modified
    let after = fs::read_to_string(&path).expect("read after");
    assert_eq!(after, original);
}

#[test]
fn sr_no_temp_left() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_temp_file(&dir, "test.txt", "hello world\nfoo bar\n");

    let reg = default_registry();
    let args = json!({
        "path": path.to_string_lossy(),
        "old": "foo bar",
        "new": "baz qux"
    });
    let res = reg
        .call("search_replace", &args)
        .expect("run search_replace");
    assert!(res.ok);

    // Verify no temp files left in the directory
    let entries: Vec<_> = fs::read_dir(dir.path()).expect("read dir").collect();
    let file_names: Vec<String> = entries
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    // Only the test file should exist
    assert_eq!(file_names.len(), 1);
    assert_eq!(file_names[0], "test.txt");
}

#[test]
fn sr_in_default_registry() {
    let reg = default_registry();
    // MT-3: registration no longer auto-enables; verify registration only.
    assert!(reg.tool_names().contains(&"search_replace"));
}
