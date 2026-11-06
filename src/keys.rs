//! Per-provider API key resolution via an injectable [`crate::keystore`]
//! backend + env fallback.
//!
//! Resolution order (D7 hard), generalized over the chosen backend
//! (see [`resolve_key_with`]):
//! 1. `store.get(provider.name)` — the selected backend (encrypted file, OS
//!    keychain, env, or a test fake)
//! 2. Provider's `api_key_env` (e.g. `NVIDIA_API_KEY`)
//! 3. Global `NEXUM_API_KEY`
//! 4. Global `OPENAI_API_KEY`
//! 5. Missing
//!
//! A store error or an unreachable backend is treated as "not found here" and
//! resolution falls through to the env layers.
//!
//! mai-log RIGID: key value NEVER logged/printed (overlay, debug log, test output).
//! Logs emit only: source, presence, env var name.
//!
//! The resolution functions live in [`resolve`] (300-line cap) and are
//! re-exported here, so `crate::keys::{resolve_key_with, resolve_key_via}`
//! paths stay stable.

use std::env;

mod resolve;

pub use resolve::{resolve_key_via, resolve_key_with};

/// Result of key resolution for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyResolution {
    /// Key found in keychain.
    Keychain(String),
    /// Key found in environment variable. `keychain_available` = whether the
    /// keychain was reachable at all (false on NoDefaultStore / PlatformFailure /
    /// NoStorageAccess).
    Env {
        key: String,
        keychain_available: bool,
    },
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
            KeyResolution::Env {
                key: _,
                keychain_available: _,
            } => None, // would need to track which env
            _ => None,
        }
    }
}

/// Which source to try first. In `EnvFirst` the OS keychain is never touched,
/// so no password prompt appears — the dev-friendly path.
///
/// `EncryptedFile` and `OsKeychain` select a concrete [`crate::keystore`]
/// backend for the generalized D7 order (see [`resolve_key_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySourceMode {
    /// Read keys from the environment only; never touch the keychain.
    EnvFirst,
    /// Try the keychain first, then env (production default, D7).
    KeychainFirst,
    /// Use the portable encrypted file store first, then env.
    EncryptedFile,
    /// Use the OS-native keychain store first, then env (same as
    /// `KeychainFirst` but routed through the `KeyStore` seam).
    OsKeychain,
}

/// Decide the key-source mode from an explicit override and whether a local
/// `.env` is present. Pure (no I/O) so it is unit-testable.
///
/// - `NXM_KEY_SOURCE=env`      → always [`KeySourceMode::EnvFirst`]
/// - `NXM_KEY_SOURCE=keychain` → always [`KeySourceMode::KeychainFirst`]
/// - `NXM_KEY_SOURCE=file`/`encrypted` → [`KeySourceMode::EncryptedFile`]
/// - `NXM_KEY_SOURCE=os`/`os-keychain` → [`KeySourceMode::OsKeychain`]
/// - `NXM_KEY_SOURCE=auto` or unset → `EnvFirst` iff a `.env` is present,
///   otherwise `KeychainFirst`.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::keys::{key_source_mode, KeySourceMode};
///
/// // A local .env (dev) skips the keychain — no password prompt.
/// assert_eq!(key_source_mode(true, None), KeySourceMode::EnvFirst);
/// // No .env, no override: production default.
/// assert_eq!(key_source_mode(false, None), KeySourceMode::KeychainFirst);
/// // Explicit override always wins.
/// assert_eq!(key_source_mode(false, Some("env")), KeySourceMode::EnvFirst);
/// assert_eq!(key_source_mode(true, Some("keychain")), KeySourceMode::KeychainFirst);
/// assert_eq!(key_source_mode(false, Some("file")), KeySourceMode::EncryptedFile);
/// ```
pub fn key_source_mode(env_present: bool, override_var: Option<&str>) -> KeySourceMode {
    match override_var.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("env") => KeySourceMode::EnvFirst,
        Some("keychain") => KeySourceMode::KeychainFirst,
        Some("file") | Some("encrypted") => KeySourceMode::EncryptedFile,
        Some("os") | Some("os-keychain") => KeySourceMode::OsKeychain,
        // "auto", unset, or anything else: infer from the .env presence.
        _ => {
            if env_present {
                KeySourceMode::EnvFirst
            } else {
                KeySourceMode::KeychainFirst
            }
        }
    }
}

/// Mode for the current process: explicit `NXM_KEY_SOURCE` override, else
/// inferred from the presence of a local `.env`. Environment inspection only.
///
/// # Examples
///
/// ```no_run
/// use nxm_tui::keys::{current_mode, KeySourceMode};
///
/// // Reads `NXM_KEY_SOURCE` + local `.env` presence, so the value is
/// // environment-dependent — this example compiles but does not run.
/// let mode = current_mode();
/// let _dev_box = mode == KeySourceMode::EnvFirst;
/// ```
pub fn current_mode() -> KeySourceMode {
    let env_present = std::path::Path::new(".env").exists();
    let override_var = env::var("NXM_KEY_SOURCE").ok();
    key_source_mode(env_present, override_var.as_deref())
}

/// Whether `mode` needs the session passphrase before any store work runs
/// (I2 gate). Only the portable encrypted file is passphrase-locked; the
/// callers prompt when this is true and no passphrase is cached yet.
///
/// # Examples
///
/// ```
/// use nxm_tui::keys::{needs_passphrase, KeySourceMode};
///
/// assert!(needs_passphrase(KeySourceMode::EncryptedFile));
/// assert!(!needs_passphrase(KeySourceMode::EnvFirst));
/// assert!(!needs_passphrase(KeySourceMode::KeychainFirst));
/// ```
pub fn needs_passphrase(mode: KeySourceMode) -> bool {
    mode == KeySourceMode::EncryptedFile
}
