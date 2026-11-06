//! Tests for the `keystore` seam (I1).
//!
//! Three groups:
//! 1. `MockKeyStore` — an in-memory fake proving the trait contract.
//! 2. `EncryptedFileStore` — a real set/get/delete roundtrip on a tempdir,
//!    plus wrong-passphrase and tamper-detection cases.
//! 3. `resolve_key_with` resolution order — this REPLACES the `#[ignore]`d
//!    `keychain_beats_env` test in `tests/keys.rs`: with an injectable store
//!    the "store beats env" assertion is now deterministic and un-ignored.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;

use nxm_tui::keys::{resolve_key_with, KeyResolution};
use nxm_tui::keystore::{EncryptedFileStore, EnvStore, KeyStore, KeyStoreError};
use nxm_tui::provider::Provider;
use zeroize::Zeroizing;

/// `resolve_key_with` + the env fallbacks read process-global env state, so the
/// resolution-order tests must not run concurrently.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// A simple in-memory fake. `available`/`fail` are tunable so a single type
/// exercises the reachable, unreachable, and error branches.
struct MockKeyStore {
    map: RefCell<BTreeMap<String, String>>,
    available: bool,
    fail: bool,
}

impl MockKeyStore {
    fn new() -> Self {
        MockKeyStore {
            map: RefCell::new(BTreeMap::new()),
            available: true,
            fail: false,
        }
    }

    fn with_entry(provider: &str, secret: &str) -> Self {
        let s = Self::new();
        s.map.borrow_mut().insert(provider.into(), secret.into());
        s
    }
}

impl KeyStore for MockKeyStore {
    fn get(&self, provider: &str) -> Result<Option<Zeroizing<String>>, KeyStoreError> {
        if self.fail {
            return Err(KeyStoreError::Backend("mock forced failure".into()));
        }
        Ok(self
            .map
            .borrow()
            .get(provider)
            .map(|s| Zeroizing::new(s.clone())))
    }

    fn set(&self, provider: &str, secret: &str) -> Result<(), KeyStoreError> {
        self.map.borrow_mut().insert(provider.into(), secret.into());
        Ok(())
    }

    fn delete(&self, provider: &str) -> Result<(), KeyStoreError> {
        self.map.borrow_mut().remove(provider);
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn kind(&self) -> &'static str {
        "mock"
    }
}

fn provider(name: &str, env_var: &str) -> Provider {
    Provider {
        name: name.into(),
        base_url: "http://localhost".into(),
        requires_api_key: true,
        api_key_env: Some(env_var.into()),
        default_model: None,
        is_cloud: true,
    }
}

/// Unique path under the OS temp dir (project convention — no tempfile crate).
fn temp_keystore_path(tag: &str) -> std::path::PathBuf {
    let id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    std::env::temp_dir()
        .join(format!("nxm_tui_keystore_{tag}_{id}"))
        .join("keys.enc")
}

// ---- 1. MockKeyStore contract ----

#[test]
fn mock_roundtrip_set_get_delete() {
    let store = MockKeyStore::new();
    assert_eq!(store.get("P").unwrap(), None);
    store.set("P", "secret").unwrap();
    assert_eq!(
        store.get("P").unwrap().map(|z| (*z).clone()),
        Some("secret".to_string())
    );
    store.delete("P").unwrap();
    assert_eq!(store.get("P").unwrap(), None);
}

// ---- 2. EncryptedFileStore roundtrip + security cases ----

