//! Key resolution over the keychain/keystore backends + env fallback (D7).
//!
//! Split out of [`crate::keys`] to keep both files under the 300-line cap.
//! The public entry points are re-exported at `crate::keys::{resolve_key_with,
//! resolve_key_via}` — import them from there.
//!
//! mai-log RIGID: key value NEVER logged/printed. Logs emit only: source,
//! presence, env var name.

use std::env;

use tracing::{debug, info, warn};
use zeroize::Zeroizing;

use crate::keystore::KeyStore;
use crate::provider::Provider;

use super::{KeyResolution, KeySourceMode};

fn resolve_env_fallback(provider: &Provider, keychain_available: bool) -> KeyResolution {
    // Provider-specific env var
    if let Some(ref env_var) = provider.api_key_env {
        if let Ok(key) = env::var(env_var) {
            if !key.is_empty() {
                info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = %env_var, "key resolved");
                return KeyResolution::Env {
                    key,
                    keychain_available,
                };
            }
        }
    }

    // Global NEXUM_API_KEY
    if let Ok(key) = env::var("NEXUM_API_KEY") {
        if !key.is_empty() {
            info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = "NEXUM_API_KEY", "key resolved");
            return KeyResolution::Env {
                key,
                keychain_available,
            };
        }
    }

    // Global OPENAI_API_KEY
    if let Ok(key) = env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            info!(target: "nexum::keys", provider = %provider.name, source = "env", env_var = "OPENAI_API_KEY", "key resolved");
            return KeyResolution::Env {
                key,
                keychain_available,
            };
        }
    }

    info!(target: "nexum::keys", provider = %provider.name, source = "missing", "no key found");
    KeyResolution::Missing
}

/// Resolve the API key for a provider through an injectable [`KeyStore`] seam.
///
/// The backend-agnostic resolution path. The resolution order generalizes D7:
///
/// 1. `store.get(provider.name)` — the chosen backend (encrypted file, OS
///    keychain, env, or a test fake).
/// 2. `provider.api_key_env` — the provider-specific env var.
/// 3. `NEXUM_API_KEY` — global.
/// 4. `OPENAI_API_KEY` — global.
/// 5. Missing.
///
/// A store error (backend unavailable/corrupt) is treated as "not found here"
/// and resolution falls through to the env layers, mirroring the historical
/// keychain-unavailable behaviour. `keychain_available` in the returned
/// [`KeyResolution::Env`] reflects whether the store was reachable.
///
/// mai-log (D7): the key value is never logged — only source/presence.
pub fn resolve_key_with(provider: &Provider, store: &dyn KeyStore) -> KeyResolution {
    let store_available = store.is_available();
    if store_available {
        match store.get(&provider.name) {
            Ok(Some(secret)) => {
                info!(target: "nexum::keys", provider = %provider.name, source = store.kind(), "key resolved");
                // `Zeroizing<String>` → owned String for the resolution; the
                // caller holds it only as long as needed (Agent spawn).
                return KeyResolution::Keychain((*secret).clone());
            }
            Ok(None) => {
                debug!(target: "nexum::keys", provider = %provider.name, kind = store.kind(), "no store entry");
            }
            Err(e) => {
                warn!(target: "nexum::keys", provider = %provider.name, kind = store.kind(), error = %e, "store error");
                return resolve_env_fallback(provider, false);
            }
        }
    } else {
        warn!(target: "nexum::keys", provider = %provider.name, kind = store.kind(), "store unavailable");
        return resolve_env_fallback(provider, false);
    }

    // Store reachable but no entry → env fallback (keychain_available = true).
    resolve_env_fallback(provider, true)
}

/// Production read path (I2): resolve through the runtime factory
/// (`crate::keystore::store_for`) + [`resolve_key_with`].
///
/// A factory failure — e.g. `EncryptedFile` without the session passphrase —
/// is store-unavailable: a `warn` (mai-log: neither the key nor the
/// passphrase is ever in the error) and the D7 env fallback. Callers gate on
/// [`crate::keys::needs_passphrase`] *before* spawning, so the
/// missing-passphrase case normally never reaches this function.
///
/// # Examples
///
/// ```no_run
/// use nxm_tui::keys::{resolve_key_via, KeySourceMode};
/// use nxm_tui::provider::Provider;
///
/// # let provider = Provider { name: "P".into(), base_url: "http://x".into(),
/// #     requires_api_key: true, api_key_env: None, default_model: None, is_cloud: false };
/// // Reads process env + the chosen backend — logs the source, never the key.
/// let resolution = resolve_key_via(&provider, KeySourceMode::EnvFirst, None);
/// # let _ = resolution.is_some();
/// ```
pub fn resolve_key_via(
    provider: &Provider,
    mode: KeySourceMode,
    passphrase: Option<Zeroizing<String>>,
) -> KeyResolution {
    debug!(
        target: "nexum::keys",
        provider = %provider.name,
        mode = ?mode,
        "resolving key via keystore factory"
    );
    match crate::keystore::store_for(mode, passphrase.as_ref()) {
        Ok(store) => resolve_key_with(provider, store.as_ref()),
        Err(e) => {
            warn!(target: "nexum::keys", provider = %provider.name, mode = ?mode, error = %e, "keystore unavailable");
            resolve_env_fallback(provider, false)
        }
    }
}
