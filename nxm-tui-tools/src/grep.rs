//! Grep tool: regex search over files, returning `file:line:text` matches.
//!
//! One surface: filesystem pattern matching. No glob, no context lines, no
//! ignore-file parsing, no async.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;
use serde_json::{json, Value};

use crate::{Tool, ToolKind, ToolResult};

/// Maximum number of matching lines returned before truncation.
const MAX_MATCHES: usize = 1000;

/// Directories skipped entirely during recursive traversal.
const EXCLUDED_DIRS: &[&str] = &[".git", "target"];

/// Search files for lines matching a regex; results as `file:line:text`.
///
/// ```
/// use nxm_tui_tools::{Tool, Grep};
///
/// let tool = Grep;
/// assert_eq!(tool.name(), "grep");
/// assert!(!tool.description().is_empty());
/// ```
pub struct Grep;

impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search files for lines matching a regex; results as `file:line:text`."
    }
    /// Filesystem search — pulled in by keyword match only (never fail-safe).
    fn kind(&self) -> ToolKind {
        ToolKind::Search
    }
    /// English + Italian task keywords (lowercase; matched case-insensitively).
    fn keywords(&self) -> &[&str] {
        &[
            "search",
            "find",
            "grep",
            "locate",
            "occurrence",
            "cerca",
            "trova",
            "regex",
            "pattern",
        ]
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for." },
                "path": { "type": "string", "description": "File or directory to search." }
            },
            "required": ["pattern", "path"]
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`pattern` argument is required"))?;
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`path` argument is required"))?;
        let path = Path::new(path);
        tracing::info!(
            target: crate::TARGET,
            "grep pattern={pattern:?} path={}",
            path.display()
        );
        let re = match Regex::new(pattern) {
            Ok(re) => re,
            Err(e) => {
                return Ok(ToolResult::err(format!("invalid regex `{pattern}`: {e}")));
            }
        };
        let metadata = fs::metadata(path).with_context(|| format!("stat {:?}", path.display()))?;
        let mut matches: Vec<(String, usize, String)> = Vec::new();
        if metadata.is_file() {
            grep_file(path, &re, &mut matches)?;
        } else if metadata.is_dir() {
            grep_dir(path, &re, &mut matches)?;
        } else {
            return Ok(ToolResult::err(format!(
                "{} is not a file or directory",
                path.display()
            )));
        }
        matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        Ok(ToolResult::ok(format_matches(&matches)))
    }
}

/// Grep a single file, appending matches to `out`. Skips non-UTF8/binary
/// files silently. Stops early once the global match cap is reached.
fn grep_file(path: &Path, re: &Regex, out: &mut Vec<(String, usize, String)>) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("read {:?}", path.display()))?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(t) => t,
        Err(_) => return Ok(()), // skip binary / non-UTF8 files
    };
    for (i, line) in text.lines().enumerate() {
        if re.is_match(line) {
            out.push((
                path.display().to_string(),
                i + 1, // 1-based line numbers
                line.to_string(),
            ));
            if out.len() >= MAX_MATCHES {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Recursively grep a directory, skipping excluded dirs and binary files.
fn grep_dir(dir: &Path, re: &Regex, out: &mut Vec<(String, usize, String)>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir {:?}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if EXCLUDED_DIRS.contains(&name) {
                    continue;
                }
            }
            grep_dir(&path, re, out)?;
            if out.len() >= MAX_MATCHES {
                return Ok(());
            }
        } else if path.is_file() {
            grep_file(&path, re, out)?;
            if out.len() >= MAX_MATCHES {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Format collected matches as `file:line:text` lines, plus a truncation
/// note when the cap was hit.
fn format_matches(matches: &[(String, usize, String)]) -> String {
    let mut out = String::new();
    for (path, line, text) in matches {
        out.push_str(&format!("{path}:{line}:{text}\n"));
    }
    if matches.len() >= MAX_MATCHES {
        out.push_str(&format!("... (truncated at {MAX_MATCHES} matches)\n"));
    }
    out
}
