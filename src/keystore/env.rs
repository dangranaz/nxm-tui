//! `EnvStore` — level-0 backend that reads secrets from the environment only.
//!
//! This store is read-only persistence-wise: `set`/`delete` are no-ops that
//! report `Unavailable`, because the environment is not something we write to
//! disk. It exists so the dev/CI path (`NXM_KEY_SOURCE=env`) goes through the
//! same [`KeyStore`] seam as every other backend.
//!
//! The provider-specific `api_key_env` lookup lives in `keys.rs` (it needs the
//! full `Provider`); this store covers the global `NEXUM_API_KEY` /
//! `OPENAI_API_KEY` fallbacks, keyed provider-agnostically.

use std::env;

use tracing::debug;
use zeroize::Zeroizing;

use super::{KeyStore, KeyStoreError};

/// Reads secrets from process environment variables. No persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvStore;

impl EnvStore {
    /// Create a new env-backed store.
    pub fn new() -> Self {
        EnvStore
    }
}

impl KeyStore for EnvStore {
    fn get(&self, provider: &str) -> Result<Option<Zeroizing<String>>, KeyStoreError> {
        for var in ["NEXUM_API_KEY", "OPENAI_API_KEY"] {
            if let Ok(val) = env::var(var) {
                if !val.is_empty() {
                    debug!(target: "nexum::keystore", %provider, env_var = var, kind = "env", "key resolved");
                    return Ok(Some(Zeroizing::new(val)));
                }
            }
        }
        Ok(None)
    }

    fn set(&self, _provider: &str, _secret: &str) -> Result<(), KeyStoreError> {
        Err(KeyStoreError::Unavailable(
            "EnvStore is read-only; cannot persist a secret to the environment".into(),
        ))
    }

    fn delete(&self, _provider: &str) -> Result<(), KeyStoreError> {
        Err(KeyStoreError::Unavailable(
            "EnvStore is read-only; cannot delete an environment variable".into(),
        ))
    }

    fn is_available(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "env"
    }
}
