//! LLM provider registry — presets + user-configured custom providers.
//!
//! A [`Provider`] pairs a human name with an OpenAI-compatible `base_url` and
//! the auth expectation. Built-in [`Provider::presets`] cover the vendors the
//! TUI ships with (Nexum Inferentia, Ollama, LM Studio, NVIDIA); custom ones
//! are persisted in `~/.nexum/tui.toml` via [`crate::config::TuiConfig`].
//!
//! Verified against vendor docs (2026-09):
//! - Ollama:    `http://localhost:11434/v1`  (no key)       — docs.ollama.com
//! - LM Studio: `http://localhost:1234/v1`   (key `lm-studio`) — lmstudio.ai
//! - NVIDIA:    `https://integrate.api.nvidia.com/v1` (Bearer NVIDIA_API_KEY) — build.nvidia.com
//! - Nexum Inferentia: `http://localhost:11434` (local Nexum engine)
//!
//! Rule: no `unwrap`/`expect` in prod — fallible input maps to `Result<_, String>`.

use serde::{Deserialize, Serialize};

/// A selectable LLM endpoint. `base_url` is the OpenAI-compatible root the
/// agent posts `/v1/chat/completions` against (so it either ends in `/v1` for
/// cloud/OpenAI-dialect servers, or is a bare host the agent appends `/v1` to).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    /// Human-facing name, e.g. `"Ollama"`. Used as the `/provider use <name>` key.
    pub name: String,
    /// OpenAI-compatible base URL, e.g. `"https://integrate.api.nvidia.com/v1"`.
    pub base_url: String,
    /// Whether this provider requires a Bearer API key.
    pub requires_api_key: bool,
    /// Conventional env var carrying the key (e.g. `"NVIDIA_API_KEY"`), if any.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Default model for this provider (set via `/provider use` or `/models`).
    /// `#[serde(default)]` so existing configs without it deserialize to `None`.
    #[serde(default)]
    pub default_model: Option<String>,
    /// True for cloud services (no local process to auto-detect).
    #[serde(default)]
    pub is_cloud: bool,
}

impl Provider {
    /// Built-in providers, in menu order. Ports/URLs verified against vendor docs.
    pub fn presets() -> Vec<Provider> {
        vec![
            Provider {
                name: "Nexum Inferentia".into(),
                base_url: "http://127.0.0.1:11434".into(),
                requires_api_key: false,
                api_key_env: None,
                default_model: None,
                is_cloud: false,
            },
            Provider {
                name: "Ollama".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                requires_api_key: false,
                api_key_env: None,
                default_model: None,
                is_cloud: false,
            },
            Provider {
                name: "LM Studio".into(),
                base_url: "http://127.0.0.1:1234/v1".into(),
                requires_api_key: false,
                api_key_env: None,
                default_model: None,
                is_cloud: false,
            },
            Provider {
                name: "NVIDIA".into(),
                base_url: "https://integrate.api.nvidia.com/v1".into(),
                requires_api_key: true,
                api_key_env: Some("NVIDIA_API_KEY".into()),
                default_model: None,
                is_cloud: true,
            },
        ]
    }

    /// Build a custom provider from user input, validating the URL scheme.
    /// Cloud is inferred from a non-loopback https URL, and an API key is then
    /// required.
    pub fn custom(name: &str, base_url: &str) -> Result<Provider, String> {
        let name = name.trim();
        let base_url = base_url.trim();
        if name.is_empty() {
            return Err("provider name must not be empty".into());
        }
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err("base URL must start with http:// or https://".into());
        }
        let is_local = base_url.contains("127.0.0.1")
            || base_url.contains("localhost")
            || base_url.contains("0.0.0.0");
        let is_cloud = !is_local;
        Ok(Provider {
            name: name.to_string(),
            base_url: base_url.to_string(),
            requires_api_key: is_cloud,
            api_key_env: None,
            default_model: None,
            is_cloud,
        })
    }
}

/// The full provider list: presets first, then user-custom (custom overrides a
/// preset with the same name). Read from config so it reflects `/provider add`.
pub fn all_providers(custom: &[Provider]) -> Vec<Provider> {
    let mut list = Provider::presets();
    for c in custom {
        if let Some(existing) = list.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&c.name)) {
            *existing = c.clone();
        } else {
            list.push(c.clone());
        }
    }
    list
}

/// Case-insensitive lookup by name across presets + custom providers.
pub fn find_provider(name: &str, custom: &[Provider]) -> Option<Provider> {
    all_providers(custom)
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
}
