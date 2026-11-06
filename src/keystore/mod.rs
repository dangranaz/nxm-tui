//! Injectable secret storage seam (I1, from study R5).
//!
//! `resolve_key` historically called `keyring::Entry` directly, which made it
//! both OS-dependent and untestable (the production native store is not the
//! `keyring_core` mock the tests install). This module introduces an
//! application-level [`KeyStore`] trait so key resolution can be driven by any
//! backend — the native OS keychain, a portable encrypted file, env-only, or
//! an in-memory fake in tests.
//!
//! Backends:
//! - [`EnvStore`] — level 0, reads from the environment only (no persistence).
//! - [`OsKeychainStore`] — wraps `keyring::Entry` (OS-native, may prompt).
//! - [`EncryptedFileStore`] — portable AES-256-GCM + Argon2id file (`keys.enc`).
//!
//! [`store_for`] is the runtime factory mapping `KeySourceMode` to one of
//! these backends (I2 — production wiring).
//!
//! mai-log RIGID (D7): a secret value is NEVER logged or printed. Logs emit
//! only the provider name, presence, and the backend kind.

mod encrypted_file;
mod env;
mod migrate;
mod os_keychain;

pub use encrypted_file::EncryptedFileStore;
pub use env::EnvStore;
pub use migrate::{migrate_keys, MigrateReport};
pub use os_keychain::OsKeychainStore;

use zeroize::Zeroizing;

use crate::keys::KeySourceMode;

/// Build the backend for `mode` — the runtime factory behind the I2 wiring.
///
/// Mapping (from `KeySourceMode` via `key_source_mode()`): `EnvFirst` →
/// [`EnvStore`], `KeychainFirst`/`OsKeychain` → [`OsKeychainStore`],
/// `EncryptedFile` → [`EncryptedFileStore`].
///
/// `EncryptedFile` needs the session passphrase; a missing or empty one is a
/// clean [`KeyStoreError::Unavailable`] (never a panic) so the caller can
/// open the masked prompt instead of spawning blocking work.
///
/// # Examples
///
/// ```
/// use nxm_tui::keys::KeySourceMode;
/// use nxm_tui::keystore::store_for;
///
/// let store = store_for(KeySourceMode::EnvFirst, None).expect("env store");
/// assert_eq!(store.kind(), "env");
/// // Missing passphrase = clean error, not a panic.
/// assert!(store_for(KeySourceMode::EncryptedFile, None).is_err());
/// ```
pub fn store_for(
    mode: KeySourceMode,
    passphrase: Option<&Zeroizing<String>>,
) -> Result<Box<dyn KeyStore>, KeyStoreError> {
    match mode {
        KeySourceMode::EnvFirst => Ok(Box::new(EnvStore::new())),
        KeySourceMode::KeychainFirst | KeySourceMode::OsKeychain => {
            Ok(Box::new(OsKeychainStore::new()))
        }
        KeySourceMode::EncryptedFile => {
            let pass = passphrase.filter(|p| !p.is_empty()).ok_or_else(|| {
                KeyStoreError::Unavailable("encrypted keystore requires a passphrase".into())
            })?;
            // `&str → String` is moved straight into the store's `Zeroizing`
            // buffer: no un-zeroized copy of the passphrase survives.
            Ok(Box::new(EncryptedFileStore::new(pass.as_str())?))
        }
    }
}

/// Errors a [`KeyStore`] backend can surface.
///
/// `Unavailable` means the backend itself could not be reached (no keychain,
/// locked file, missing passphrase) — the caller should fall back to the next
/// source. It is distinct from a successful lookup that simply found nothing
/// (represented as `Ok(None)`).
#[derive(Debug)]
pub enum KeyStoreError {
    /// The backend is not reachable (no store, locked, no passphrase, I/O).
    Unavailable(String),

    /// The stored data is present but could not be decrypted or parsed
    /// (wrong passphrase, corrupt/truncated file, bad header).
    Corrupt(String),

    /// An underlying backend error that is neither "unavailable" nor "corrupt".
    Backend(String),
}

impl std::fmt::Display for KeyStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyStoreError::Unavailable(m) => write!(f, "keystore backend unavailable: {m}"),
            KeyStoreError::Corrupt(m) => write!(f, "keystore data corrupt or undecryptable: {m}"),
            KeyStoreError::Backend(m) => write!(f, "keystore backend error: {m}"),
        }
    }
}

impl std::error::Error for KeyStoreError {}

/// A pluggable secret store keyed by provider name.
///
/// Implementations must never log or print secret values (D7 mai-log). The
/// returned secret is wrapped in [`Zeroizing`] so it is scrubbed from memory
/// on drop.
pub trait KeyStore {
    /// Fetch the secret for `provider`.
    ///
    /// - `Ok(Some(secret))` — found.
    /// - `Ok(None)`         — backend reachable, no entry for this provider.
    /// - `Err(_)`           — backend unavailable/corrupt (caller falls back).
    fn get(&self, provider: &str) -> Result<Option<Zeroizing<String>>, KeyStoreError>;

    /// Store (or overwrite) the secret for `provider`.
    fn set(&self, provider: &str, secret: &str) -> Result<(), KeyStoreError>;

    /// Delete the secret for `provider`. Deleting a missing entry is `Ok(())`.
    fn delete(&self, provider: &str) -> Result<(), KeyStoreError>;

    /// Whether the backend is reachable right now. Used to decide fallback
    /// before attempting a `get` (e.g. headless/CI with no keychain).
    fn is_available(&self) -> bool;

    /// A short, stable label for logging/status (never the value).
    fn kind(&self) -> &'static str;
}
