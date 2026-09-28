//! Session management for nexum-terminal TUI.
//! Handles save/load/list of chat sessions.

use std::path::PathBuf;
use crate::app::{AgentMode, Message, Role};

/// Session storage directory (~/.nexum/sessions).
pub fn session_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".nexum")
        .join("sessions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Sanitize and validate a session name.
pub fn sanitize_session_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 64 {
        return Err("Session name must be 1-64 characters".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("Session name contains invalid characters".into());
    }
    Ok(name.to_string())
}

/// List saved session names (sorted alphabetically).
pub fn list_sessions() -> Vec<String> {
    let dir = session_dir();
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    sessions.push(name.to_string());
                }
            }
        }
    }
    sessions.sort();
    sessions
}

/// Save current chat session to JSON file.
pub fn save_session(name: &str, messages: &[Message], mode: AgentMode) -> Result<(), String> {
    let name = sanitize_session_name(name)?;
    let data = serde_json::json!({
        "mode": mode.label(),
        "messages": messages.iter().map(|m| serde_json::json!({
            "role": match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
                Role::Tool => "tool",
            },
            "content": m.content,
        })).collect::<Vec<_>>(),
    });
    let path = session_dir().join(format!("{name}.json"));
    let content = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load chat session from JSON file.
pub fn load_session(name: &str) -> Result<(Vec<Message>, AgentMode), String> {
    let name = sanitize_session_name(name)?;
    let path = session_dir().join(format!("{name}.json"));
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let mode = match data["mode"].as_str() {
        Some("Architect") => AgentMode::Architect,
        Some("Developer") => AgentMode::Developer,
        Some("Researcher") => AgentMode::Researcher,
        _ => AgentMode::Chat,
    };
    let messages = data["messages"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let role = match v["role"].as_str()? {
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        "system" => Role::System,
                        "tool" => Role::Tool,
                        _ => return None,
                    };
                    let content = v["content"].as_str()?.to_string();
                    Some(Message::new(role, content))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((messages, mode))
}