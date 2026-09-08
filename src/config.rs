//! TUI configuration — persisted in ~/.nexum/tui.toml

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    pub endpoint: Option<String>,
    pub model_name: Option<String>,
    /// Bearer auth token for OpenAI/NVIDIA/Ollama-compatible servers.
    /// `#[serde(skip)]` so it is NEVER persisted to `~/.nexum/tui.toml`.
    #[serde(skip)]
    pub api_key: Option<String>,
    /// Sidebar visible by default
    pub sidebar_open: Option<bool>,
    /// Max context tokens
    pub max_context: Option<u32>,
    /// User-configured custom providers (persisted). Presets live in
    /// `crate::provider::Provider::presets()` and are never written here.
    /// API keys are NEVER stored on this struct's disk form (see `api_key`).
    #[serde(default)]
    pub providers: Vec<crate::provider::Provider>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            model_name: None,
            api_key: None,
            sidebar_open: Some(true),
            max_context: Some(8192),
            providers: Vec::new(),
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
        let mut cfg: Self = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        // `api_key` is serde-skipped (never on disk) — fill from env so the TUI
        // can auth against NVIDIA/Ollama/OpenAI: `NEXUM_API_KEY` first, then the
        // conventional `OPENAI_API_KEY`.
        if cfg.api_key.is_none() {
            cfg.api_key = std::env::var("NEXUM_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .ok();
        }
        cfg
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
