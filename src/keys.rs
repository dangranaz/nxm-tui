//! Per-provider API key resolution via keychain + env fallback.
//!
//! Resolution order (D7 hard):
//! 1. Keychain entry `nexum-tui` / `<provider name>` (via `spawn_blocking`)
//! 2. Provider's `api_key_env` (e.g. `NVIDIA_API_KEY`)
//! 3. Global `NEXUM_API_KEY`
//! 4. Global `OPENAI_API_KEY`
//! 5. Missing
//!
//! Keychain errors `NoDefaultStore`/`PlatformFailure`/`NoStorageAccess` =
//! keychain unavailable → env fallback. `NoEntry` = no entry → env fallback.
//! `Error` is `#[non_exhaustive]` → always wildcard branch.
//!
//! mai-log RIGID: key value NEVER logged/printed (overlay, debug log, test output).
//! Logs emit only: source, presence, env var name.

use std::env;

use keyring::Entry;
use tracing::{debug, info, warn};

use crate::provider::Provider;

/// Result of key resolution for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyResolution {
    /// Key found in keychain.
    Keychain(String),
    /// Key found in environment variable. `keychain_available` = whether the
    /// keychain was reachable at all (false on NoDefaultStore / PlatformFailure /
    /// NoStorageAccess).
    Env { key: String, keychain_available: bool },
    /// No key found anywhere.
    Missing,
}

impl KeyResolution {
    /// Consume and return the key string if present.
    pub fn into_key(self) -> Option<String> {
        match self {
            KeyResolution::Keychain(key) | KeyResolution::Env { key, .. } => Some(key),
            KeyResolution::Missing => None,
        }
    }

    /// Whether a key was found (any source).
    pub fn is_some(&self) -> bool {
        !matches!(self, KeyResolution::Missing)
    }

    /// Source label for status/logging (never the value).
    pub fn source(&self) -> &'static str {
        match self {
            KeyResolution::Keychain(_) => "keychain",
            KeyResolution::Env { .. } => "env",
            KeyResolution::Missing => "missing",
        }
    }

    /// Env var name if source is Env, for one-time warning.
    pub fn env_var(&self) -> Option<&str> {
        match self {
            KeyResolution::Env { key: _, keychain_available: _ } => None, // would need to track which env
            _ => None,
        }
    }
}

/// Resolve the API key for a provider.
///
/// Called ONCE at Agent spawn (never in the render loop). Keychain access runs
/// on `spawn_blocking` from the caller (`main.rs`).
pub fn resolve_key(provider: &Provider) -> KeyResolution {
    // 1. Keychain
    let entry = match Entry::new("nexum-tui", &provider.name) {
        Ok(e) => e,
        Err(e) => {
            warn!(target: "nexum::keys", provider = %provider.name, error = %e, "keychain entry creation failed");
            return resolve_env_fallback(provider, false);
        }
    };
    match entry.get_password() {
        Ok(key) => {
            info!(target: "nexum::keys", provider = %provider.name, source = "keychain", "key resolved");
            return KeyResolution::Keychain(key);
        }
        Err(keyring::Error::NoEntry) => {
            debug!(target: "nexum::keys", provider = %provider.name, "no keychain entry");
        }
        Err(keyring::Error::NoDefaultStore) |
        Err(keyring::Error::PlatformFailure(_)) |
        Err(keyring::Error::NoStorageAccess(_)) => {
            warn!(target: "nexum::keys", provider = %provider.name, "keychain unavailable");
            return resolve_env_fallback(provider, false);
        }
        Err(e) => {
            // keyring::Error is #[non_exhaustive] — wildcard catch-all.
            warn!(target: "nexum::keys", provider = %provider.name, error = %e, "keychain error");
            return resolve_env_fallback(provider, false);
        }
    }

    // 2-4. Env fallback (keychain was available but no entry)
    resolve_env_fallback(provider, true)
}

