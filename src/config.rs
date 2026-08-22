//! TUI configuration — persisted in ~/.nexum/tui.toml

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    pub endpoint: Option<String>,
    pub model_name: Option<String>,
    /// Sidebar visible by default
    pub sidebar_open: Option<bool>,
    /// Max context tokens
    pub max_context: Option<u32>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            model_name: None,
            sidebar_open: Some(true),
            max_context: Some(8192),
        }
    }
}

impl TuiConfig {
    fn path() -> std::path::PathBuf {
        let dir = dirs::home_dir().unwrap_or_default().join(".nexum");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("tui.toml")
    }

    /// Load config from file, or return defaults.
    pub fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save config to file.
    pub fn save(&self) {
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), s);
        }
    }

    /// Validate config values.
    /// Returns Ok(()) if valid, Err(String) with error message.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref ep) = self.endpoint {
            if !ep.starts_with("http://") && !ep.starts_with("https://") {
                return Err("Endpoint must start with http:// or https://".into());
            }
        }

        if let Some(ctx) = self.max_context {
            if ctx < 256 || ctx > 1_000_000 {
                return Err("Max context must be between 256 and 1,000,000".into());
            }
        }

        Ok(())
    }
}