#[test]
fn encrypted_file_roundtrip() {
    let path = temp_keystore_path("roundtrip");
    let store = EncryptedFileStore::at(&path, "correct horse battery staple");

    // Empty before any write.
    assert_eq!(store.get("NVIDIA").unwrap(), None);

    store.set("NVIDIA", "nv-secret").unwrap();
    store.set("OpenAI", "oa-secret").unwrap();

    assert_eq!(
        store.get("NVIDIA").unwrap().map(|z| (*z).clone()),
        Some("nv-secret".to_string())
    );
    assert_eq!(
        store.get("OpenAI").unwrap().map(|z| (*z).clone()),
        Some("oa-secret".to_string())
    );

    // Overwrite.
    store.set("NVIDIA", "nv-secret-2").unwrap();
    assert_eq!(
        store.get("NVIDIA").unwrap().map(|z| (*z).clone()),
        Some("nv-secret-2".to_string())
    );

    // Delete one, the other survives.
    store.delete("NVIDIA").unwrap();
    assert_eq!(store.get("NVIDIA").unwrap(), None);
    assert_eq!(
        store.get("OpenAI").unwrap().map(|z| (*z).clone()),
        Some("oa-secret".to_string())
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn encrypted_file_persists_across_instances() {
    let path = temp_keystore_path("persist");
    {
        let store = EncryptedFileStore::at(&path, "pass-123");
        store.set("P", "persisted").unwrap();
    }
    // A fresh instance with the same passphrase reads it back.
    let store2 = EncryptedFileStore::at(&path, "pass-123");
    assert_eq!(
        store2.get("P").unwrap().map(|z| (*z).clone()),
        Some("persisted".to_string())
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn encrypted_file_wrong_passphrase_is_corrupt_not_leak() {
    let path = temp_keystore_path("wrongpass");
    {
        let store = EncryptedFileStore::at(&path, "right-pass");
        store.set("P", "top-secret").unwrap();
    }
    // Wrong passphrase → decrypt fails → Corrupt error, never the plaintext.
    let bad = EncryptedFileStore::at(&path, "WRONG-pass");
    match bad.get("P") {
        Err(KeyStoreError::Corrupt(_)) => {}
        other => panic!("expected Corrupt, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn encrypted_file_tamper_is_detected() {
    let path = temp_keystore_path("tamper");
    {
        let store = EncryptedFileStore::at(&path, "pass");
        store.set("P", "secret").unwrap();
    }
    // Flip a byte in the ciphertext region (past the 33-byte header).
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();

    let store = EncryptedFileStore::at(&path, "pass");
    match store.get("P") {
        Err(KeyStoreError::Corrupt(_)) => {}
        other => panic!("expected Corrupt (AEAD tag mismatch), got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn encrypted_file_missing_file_is_empty_not_error() {
    let path = temp_keystore_path("missing");
    let store = EncryptedFileStore::at(&path, "pass");
    assert_eq!(store.get("anything").unwrap(), None);
}

// ---- 3. resolve_key_with order — REPLACES the ignored keychain_beats_env ----

/// Store hit beats env: a secret present in the store wins over any env var.
/// This is the deterministic, un-ignored successor to `keychain_beats_env`.
#[test]
fn store_beats_env_when_present() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("TEST_STORE_1", "env-key");
    std::env::set_var("NEXUM_API_KEY", "global-key");
    let store = MockKeyStore::with_entry("StoreProvider1", "store-key");
    let p = provider("StoreProvider1", "TEST_STORE_1");

    let res = resolve_key_with(&p, &store);
    assert_eq!(res, KeyResolution::Keychain("store-key".into()));

    std::env::remove_var("TEST_STORE_1");
    std::env::remove_var("NEXUM_API_KEY");
}

/// Store reachable but empty → falls back to the provider-specific env var.
#[test]
fn store_empty_falls_back_to_provider_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("TEST_STORE_2", "env-key");
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let store = MockKeyStore::new(); // reachable, no entry
    let p = provider("StoreProvider2", "TEST_STORE_2");

    match resolve_key_with(&p, &store) {
        KeyResolution::Env {
            key,
            keychain_available,
        } => {
            assert_eq!(key, "env-key");
            assert!(keychain_available, "store was reachable");
        }
        other => panic!("expected Env, got {other:?}"),
    }
    std::env::remove_var("TEST_STORE_2");
}

/// Store unavailable → env fallback with keychain_available = false.
#[test]
fn store_unavailable_falls_back_to_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("TEST_STORE_3", "env-key");
    let store = MockKeyStore {
        map: RefCell::new(BTreeMap::new()),
        available: false,
        fail: false,
    };
    let p = provider("StoreProvider3", "TEST_STORE_3");

    match resolve_key_with(&p, &store) {
        KeyResolution::Env {
            key,
            keychain_available,
        } => {
            assert_eq!(key, "env-key");
            assert!(!keychain_available, "store was unavailable");
        }
        other => panic!("expected Env (unavailable), got {other:?}"),
    }
    std::env::remove_var("TEST_STORE_3");
}

/// Store error (reachable but get() fails) → env fallback, no panic/leak.
#[test]
fn store_error_falls_back_to_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("TEST_STORE_4", "env-key");
    let store = MockKeyStore {
        map: RefCell::new(BTreeMap::new()),
        available: true,
        fail: true,
    };
    let p = provider("StoreProvider4", "TEST_STORE_4");

    match resolve_key_with(&p, &store) {
        KeyResolution::Env {
            key,
            keychain_available,
        } => {
            assert_eq!(key, "env-key");
            assert!(!keychain_available);
        }
        other => panic!("expected Env (store error), got {other:?}"),
    }
    std::env::remove_var("TEST_STORE_4");
}

/// Nothing anywhere → Missing.
#[test]
fn missing_everywhere() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("TEST_STORE_5");
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let store = MockKeyStore::new();
    let p = provider("StoreProvider5", "TEST_STORE_5");

    assert_eq!(resolve_key_with(&p, &store), KeyResolution::Missing);
}

/// `EnvStore` as a real backend: resolves the global NEXUM_API_KEY.
#[test]
fn env_store_resolves_global() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("NEXUM_API_KEY", "global-nexum");
    let store = EnvStore::new();
    assert_eq!(
        store.get("anyprovider").unwrap().map(|z| (*z).clone()),
        Some("global-nexum".to_string())
    );
    std::env::remove_var("NEXUM_API_KEY");
}