fn resolve_env_fallback(provider: &Provider, keychain_available: bool) -> KeyResolution {
    // Provider-specific env var
    if let Some(ref env_var) = provider.api_key_env {
        if let Ok(key) = env::var(env_var) {
            if !key.is_empty() {
                info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = %env_var, "key resolved");
                return KeyResolution::Env { key, keychain_available };
            }
        }
    }

    // Global NEXUM_API_KEY
    if let Ok(key) = env::var("NEXUM_API_KEY") {
        if !key.is_empty() {
            info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = "NEXUM_API_KEY", "key resolved");
            return KeyResolution::Env { key, keychain_available };
        }
    }

    // Global OPENAI_API_KEY
    if let Ok(key) = env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = "OPENAI_API_KEY", "key resolved");
            return KeyResolution::Env { key, keychain_available };
        }
    }

    info!(target: "nexum::keys", provider = %provider.name, source = "missing", "no key found");
    KeyResolution::Missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring_core::{set_default_store, mock::Store, Error};
    use keyring_core::mock::Cred;

    /// Setup a fresh mock store. Call once per test process.
    /// Returns a handle to the store for configuring credentials.
    fn setup_mock() {
        let store = Store::new().unwrap();
        let _ = set_default_store(store);
    }

    #[test]
    fn keychain_beats_env() {
        setup_mock();
        let entry = keyring_core::Entry::new("nexum-tui", "TestProvider1").unwrap();
        entry.set_password("keychain-key").unwrap();

        let provider = Provider {
            name: "TestProvider1".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_1".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::set_var("TEST_API_KEY_1", "env-key");

        let res = resolve_key(&provider);
        assert_eq!(res, KeyResolution::Keychain("keychain-key".into()));
        assert_eq!(res.into_key(), Some("keychain-key".into()));
    }

    #[test]
    fn fallback_to_provider_env_on_no_entry() {
        // Use a fresh provider name to avoid interference from test 1
        let _ = setup_mock(); // no-op after first call, but keeps pattern

        let provider = Provider {
            name: "TestProvider2".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_2".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::set_var("TEST_API_KEY_2", "env-key");

        let res = resolve_key(&provider);
        match res {
            KeyResolution::Env { key, keychain_available } => {
                assert_eq!(key, "env-key");
                assert!(keychain_available);
            }
            _ => panic!("expected Env, got {:?}", res),
        }
    }

    #[test]
    fn fallback_to_global_nexum_on_missing_provider_env() {
        let _ = setup_mock();

        let provider = Provider {
            name: "TestProvider3".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_3".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::remove_var("TEST_API_KEY_3");
        std::env::set_var("NEXUM_API_KEY", "nexum-key");

        let res = resolve_key(&provider);
        match res {
            KeyResolution::Env { key, keychain_available } => {
                assert_eq!(key, "nexum-key");
                assert!(keychain_available);
            }
            _ => panic!("expected Env, got {:?}", res),
        }
    }

    #[test]
    fn fallback_to_openai_on_missing_nexum() {
        let _ = setup_mock();

        let provider = Provider {
            name: "TestProvider4".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_4".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::remove_var("TEST_API_KEY_4");
        std::env::remove_var("NEXUM_API_KEY");
        std::env::set_var("OPENAI_API_KEY", "openai-key");

        let res = resolve_key(&provider);
        match res {
            KeyResolution::Env { key, keychain_available } => {
                assert_eq!(key, "openai-key");
                assert!(keychain_available);
            }
            _ => panic!("expected Env, got {:?}", res),
        }
    }

    #[test]
    fn missing_when_nothing_set() {
        let _ = setup_mock();

        let provider = Provider {
            name: "TestProvider5".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_5".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::remove_var("TEST_API_KEY_5");
        std::env::remove_var("NEXUM_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");

        let res = resolve_key(&provider);
        assert_eq!(res, KeyResolution::Missing);
        assert_eq!(res.into_key(), None);
    }

    #[test]
    fn keychain_unavailable_falls_back_to_env() {
        let _ = setup_mock();
        let entry = keyring_core::Entry::new("nexum-tui", "TestProvider6").unwrap();
        let cred: &Cred = entry.as_any().downcast_ref().unwrap();
        // Simulate keychain unavailable (PlatformFailure)
        cred.set_error(Error::PlatformFailure("mock unavailable".into()));

        let provider = Provider {
            name: "TestProvider6".into(),
            base_url: "http://localhost".into(),
            requires_api_key: true,
            api_key_env: Some("TEST_API_KEY_6".into()),
            default_model: None,
            is_cloud: true,
        };
        std::env::set_var("TEST_API_KEY_6", "env-key");

        let res = resolve_key(&provider);
        match res {
            KeyResolution::Env { key, keychain_available } => {
                assert_eq!(key, "env-key");
                assert!(!keychain_available);
            }
            _ => panic!("expected Env with keychain_available=false, got {:?}", res),
        }
    }
}
