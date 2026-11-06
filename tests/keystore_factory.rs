//! Tests for the runtime keystore factory (I2 — wire KeyStore in production).
//!
//! `store_for(mode, passphrase)` is the single place that maps a
//! `KeySourceMode` to a concrete [`KeyStore`] backend. Two properties matter
//! for production:
//! 1. mode → backend kind is exactly the ticket mapping;
//! 2. `EncryptedFile` without a session passphrase is a CLEAN error
//!    (`KeyStoreError::Unavailable`), never a panic — the caller activates
//!    the masked prompt instead of spawning blocking work.
//!
//! Plus the keys-level helpers added for the wiring: `needs_passphrase`
//! (gate) and `resolve_key_via` (factory + `resolve_key_with`, env fallback
//! when the factory fails).

use std::sync::Mutex;

use nxm_tui::keys::{needs_passphrase, resolve_key_via, KeyResolution, KeySourceMode};
use nxm_tui::keystore::{store_for, KeyStoreError};
use nxm_tui::provider::Provider;
use zeroize::Zeroizing;

/// `resolve_key_via` reads process-global env state — no parallel runs.
static ENV_LOCK: Mutex<()> = Mutex::new(());

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

// ---- 1. mode → backend mapping ----

#[test]
fn factory_maps_every_mode_to_its_backend_kind() {
    assert_eq!(
        store_for(KeySourceMode::EnvFirst, None)
            .expect("env store")
            .kind(),
        "env"
    );
    assert_eq!(
        store_for(KeySourceMode::KeychainFirst, None)
            .expect("keychain store")
            .kind(),
        "os_keychain"
    );
    assert_eq!(
        store_for(KeySourceMode::OsKeychain, None)
            .expect("os store")
            .kind(),
        "os_keychain"
    );
    let pass = Zeroizing::new("correct horse battery staple".to_string());
    assert_eq!(
        store_for(KeySourceMode::EncryptedFile, Some(&pass))
            .expect("encrypted store")
            .kind(),
        "encrypted_file"
    );
}

// ---- 2. EncryptedFile without passphrase = clean error, never a panic ----

#[test]
fn factory_encrypted_file_without_passphrase_is_clean_error() {
    match store_for(KeySourceMode::EncryptedFile, None) {
        Err(KeyStoreError::Unavailable(msg)) => {
            assert!(
                msg.contains("passphrase"),
                "message must name the cause: {msg}"
            )
        }
        Err(other) => panic!("expected Unavailable, got {other}"),
        Ok(_) => panic!("expected Err when the passphrase is missing"),
    }
}

#[test]
fn factory_encrypted_file_with_empty_passphrase_is_clean_error() {
    let empty = Zeroizing::new(String::new());
    match store_for(KeySourceMode::EncryptedFile, Some(&empty)) {
        Err(KeyStoreError::Unavailable(_)) => {}
        Err(other) => panic!("expected Unavailable, got {other}"),
        Ok(_) => panic!("expected Err for an empty passphrase"),
    }
}

// ---- 3. needs_passphrase gate ----

#[test]
fn passphrase_is_required_only_for_encrypted_file() {
    assert!(needs_passphrase(KeySourceMode::EncryptedFile));
    assert!(!needs_passphrase(KeySourceMode::EnvFirst));
    assert!(!needs_passphrase(KeySourceMode::KeychainFirst));
    assert!(!needs_passphrase(KeySourceMode::OsKeychain));
}

// ---- 4. resolve_key_via = factory + resolve_key_with ----

/// Happy path: `EnvFirst` resolves through the seam from the env layers
/// without ever touching the OS keychain.
#[test]
fn resolve_key_via_env_first_resolves_from_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("FACTORY_ENV_1", "env-key");
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let p = provider("FactoryProvider1", "FACTORY_ENV_1");

    match resolve_key_via(&p, KeySourceMode::EnvFirst, None) {
        KeyResolution::Env { key, .. } => assert_eq!(key, "env-key"),
        other => panic!("expected Env, got {other:?}"),
    }
    std::env::remove_var("FACTORY_ENV_1");
}

/// Factory failure (EncryptedFile, no passphrase) is store-unavailable:
/// clean env fallback, no panic, no prompt inside the resolver itself.
#[test]
fn resolve_key_via_without_passphrase_falls_back_to_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("FACTORY_ENV_2", "env-key");
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let p = provider("FactoryProvider2", "FACTORY_ENV_2");

    match resolve_key_via(&p, KeySourceMode::EncryptedFile, None) {
        KeyResolution::Env {
            key,
            keychain_available,
        } => {
            assert_eq!(key, "env-key");
            assert!(!keychain_available, "factory failed → store unreachable");
        }
        other => panic!("expected Env, got {other:?}"),
    }
    std::env::remove_var("FACTORY_ENV_2");
}
