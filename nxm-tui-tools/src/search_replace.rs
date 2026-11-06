//! Search and replace tool: exact-string find→replace with atomic write.
//!
//! Replaces a single, unique occurrence of `old` with `new` in a file.
//! - `old` must occur exactly once (0 or >1 → error, no write).
//! - Write is atomic: temp file in same directory + rename.
//! - No regex, no replace-all, no multi-file.
//!
//! ```
//! use nxm_tui_tools::{Tool, SearchReplace};
//!
//! let tool = SearchReplace;
//! assert_eq!(tool.name(), "search_replace");
//! assert!(!tool.description().is_empty());
//! ```

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::{Tool, ToolKind, ToolResult, TARGET};

/// Replace exactly one occurrence of `old` with `new` in a file.
///
/// # Example
/// ```
/// use std::fs;
/// use tempfile::tempdir;
/// use nxm_tui_tools::{Tool, SearchReplace};
///
/// let dir = tempdir().unwrap();
/// let path = dir.path().join("test.txt");
/// fs::write(&path, "hello world").unwrap();
///
/// let tool = SearchReplace;
/// let args = serde_json::json!({
///     "path": path.to_string_lossy(),
///     "old": "world",
///     "new": "rust"
/// });
/// let res = tool.run(&args).unwrap();
/// assert!(res.ok);
/// assert_eq!(fs::read_to_string(&path).unwrap(), "hello rust");
/// ```
pub struct SearchReplace;

impl Tool for SearchReplace {
    fn name(&self) -> &str {
        "search_replace"
    }
    fn description(&self) -> &str {
        "Replace exactly one occurrence of a string in a file (exact match, atomic write)."
    }
    /// File mutation — pulled in by keyword match only (never fail-safe).
    fn kind(&self) -> ToolKind {
        ToolKind::Edit
    }
    /// English + Italian task keywords (lowercase; matched case-insensitively).
    fn keywords(&self) -> &[&str] {
        &[
            "edit",
            "replace",
            "modify",
            "change",
            "rewrite",
            "fix",
            "insert",
            "sostituisci",
            "modifica",
            "cambia",
        ]
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to edit." },
                "old": { "type": "string", "description": "Exact string to find (must occur exactly once)." },
                "new": { "type": "string", "description": "Replacement string." }
            },
            "required": ["path", "old", "new"]
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`path` argument is required"))?;
        let old = args
            .get("old")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`old` argument is required"))?;
        let new = args
            .get("new")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`new` argument is required"))?;

        if old.is_empty() {
            return Ok(ToolResult::err("`old` must not be empty".to_string()));
        }
        if old == new {
            return Ok(ToolResult::err(
                "`old` and `new` are identical, no change".to_string(),
            ));
        }

        let path = Path::new(path);
        tracing::info!(target: TARGET, "search_replace path={} old_len={} new_len={}", path.display(), old.len(), new.len());

        let content =
            fs::read_to_string(path).with_context(|| format!("read file {}", path.display()))?;

        // Count occurrences
        let count = content.matches(old).count();
        if count == 0 {
            return Ok(ToolResult::err(format!(
                "`old` not found in {} (0 matches)",
                path.display()
            )));
        }
        if count > 1 {
            return Ok(ToolResult::err(format!(
                "`old` occurs {} times in {} (ambiguous, make `old` more specific)",
                count,
                path.display()
            )));
        }

        // Exactly one match - perform replacement
        let new_content = content.replace(old, new);

        // Atomic write: temp file in same directory + rename
        let temp_path = path.with_file_name(format!(
            ".{}.tmp",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("search_replace")
        ));
        fs::write(&temp_path, new_content)
            .with_context(|| format!("write temp file {}", temp_path.display()))?;
        if let Err(e) = fs::rename(&temp_path, path) {
            // Clean up temp file on rename failure
            let _ = fs::remove_file(&temp_path);
            return Err(e).with_context(|| format!("rename temp file to {}", path.display()));
        }

        Ok(ToolResult::ok(format!(
            "1 replacement in {}",
            path.display()
        )))
    }
}
