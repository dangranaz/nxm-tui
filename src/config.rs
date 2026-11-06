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
    /// Hard cap on `Agent::run` iterations (loopeng-01). `None` = built-in
    /// default (`bounded::DEFAULT_MAX_STEPS`), applied at the call site so
    /// `bounded.rs` stays the single source of the default.
    #[serde(default)]
    pub max_steps: Option<u32>,
    /// Consecutive identical tool-call iterations before the stagnation
    /// breaker trips (loopeng-01-02). `None` = built-in default
    /// (`circuit_breaker::DEFAULT_STALL_THRESHOLD`), applied at the call site
    /// so `circuit_breaker.rs` stays the single source of the default.
    #[serde(default)]
    pub stall_threshold: Option<u32>,
    /// User-configured custom providers (persisted). Presets live in
    /// `crate::provider::Provider::presets()` and are never written here.
    /// API keys are NEVER stored on this struct's disk form (see `keys.rs`).
    #[serde(default)]
    pub providers: Vec<crate::provider::Provider>,
    /// MCP host settings (server to spawn for sampling). See `crate::mcp`.
    #[serde(default)]
    pub mcp: McpConfig,
}

/// Configuration for the MCP host subsystem.
///
/// The TUI spawns `command args...` as its own stdio child when `enabled` and
/// the binary is launchable, then serves `sampling/createMessage` from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Whether to attempt an MCP session at all.
    pub enabled: bool,
    /// Server binary (name on PATH or absolute path).
    pub command: String,
    /// Arguments passed to the server binary.
    pub args: Vec<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            command: "nxm-session-mcp".to_string(),
            args: vec!["--transport".to_string(), "stdio".to_string()],
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            model_name: None,
            sidebar_open: Some(true),
            max_context: Some(8192),
            max_steps: None,
            stall_threshold: None,
            providers: Vec::new(),
            mcp: McpConfig::default(),
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

    /// Add or replace a custom provider, then persist.
    pub fn upsert_provider(&mut self, provider: crate::provider::Provider) {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(&provider.name))
        {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
        self.save();
    }

    /// Remove a custom provider by name (case-insensitive). Returns whether one
    /// was removed. Presets cannot be removed (they are not stored here).
    pub fn remove_provider(&mut self, name: &str) -> bool {
        let before = self.providers.len();
        self.providers
            .retain(|p| !p.name.eq_ignore_ascii_case(name.trim()));
        let removed = self.providers.len() != before;
        if removed {
            self.save();
        }
        removed
    }

    /// Validate config values.
    /// Returns Ok(()) if valid, Err(String) with error message.
    #[allow(dead_code)] // used by config validation path (not yet called on load)
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref ep) = self.endpoint {
            if !ep.starts_with("http://") && !ep.starts_with("https://") {
                return Err("Endpoint must start with http:// or https://".into());
            }
        }

        if let Some(ctx) = self.max_context {
            if !(256..=1_000_000).contains(&ctx) {
                return Err("Max context must be between 256 and 1,000,000".into());
            }
        }

        Ok(())
    }
}
