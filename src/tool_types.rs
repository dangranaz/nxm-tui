//! Tool call parts rendering for nexum-terminal TUI.
//! Based on OpenCode tool output rendering.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Approval policy: which tools run without asking.
///
/// `list_resources` only lists directory entries (read-only, no network, no
/// file contents) so it is auto-approved. Everything else — `read_file`
/// (arbitrary file contents, may include secrets), `web_search` (network
/// egress) and any unknown future tool — requires an explicit Y/N in the
/// approval overlay (codex `patch_approval` / kimi `approval-preview` pattern).
pub fn needs_approval(tool_name: &str) -> bool {
    !matches!(tool_name, "list_resources")
}

/// Risk badge for the approval overlay: an extra warning line on top of the
/// Y/N decision (advisory only — the user still decides).
///
/// - `read_file` / `list_resources` whose `path` looks secret-bearing
///   (`.env`, keys, credentials, `..` traversal, …) → sensitive badge.
/// - `web_search` → network-egress badge.
/// - Anything else gated (unknown future tools) → unrecognized-tool badge.
/// - `None` → no extra risk beyond the approval itself.
pub fn risk_badge(inv: &ToolInvocation) -> Option<&'static str> {
    match inv.name.as_str() {
        "web_search" => Some("⇄ network egress — query leaves the machine"),
        "read_file" | "list_resources" => {
            if is_sensitive_path(&args_path(&inv.args)) {
                Some("⚠ sensitive path — may expose secrets")
            } else {
                None
            }
        }
        _ => Some("⚠ unrecognized tool — check name and args"),
    }
}

/// Extract the `path` argument from a tool args JSON object, if present.
fn args_path(args: &str) -> String {
    serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default()
}

/// Conservative secret-bearing path matcher (case-insensitive, matches on
/// the file name or any path segment). Deliberately broad: a false positive
/// only adds a warning line, while a miss could leak a secret into the
/// transcript the model then re-sends.
fn is_sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    if lower.contains("..") {
        return true;
    }
    let file = lower.rsplit(['/', '\\']).next().unwrap_or(&lower);
    if file.starts_with(".env") {
        return true;
    }
    const SEGMENTS: &[&str] = &[".ssh", ".aws", ".gnupg", ".pki"];
    if lower.split(['/', '\\']).any(|seg| SEGMENTS.contains(&seg)) {
        return true;
    }
    const FRAGMENTS: &[&str] = &[
        ".pem",
        ".key",
        "id_rsa",
        "id_ed25519",
        "credential",
        "secret",
        "token",
        "password",
        "passwd",
        "private",
        "shadow",
        "sudoers",
    ];
    FRAGMENTS.iter().any(|frag| lower.contains(frag))
}

/// A request from the agent task to the main UI loop asking whether a tool
/// call may run. The UI answers through `reply` (true = run, false = skip).
/// Carried on its own channel because `ToolPart` must stay cloneable for the
/// transcript while a oneshot sender is single-use by design.
pub struct ApprovalRequest {
    pub invocation: ToolInvocation,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}

/// Tool call part types matching OpenAI API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPart {
    /// Text content part (streaming).
    Text(String),
    /// Reasoning token part.
    Reasoning(String),
    /// Tool call invocation.
    ToolInvocation(ToolInvocation),
    /// Tool call result.
    ToolResult(ToolResult),
    /// Error message.
    #[allow(dead_code)] // emitted on the agent error path; asserted in tests
    Error(String),
}

/// Tool invocation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    pub id: String,
    pub name: String,
    pub args: String,
}

/// Tool result data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
}

impl ToolPart {
    /// Creates a text part.
    #[allow(dead_code)] // convenience constructor for callers/tests
    pub fn text(content: String) -> Self {
        ToolPart::Text(content)
    }

    /// Creates a reasoning part.
    #[allow(dead_code)] // convenience constructor for callers/tests
    pub fn reasoning(content: String) -> Self {
        ToolPart::Reasoning(content)
    }

    /// Char-safe truncation: never splits a UTF-8 codepoint (the old byte
    /// slice `&s[..100]` panicked on multibyte boundaries). Appends "…" when
    /// truncated.
    pub fn truncate_chars(s: &str, max: usize) -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }

    /// Single-line args preview for the invocation row.
    pub fn args_preview(inv: &ToolInvocation, max: usize) -> String {
        let flat: String = inv.args.split_whitespace().collect::<Vec<_>>().join(" ");
        Self::truncate_chars(&flat, max)
    }

    /// Renders the tool part as lines.
    pub fn render_lines(&self) -> Vec<Line<'static>> {
        match self {
            ToolPart::Text(s) => {
                if s.is_empty() {
                    vec![]
                } else {
                    vec![Line::from(Span::raw(s.clone()))]
                }
            }
            ToolPart::Reasoning(s) => {
                vec![Line::from(vec![
                    Span::styled(" ◆ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(s.clone(), Style::default().fg(Color::DarkGray)),
                ])]
            }
            ToolPart::ToolInvocation(inv) => {
                let preview = Self::args_preview(inv, 80);
                if preview.is_empty() {
                    vec![Line::from(vec![
                        Span::styled(" ▶ ", Style::default().fg(Color::Cyan)),
                        Span::styled(inv.name.clone(), Style::default().fg(Color::Yellow)),
                    ])]
                } else {
                    vec![Line::from(vec![
                        Span::styled(" ▶ ", Style::default().fg(Color::Cyan)),
                        Span::styled(inv.name.clone(), Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!(" {preview}"),
                            Style::default().fg(Color::Rgb(140, 140, 140)),
                        ),
                    ])]
                }
            }
            ToolPart::ToolResult(res) => {
                let truncated = Self::truncate_chars(&res.output, 100);
                vec![Line::from(vec![
                    Span::styled(" ◀ ", Style::default().fg(Color::Cyan)),
                    Span::styled(truncated, Style::default().fg(Color::Rgb(180, 180, 80))),
                ])]
            }
            ToolPart::Error(e) => {
                vec![Line::from(Span::styled(
                    format!("Error: {}", e),
                    Style::default().fg(Color::Red),
                ))]
            }
        }
    }
}