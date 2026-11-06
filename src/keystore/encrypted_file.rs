//! `EncryptedFileStore` — portable, OS-independent encrypted key file.
//!
//! The whole provider→secret map is serialized to JSON and encrypted as one
//! blob with **AES-256-GCM** (AEAD). The 256-bit key is derived from a
//! user passphrase with **Argon2id** and a per-file random salt. Secrets and
//! the derived key live in `zeroize`-scrubbed buffers.
//!
//! On-disk layout (little-endian, single file):
//! ```text
//! offset  size  field
//! 0       4     MAGIC  = b"NXMK"
//! 4       1     VERSION = 1
//! 5       16    salt   (Argon2id)
//! 21      12    nonce  (AES-GCM, unique per write)
//! 33      ..    ciphertext || GCM tag
//! ```
//! The file is written `0600` (unix) via an atomic temp-file + rename, so a
//! crash never leaves a half-written keystore.
//!
//! mai-log (D7): neither the passphrase, the derived key, nor any secret is
//! ever logged — only the provider name and the backend kind.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use rand::RngExt;
use tracing::debug;
use zeroize::{Zeroize, Zeroizing};

use super::{KeyStore, KeyStoreError};

const MAGIC: &[u8; 4] = b"NXMK";
const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 33

/// A portable AES-256-GCM encrypted key file, unlocked by a passphrase.
///
/// Construct with [`EncryptedFileStore::new`] (default path) or
/// [`EncryptedFileStore::at`] (explicit path, used by tests). The passphrase
/// is held in a `Zeroizing` buffer for the lifetime of the store and scrubbed
/// on drop.
pub struct EncryptedFileStore {
    path: PathBuf,
    passphrase: Zeroizing<String>,
}

