//! Sidebar component for nexum-terminal TUI.
//! Renders session title, model info, and workspace status.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Sidebar width constant (matches opencode: 42 chars)
pub const SIDEBAR_WIDTH: u16 = 42;

/// Sidebar rendering state containing session and workspace info.
#[derive(Debug, Clone)]
pub struct SidebarInfo {
    /// Session title (truncated if too long)
    pub title: String,
    /// Model name being used
    pub model: String,
    /// Current working directory (relative to home)
    pub directory: String,
    /// Git branch (if in a git repo)
    pub git_branch: Option<String>,
}

impl Default for SidebarInfo {
    fn default() -> Self {
        Self {
            title: "unnamed".to_string(),
            model: "—".to_string(),
            directory: "".to_string(),
            git_branch: None,
        }
    }
}

impl SidebarInfo {
    /// Creates sidebar info from app state.
    pub fn from_app(app: &crate::app::App) -> Self {
        let cwd = crate::app::current_dir_name();
        let git_branch = get_git_branch();
        Self {
            title: app.session_display_name(),
            model: app.model_worker.name.clone(),
            directory: cwd,
            git_branch,
        }
    }
}

/// Get current git branch if in a git repository.
/// Returns None if not in a git repo or if git command fails.
pub fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
}

/// Render the sidebar into the provided buffer.
pub fn render_sidebar(info: &SidebarInfo, area: Rect, buf: &mut Buffer) {
    let lines = sidebar_lines(info);
    let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    paragraph.render(area, buf);
}

/// Generate sidebar content lines.
/// Structure matches opencode sidebar: title, model, then workspace.
fn sidebar_lines(info: &SidebarInfo) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    
    // Title line: "OC • {title}" (mimics OpenCode's "OC" prefix)
    lines.push(Line::from(vec![
        Span::styled(" OC ", Style::default().fg(Color::Cyan).bg(Color::Rgb(50, 50, 50))),
        Span::styled("• ", Style::default().fg(Color::DarkGray)),
        Span::styled(truncate_title(&info.title), Style::default().fg(Color::White)),
    ]));
    
    // Model line (gray, smaller)
    lines.push(Line::from(Span::styled(
        format!(" {}", info.model),
        Style::default().fg(Color::Rgb(120, 120, 120)),
    )));
    
    // Empty line for spacing
    lines.push(Line::from(""));
    
    // Workspace line: "~/{dir} [{branch}]" or "~/{dir}" if no branch
    let workspace = if let Some(ref branch) = info.git_branch {
        format!("~/{}/{}", info.directory, branch)
    } else {
        format!("~/{}/", info.directory)
    };
    
    lines.push(Line::from(Span::styled(
        workspace,
        Style::default().fg(Color::Rgb(100, 100, 100)),
    )));
    
    lines
}

/// Truncate title for sidebar display (max 20 chars).
fn truncate_title(title: &str) -> String {
    if title.len() > 20 {
        format!("{}...", &title[..17])
    } else {
        title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_truncate_title_short() {
        assert_eq!(truncate_title("short"), "short");
    }
    
    #[test]
    fn test_truncate_title_long() {
        assert_eq!(truncate_title("this is a very long title"), "this is a very lo...");
    }
    
    #[test]
    fn test_sidebar_info_default() {
        let info = SidebarInfo::default();
        assert_eq!(info.title, "unnamed");
        assert_eq!(info.model, "—");
    }
}