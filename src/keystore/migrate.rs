//! One-shot key migration between two [`KeyStore`] backends (I3).
//!
//! `/provider migrate-keys` imports every provider key from a *source* backend
//! (the OS-native keychain) into a *destination* backend (the portable
//! encrypted file), so a machine move does not strand the keys in the OS
//! keychain. The logic here is backend-agnostic — it takes two `&dyn KeyStore`
//! — so it is exercised in tests with in-memory fakes, no real keychain.
//!
//! Properties:
//! - **Idempotent.** Re-running copies the same values again; `set` overwrites,
//!   so no duplication and no error on a second pass.
//! - **mai-log RIGID (D7).** The secret value is NEVER logged. We emit only the
//!   provider name, the per-provider outcome, and the final counts.
//! - **Fail-soft per provider.** A read/write error on one provider is recorded
//!   and migration continues; the report carries the error count.

use tracing::{info, warn};

use super::KeyStore;

/// Outcome of a migration pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrateReport {
    /// Providers whose key was found in the source and written to the dest.
    pub migrated: usize,
    /// Providers with no key in the source (nothing to do).
    pub absent: usize,
    /// Providers that errored on read or write (migration continued).
    pub errors: usize,
}

impl MigrateReport {
    /// A short, value-free status line for the TUI.
    pub fn status_line(&self) -> String {
        if self.errors == 0 {
            format!("{} chiavi migrate", self.migrated)
        } else {
            format!(
                "{} chiavi migrate ({} errori, {} assenti)",
                self.migrated, self.errors, self.absent
            )
        }
    }
}

/// Copy each provider's key from `source` to `dest`.
///
/// For every name in `providers`: `source.get` → if `Some`, `dest.set`. A
/// `None` is counted as `absent`; any `Err` on either side is counted as an
/// error and does not abort the pass. The secret is held only as long as the
/// `set` call and is never logged.
pub fn migrate_keys(
    source: &dyn KeyStore,
    dest: &dyn KeyStore,
    providers: &[String],
) -> MigrateReport {
    let mut report = MigrateReport::default();

    for name in providers {
        match source.get(name) {
            Ok(Some(secret)) => match dest.set(name, &secret) {
                Ok(()) => {
                    report.migrated += 1;
                    info!(
                        target: "nexum::keystore::migrate",
                        provider = %name,
                        from = source.kind(),
                        to = dest.kind(),
                        "key migrated"
                    );
                }
                Err(e) => {
                    report.errors += 1;
                    warn!(
                        target: "nexum::keystore::migrate",
                        provider = %name,
                        to = dest.kind(),
                        error = %e,
                        "migrate: destination write failed"
                    );
                }
            },
            Ok(None) => {
                report.absent += 1;
            }
            Err(e) => {
                report.errors += 1;
                warn!(
                    target: "nexum::keystore::migrate",
                    provider = %name,
                    from = source.kind(),
                    error = %e,
                    "migrate: source read failed"
                );
            }
        }
    }

    info!(
        target: "nexum::keystore::migrate",
        migrated = report.migrated,
        absent = report.absent,
        errors = report.errors,
        "migration pass complete"
    );
    report
}