impl EncryptedFileStore {
    /// Default keystore path: `dirs::config_dir()/nxm-tui/keys.enc`.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("nxm-tui").join("keys.enc"))
    }

    /// Build a store at the default path with the given passphrase.
    pub fn new(passphrase: impl Into<String>) -> Result<Self, KeyStoreError> {
        let path = Self::default_path().ok_or_else(|| {
            KeyStoreError::Unavailable("no config dir for encrypted keystore".into())
        })?;
        Ok(Self::at(path, passphrase))
    }

    /// Build a store at an explicit path (tests use a tempdir).
    pub fn at(path: impl Into<PathBuf>, passphrase: impl Into<String>) -> Self {
        EncryptedFileStore {
            path: path.into(),
            passphrase: Zeroizing::new(passphrase.into()),
        }
    }

    /// Derive the 256-bit AES key from the passphrase + salt via Argon2id.
    fn derive_key(&self, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, KeyStoreError> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::default()
            .hash_password_into(self.passphrase.as_bytes(), salt, key.as_mut())
            .map_err(|e| KeyStoreError::Backend(format!("argon2: {e}")))?;
        Ok(key)
    }

    /// Read + decrypt the full map. A missing file is an empty map.
    fn load(&self) -> Result<BTreeMap<String, String>, KeyStoreError> {
        let raw = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(KeyStoreError::Unavailable(format!("read {e}"))),
        };
        if raw.len() < HEADER_LEN {
            return Err(KeyStoreError::Corrupt("file shorter than header".into()));
        }
        if &raw[0..4] != MAGIC {
            return Err(KeyStoreError::Corrupt("bad magic".into()));
        }
        if raw[4] != VERSION {
            return Err(KeyStoreError::Corrupt(format!(
                "unsupported version {}",
                raw[4]
            )));
        }
        let salt = &raw[5..5 + SALT_LEN];
        let nonce = &raw[5 + SALT_LEN..HEADER_LEN];
        let ciphertext = &raw[HEADER_LEN..];

        let key = self.derive_key(salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|e| KeyStoreError::Backend(format!("aes init: {e}")))?;
        let nonce = Nonce::try_from(nonce)
            .map_err(|_| KeyStoreError::Corrupt("bad nonce length".into()))?;
        let mut plaintext = cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| KeyStoreError::Corrupt("decrypt failed (wrong passphrase?)".into()))?;

        let map: BTreeMap<String, String> = serde_json::from_slice(&plaintext)
            .map_err(|e| KeyStoreError::Corrupt(format!("json: {e}")))?;
        plaintext.zeroize();
        Ok(map)
    }

    /// Encrypt + atomically write the full map (fresh salt + nonce per write).
    fn store(&self, map: &BTreeMap<String, String>) -> Result<(), KeyStoreError> {
        let mut rng = rand::rng();
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill(&mut salt);
        rng.fill(&mut nonce);

        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|e| KeyStoreError::Backend(format!("aes init: {e}")))?;

        let mut plaintext =
            serde_json::to_vec(map).map_err(|e| KeyStoreError::Backend(format!("json: {e}")))?;
        let nonce_arr = Nonce::try_from(&nonce[..])
            .map_err(|e| KeyStoreError::Backend(format!("nonce: {e}")))?;
        let ciphertext = cipher
            .encrypt(&nonce_arr, plaintext.as_slice())
            .map_err(|e| KeyStoreError::Backend(format!("encrypt: {e}")))?;
        plaintext.zeroize();

        let mut buf = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        buf.extend_from_slice(MAGIC);
        buf.push(VERSION);
        buf.extend_from_slice(&salt);
        buf.extend_from_slice(&nonce);
        buf.extend_from_slice(&ciphertext);

        self.atomic_write(&buf)
    }

    /// Write `buf` to a sibling temp file (0600) then rename over the target.
    fn atomic_write(&self, buf: &[u8]) -> Result<(), KeyStoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| KeyStoreError::Unavailable(format!("mkdir {e}")))?;
        }
        let tmp = self.tmp_path();
        {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts
                .open(&tmp)
                .map_err(|e| KeyStoreError::Unavailable(format!("open tmp {e}")))?;
            f.write_all(buf)
                .map_err(|e| KeyStoreError::Unavailable(format!("write tmp {e}")))?;
            f.sync_all()
                .map_err(|e| KeyStoreError::Unavailable(format!("sync tmp {e}")))?;
        }
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| KeyStoreError::Unavailable(format!("rename {e}")))?;
        Ok(())
    }

    fn tmp_path(&self) -> PathBuf {
        let mut os = self.path.clone().into_os_string();
        os.push(".tmp");
        PathBuf::from(os)
    }

    fn exists(path: &Path) -> bool {
        path.exists()
    }
}

impl KeyStore for EncryptedFileStore {
    fn get(&self, provider: &str) -> Result<Option<Zeroizing<String>>, KeyStoreError> {
        let map = self.load()?;
        match map.get(provider) {
            Some(secret) => {
                debug!(target: "nexum::keystore", %provider, kind = "encrypted_file", "key resolved");
                Ok(Some(Zeroizing::new(secret.clone())))
            }
            None => Ok(None),
        }
    }

    fn set(&self, provider: &str, secret: &str) -> Result<(), KeyStoreError> {
        let mut map = self.load()?;
        map.insert(provider.to_string(), secret.to_string());
        self.store(&map)?;
        debug!(target: "nexum::keystore", %provider, kind = "encrypted_file", "key stored");
        Ok(())
    }

    fn delete(&self, provider: &str) -> Result<(), KeyStoreError> {
        let mut map = self.load()?;
        if map.remove(provider).is_some() {
            self.store(&map)?;
            debug!(target: "nexum::keystore", %provider, kind = "encrypted_file", "key deleted");
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        // Reachable if the file does not exist yet (first write will create it)
        // or exists and is readable. A non-empty passphrase is required.
        !self.passphrase.is_empty()
            && (!Self::exists(&self.path)
                || std::fs::metadata(&self.path)
                    .map(|m| m.is_file())
                    .unwrap_or(false))
    }

    fn kind(&self) -> &'static str {
        "encrypted_file"
    }
}
