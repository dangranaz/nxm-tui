//! `OsKeychainStore` — wraps `keyring::Entry` behind the [`KeyStore`] seam.
//!
//! This is the OS-native backend (Apple keychain / Secret Service / Windows
//! credential manager via keyring 4). It may prompt the user. The service name
//! is fixed to `"nexum-tui"` to match the historical `keys.rs` behaviour.
//!
//! keyring error mapping (keyring 4):
//! - `NoEntry` → `Ok(None)` (reachable, nothing stored)
//! - `NoDefaultStore` / `PlatformFailure` / `NoStorageAccess` →
//!   `KeyStoreError::Unavailable` (fall back)
//! - anything else (`#[non_exhaustive]`) → `KeyStoreError::Backend`

use keyring::Entry;
use tracing::{debug, warn};
use zeroize::Zeroizing;

use super::{KeyStore, KeyStoreError};

/// The keychain service name shared by every provider entry.
const SERVICE: &str = "nexum-tui";

/// OS-native keychain backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsKeychainStore;

impl OsKeychainStore {
    /// Create a new OS keychain store.
    pub fn new() -> Self {
        OsKeychainStore
    }

    fn entry(provider: &str) -> Result<Entry, KeyStoreError> {
        Entry::new(SERVICE, provider).map_err(map_err)
    }
}

/// Map a keyring error to a [`KeyStoreError`]. `NoEntry` is handled by callers
/// (it is not an error), so it maps to `Backend` here as a defensive default.
fn map_err(e: keyring::Error) -> KeyStoreError {
    match e {
        keyring::Error::NoDefaultStore
        | keyring::Error::PlatformFailure(_)
        | keyring::Error::NoStorageAccess(_) => KeyStoreError::Unavailable(e.to_string()),
        other => KeyStoreError::Backend(other.to_string()),
    }
}

impl KeyStore for OsKeychainStore {
    fn get(&self, provider: &str) -> Result<Option<Zeroizing<String>>, KeyStoreError> {
        let entry = Self::entry(provider)?;
        match entry.get_password() {
            Ok(secret) => {
                debug!(target: "nexum::keystore", %provider, kind = "os_keychain", "key resolved");
                Ok(Some(Zeroizing::new(secret)))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn set(&self, provider: &str, secret: &str) -> Result<(), KeyStoreError> {
        let entry = Self::entry(provider)?;
        entry.set_password(secret).map_err(map_err)?;
        debug!(target: "nexum::keystore", %provider, kind = "os_keychain", "key stored");
        Ok(())
    }

    fn delete(&self, provider: &str) -> Result<(), KeyStoreError> {
        let entry = Self::entry(provider)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }

    fn is_available(&self) -> bool {
        // Probe cheaply: creating an entry fails on NoDefaultStore. A full
        // get() would prompt; entry creation does not.
        match Entry::new(SERVICE, "__availability_probe__") {
            Ok(_) => true,
            Err(e) => {
                warn!(target: "nexum::keystore", error = %e, kind = "os_keychain", "keychain unavailable");
                false
            }
        }
    }

    fn kind(&self) -> &'static str {
        "os_keychain"
    }
}
