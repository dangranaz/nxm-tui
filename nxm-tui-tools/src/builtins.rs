//! Built-in tool implementations: `read_file`, `list_resources`, `web_search`.
//!
//! Split out of `lib.rs` to respect the 300-line `RULES.md` cap (the registry
//! and selection logic live there). Re-exported from the crate root, so the
//! public API (`nxm_tui_tools::ReadFile`, etc.) is unchanged.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::warn;

use crate::{ssrf, Tool, ToolKind, ToolResult};

/// Read the full contents of a file (`args.path`).
pub struct ReadFile;

impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read the full contents of a file on disk."
    }
    /// Core: read access only, no write, no network — fail-safe set member.
    fn kind(&self) -> ToolKind {
        ToolKind::Core
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute or relative file path." }
            },
            "required": ["path"]
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`path` argument is required"))?;
        let content = std::fs::read_to_string(PathBuf::from(path))?;
        Ok(ToolResult::ok(content))
    }
}

/// List entries in a directory (`args.path`, default cwd).
pub struct ListResources;

impl Tool for ListResources {
    fn name(&self) -> &str {
        "list_resources"
    }
    fn description(&self) -> &str {
        "List files and directories in a path."
    }
    /// Core: read/navigate only, no write, no network — fail-safe set member.
    fn kind(&self) -> ToolKind {
        ToolKind::Core
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list; defaults to cwd." }
            },
            "required": []
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let mut names: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(PathBuf::from(path))? {
            let entry = entry?;
            names.push(entry.file_name().to_string_lossy().to_string());
        }
        names.sort();
        Ok(ToolResult::ok(json!({ "entries": names }).to_string()))
    }
}

/// Web search via a configurable HTTP endpoint (Nexum engine / provider).
/// Endpoint from `NEXUM_SEARCH_ENDPOINT` (default `http://localhost:8080`).
pub struct WebSearch {
    endpoint: String,
}

impl WebSearch {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }
}

impl Default for WebSearch {
    fn default() -> Self {
        let ep = std::env::var("NEXUM_SEARCH_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        Self::new(ep)
    }
}

impl Tool for WebSearch {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web via a configured HTTP search endpoint."
    }
    /// Network egress — pulled in by keyword match only (never in the fail-safe).
    fn kind(&self) -> ToolKind {
        ToolKind::Web
    }
    /// English + Italian task keywords (lowercase; matched case-insensitively).
    fn keywords(&self) -> &[&str] {
        &[
            "web search",
            "search online",
            "google",
            "lookup online",
            "cerca sul web",
            "ricerca web",
        ]
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query." }
            },
            "required": ["query"]
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`query` argument is required"))?;
        let mut url =
            reqwest::Url::parse(&format!("{}/search", self.endpoint.trim_end_matches('/')))?;
        url.query_pairs_mut().append_pair("q", query);
        let resp = reqwest::blocking::get(url.as_str())?;
        if !resp.status().is_success() {
            return Ok(ToolResult::err(format!(
                "search endpoint returned {}",
                resp.status()
            )));
        }
        let text = resp.text()?;
        Ok(ToolResult::ok(text))
    }
}

/// Fetch an URL via HTTP GET and return the body as plain text.
/// No HTML extraction, no markdown conversion, no sandbox policy.
///
/// ```
/// use nxm_tui_tools::{Tool, WebFetch};
///
/// let tool = WebFetch;
/// assert_eq!(tool.name(), "web_fetch");
/// assert!(!tool.description().is_empty());
/// ```
pub struct WebFetch;

impl Tool for WebFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetch an URL via HTTP GET and return the body as plain text."
    }
    /// Network egress — pulled in by keyword match only (never in the fail-safe).
    fn kind(&self) -> ToolKind {
        ToolKind::Web
    }
    /// English + Italian task keywords (lowercase; matched case-insensitively).
    fn keywords(&self) -> &[&str] {
        &[
            "fetch", "download", "url", "http", "https", "page", "scarica", "pagina",
        ]
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch (HTTP GET)." }
            },
            "required": ["url"]
        })
    }
    fn run(&self, args: &Value) -> Result<ToolResult> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("`url` argument is required"))?;
        tracing::info!(target: crate::TARGET, "web_fetch GET {url}");
        let resp = match ssrf::fetch_with_ssrf_guard(url) {
            Ok(r) => r,
            Err(e) => {
                warn!(target: crate::TARGET, "web_fetch SSRF block: {e}");
                // Include full error chain for debugging
                let mut msg = format!("SSRF guard: {e}");
                let mut source = e.source();
                while let Some(s) = source {
                    msg.push_str(&format!("\n  caused by: {s}"));
                    source = s.source();
                }
                return Ok(ToolResult::err(msg));
            }
        };
        if !resp.status().is_success() {
            return Ok(ToolResult::err(format!(
                "fetch {url} returned {}",
                resp.status()
            )));
        }
        let text = resp.text()?;
        Ok(ToolResult::ok(text))
    }
}
