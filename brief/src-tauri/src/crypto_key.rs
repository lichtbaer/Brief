//! Load or create the SQLCipher key: prefer OS keychain ([`keyring`]), fallback to app data file.

use rand::RngCore;
use std::path::Path;

const KEYRING_SERVICE: &str = "com.ubuntu.brief";
const KEYRING_USER: &str = "sqlcipher_db_key";
const FALLBACK_FILENAME: &str = ".brief_encryption_key";

/// Returns a 64-character hex key (256-bit material as hex string for PRAGMA key).
pub fn get_or_create_encryption_key(app_data_dir: &Path) -> Result<String, String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(k) = entry.get_password() {
            if !k.is_empty() {
                return Ok(k);
            }
        }
    }

    let fallback = app_data_dir.join(FALLBACK_FILENAME);
    if fallback.exists() {
        let contents = std::fs::read_to_string(&fallback).map_err(|e| e.to_string())?;
        let trimmed = contents.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let key = generate_secure_key()?;

    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if entry.set_password(&key).is_ok() {
            return Ok(key);
        }
    }

    std::fs::write(&fallback, &key).map_err(|e| e.to_string())?;
    Ok(key)
}

fn generate_secure_key() -> Result<String, String> {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    Ok(hex::encode(buf))
}
