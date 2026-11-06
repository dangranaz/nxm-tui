//! Tests for `keystore::migrate_keys` (I3): one-shot key migration between two
//! backends, exercised with in-memory fakes (no real OS keychain).
//!
//! Covers the properties the TUI relies on: exact counts, idempotency,
//! absent-provider accounting, and fail-soft on per-provider read/write errors.
//! All tests are pure (no process-global env), so none need serialization.

use std::cell::RefCell;
use std::collections::BTreeMap;

use nxm_tui::keystore::{migrate_keys, KeyStore, KeyStoreError, MigrateReport};
use zeroize::Zeroizing;

/// In-memory fake. `fail` forces every `get`/`set` to error, for the
/// fail-soft paths; otherwise it is a plain map-backed store.
struct MockKeyStore {
    map: RefCell<BTreeMap<String, String>>,
    fail: bool,
}

impl MockKeyStore {
    fn new() -> Self {
        MockKeyStore {
            map: RefCell::new(BTreeMap::new()),
            fail: false,
        }
    }

    fn failing() -> Self {
        MockKeyStore {
            map: RefCell::new(BTreeMap::new()),
            fail: true,
        }
    }

    fn seed(self, entries: &[(&str, &str)]) -> Self {
        for (k, v) in entries {
            self.map.borrow_mut().insert((*k).into(), (*v).into());
        }
        self
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
        if self.fail {
            return Err(KeyStoreError::Backend("mock forced failure".into()));
        }
        self.map.borrow_mut().insert(provider.into(), secret.into());
        Ok(())
    }

    fn delete(&self, provider: &str) -> Result<(), KeyStoreError> {
        self.map.borrow_mut().remove(provider);
        Ok(())
    }

    fn is_available(&self) -> bool {
        !self.fail
    }

    fn kind(&self) -> &'static str {
        "mock"
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn got(store: &MockKeyStore, provider: &str) -> Option<String> {
    store.get(provider).unwrap().map(|z| (*z).clone())
}

/// Happy path: present keys are copied to the dest; values match, counts exact.
#[test]
fn migrate_copies_present_keys_to_dest() {
    let source = MockKeyStore::new().seed(&[("NVIDIA", "nv-secret"), ("OpenAI", "oa-secret")]);
    let dest = MockKeyStore::new();

    let report = migrate_keys(&source, &dest, &names(&["NVIDIA", "OpenAI"]));

    assert_eq!(
        report,
        MigrateReport {
            migrated: 2,
            absent: 0,
            errors: 0
        }
    );
    assert_eq!(got(&dest, "NVIDIA"), Some("nv-secret".into()));
    assert_eq!(got(&dest, "OpenAI"), Some("oa-secret".into()));
}

/// Providers absent in the source are counted as `absent`, not migrated/error.
#[test]
fn migrate_counts_absent_providers() {
    let source = MockKeyStore::new().seed(&[("NVIDIA", "nv-secret")]);
    let dest = MockKeyStore::new();

    let report = migrate_keys(&source, &dest, &names(&["NVIDIA", "Anthropic", "Groq"]));

    assert_eq!(
        report,
        MigrateReport {
            migrated: 1,
            absent: 2,
            errors: 0
        }
    );
    assert_eq!(got(&dest, "Anthropic"), None);
}

/// Idempotent: a second pass yields identical counts and no duplication.
#[test]
fn migrate_is_idempotent() {
    let source = MockKeyStore::new().seed(&[("NVIDIA", "nv-secret"), ("OpenAI", "oa-secret")]);
    let dest = MockKeyStore::new();
    let want = names(&["NVIDIA", "OpenAI"]);

    let first = migrate_keys(&source, &dest, &want);
    let second = migrate_keys(&source, &dest, &want);

    assert_eq!(first, second);
    assert_eq!(
        second,
        MigrateReport {
            migrated: 2,
            absent: 0,
            errors: 0
        }
    );
    assert_eq!(got(&dest, "NVIDIA"), Some("nv-secret".into()));
    assert_eq!(got(&dest, "OpenAI"), Some("oa-secret".into()));
}

/// A source read error is counted and does not abort the pass.
#[test]
fn migrate_source_error_is_counted_and_continues() {
    let source = MockKeyStore::failing();
    let dest = MockKeyStore::new();

    let report = migrate_keys(&source, &dest, &names(&["NVIDIA", "OpenAI"]));
    assert_eq!(
        report,
        MigrateReport {
            migrated: 0,
            absent: 0,
            errors: 2
        }
    );
}

/// A destination write error is counted; migration continues to the next.
#[test]
fn migrate_dest_error_is_counted_and_continues() {
    let source = MockKeyStore::new().seed(&[("NVIDIA", "nv-secret"), ("OpenAI", "oa-secret")]);
    let dest = MockKeyStore::failing();

    let report = migrate_keys(&source, &dest, &names(&["NVIDIA", "OpenAI"]));
    assert_eq!(
        report,
        MigrateReport {
            migrated: 0,
            absent: 0,
            errors: 2
        }
    );
}

/// An empty provider list is a clean no-op.
#[test]
fn migrate_empty_provider_list_is_noop() {
    let source = MockKeyStore::new();
    let dest = MockKeyStore::new();
    assert_eq!(migrate_keys(&source, &dest, &[]), MigrateReport::default());
}

/// Status line: value-free, reports the migrated count; surfaces errors.
#[test]
fn migrate_report_status_line() {
    assert_eq!(
        MigrateReport {
            migrated: 3,
            absent: 0,
            errors: 0
        }
        .status_line(),
        "3 chiavi migrate"
    );
    assert_eq!(
        MigrateReport {
            migrated: 1,
            absent: 2,
            errors: 1
        }
        .status_line(),
        "1 chiavi migrate (1 errori, 2 assenti)"
    );
}
