//! Tests for `keys`: key-source mode selection (pure) and the env-only
//! resolution path through the [`KeyStore`] seam.
//!
//! The former keychain-backed cases that called the legacy process-global
//! `resolve_key` were removed with that wrapper (I3). Their logic now lives,
//! deterministic and un-ignored, in `tests/keystore.rs`
//! (`store_empty_falls_back_to_provider_env`,
//! `store_unavailable_falls_back_to_env`). The env-first path (keychain never
//! touched) is kept here, re-expressed over the injectable `EnvStore` backend
//! — no process-global mock keychain.

use nxm_tui::keys::{key_source_mode, resolve_key_with, KeyResolution, KeySourceMode};
use nxm_tui::keystore::EnvStore;
use nxm_tui::provider::Provider;
use std::sync::Mutex;

/// The env-fallback layers read process-global env state, so the resolution
/// test below must not run concurrently with other env-mutating tests.
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

// ---- key_source_mode (pure, no I/O) ----

#[test]
fn mode_env_present_defaults_to_env_first() {
    assert_eq!(key_source_mode(true, None), KeySourceMode::EnvFirst);
}

#[test]
fn mode_no_env_defaults_to_keychain_first() {
    assert_eq!(key_source_mode(false, None), KeySourceMode::KeychainFirst);
}

#[test]
fn mode_explicit_env_override_wins() {
    assert_eq!(key_source_mode(false, Some("env")), KeySourceMode::EnvFirst);
    assert_eq!(key_source_mode(false, Some("ENV")), KeySourceMode::EnvFirst);
}

#[test]
fn mode_explicit_keychain_override_wins() {
    assert_eq!(
        key_source_mode(true, Some("keychain")),
        KeySourceMode::KeychainFirst
    );
}

#[test]
fn mode_auto_or_unknown_falls_back_to_env_presence() {
    assert_eq!(key_source_mode(true, Some("auto")), KeySourceMode::EnvFirst);
    assert_eq!(
        key_source_mode(false, Some("garbage")),
        KeySourceMode::KeychainFirst
    );
}

// ---- env-first resolution over the EnvStore backend ----
//
// In `EnvFirst` the runtime factory (`store_for`) hands back an `EnvStore`,
// whose `get` always returns `Ok(None)` so resolution falls through to the env
// layers without ever touching a keychain. This is the seam-based successor to
// the old `env_first_resolves_from_provider_env_without_keychain`.

#[test]
fn env_store_resolves_from_provider_env() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Clear the globals EnvStore::get would otherwise return first, so the
    // resolution reaches the provider-specific var in `resolve_env_fallback`.
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::set_var("TEST_ENVFIRST_1", "env-key");
    let p = provider("EnvFirstProvider1", "TEST_ENVFIRST_1");

    match resolve_key_with(&p, &EnvStore::new()) {
        KeyResolution::Env {
            key,
            keychain_available,
        } => {
            assert_eq!(key, "env-key");
            // EnvStore is always "reachable": it was not found *unavailable*,
            // we simply never consult a keychain. Reported true.
            assert!(keychain_available);
        }
        other => panic!("expected Env, got {other:?}"),
    }
    std::env::remove_var("TEST_ENVFIRST_1");
}

#[test]
fn env_store_missing_when_no_env_set() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("TEST_ENVFIRST_2");
    std::env::remove_var("NEXUM_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let p = provider("EnvFirstProvider2", "TEST_ENVFIRST_2");

    assert_eq!(
        resolve_key_with(&p, &EnvStore::new()),
        KeyResolution::Missing
    );
}
