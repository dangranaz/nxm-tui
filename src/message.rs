//! Domain message types: the `Message` struct and `Role` enum, plus their
//! constructors.
//!
//! Extracted verbatim from `app.rs` (T6 split, no behavior change) and
//! re-exported from `app` so existing call sites keep working.

use crate::tool_types::ToolPart;

/// A single message in a chat transcript.
///
/// # Examples
///
/// ```text
/// use nxm_tui::message::{Message, Role};
///
/// let msg = Message::new(Role::User, "hello".into());
/// assert_eq!(msg.role, Role::User);
/// assert_eq!(msg.content, "hello");
/// ```
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Optional tool parts for tool call output.
    pub tool_parts: Vec<ToolPart>,
    /// `tool_call_id` for `role: "tool"` messages (OpenAI round-trip).
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a plain message with the given role and content.
    ///
    /// # Examples
    ///
    /// ```text
    /// use nxm_tui::message::{Message, Role};
    ///
    /// let msg = Message::new(Role::Assistant, "hi".into());
    /// assert!(msg.tool_parts.is_empty());
    /// assert!(msg.tool_call_id.is_none());
    /// ```
    pub fn new(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            tool_parts: Vec::new(),
            tool_call_id: None,
        }
    }

    /// Create a message carrying tool parts (e.g. a tool call result) with no
    /// text content.
    ///
    /// # Examples
    ///
    /// ```text
    /// use nxm_tui::message::{Message, Role};
    /// use nxm_tui::tool_types::ToolPart;
    ///
    /// let msg = Message::with_tool(Role::Tool, vec![]);
    /// assert_eq!(msg.role, Role::Tool);
    /// assert!(msg.content.is_empty());
    /// ```
    pub fn with_tool(role: Role, parts: Vec<ToolPart>) -> Self {
        Self {
            role,
            content: String::new(),
            tool_parts: parts,
            tool_call_id: None,
        }
    }

    /// A `role: "tool"` result message carrying the assistant `tool_call_id`.
    ///
    /// # Examples
    ///
    /// ```text
    /// use nxm_tui::message::{Message, Role};
    ///
    /// let msg = Message::tool("call_1", "ok".into());
    /// assert_eq!(msg.role, Role::Tool);
    /// assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
    /// ```
    pub fn tool(call_id: &str, content: String) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_parts: Vec::new(),
            tool_call_id: Some(call_id.to_string()),
        }
    }
}

/// The role of a message author in a chat transcript.
///
/// # Examples
///
/// ```text
/// use nxm_tui::message::Role;
///
/// let role = Role::User;
/// assert_eq!(role as u8, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}
