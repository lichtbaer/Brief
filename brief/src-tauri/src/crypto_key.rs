//! Load or create the SQLCipher key: prefer OS keychain ([`keyring`]), fallback to app data file.
//! The key is always persisted to **both** locations so that losing one does not cause data loss.

use rand::RngCore;
use std::path::Path;

/// Writes `content` to `path` with restrictive permissions (0600 on Unix).
fn write_key_file(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

const KEYRING_SERVICE: &str = "com.ubuntu.brief";
const KEYRING_USER: &str = "sqlcipher_db_key";
const FALLBACK_FILENAME: &str = ".brief_encryption_key";

/// Returns a 64-character hex key (256-bit material as hex string for PRAGMA key).
///
/// Recovery order: keychain -> fallback file -> generate new.
/// After obtaining a key from any source, the missing store is back-filled
/// so both always stay in sync.
pub fn get_or_create_encryption_key(app_data_dir: &Path) -> Result<String, String> {
    let fallback = app_data_dir.join(FALLBACK_FILENAME);

    // 1. Try keychain first.
    let keychain_key = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|k| !k.is_empty());

    // 2. Try fallback file.
    let file_key = read_fallback_key(&fallback);

    match (keychain_key, file_key) {
        // Both present — prefer keychain (canonical source).
        (Some(kc), _) => {
            ensure_fallback(&fallback, &kc);
            Ok(kc)
        }
        // Keychain lost, but fallback file survives — restore keychain.
        (None, Some(fk)) => {
            ensure_keychain(&fk);
            Ok(fk)
        }
        // Neither exists — generate a fresh key and persist to both.
        (None, None) => {
            let key = generate_secure_key()?;
            ensure_keychain(&key);
            write_key_file(&fallback, &key)
                .map_err(|e| format!("Encryption-key fallback write failed: {}", e))?;
            Ok(key)
        }
    }
}

/// Reads the fallback key file, returning `Some(key)` if non-empty.
fn read_fallback_key(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Best-effort: write key to fallback file with restricted permissions (no error propagated).
fn ensure_fallback(path: &Path, key: &str) {
    if path.exists() {
        return;
    }
    let _ = write_key_file(path, key);
}

/// Best-effort: write key to OS keychain (no error propagated).
fn ensure_keychain(key: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.set_password(key);
    }
}

fn generate_secure_key() -> Result<String, String> {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    Ok(hex::encode(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // -- generate_secure_key --

    #[test]
    fn secure_key_has_correct_length() {
        let key = generate_secure_key().unwrap();
        assert_eq!(key.len(), 64, "256-bit key = 64 hex chars");
    }

    #[test]
    fn secure_key_is_valid_hex() {
        let key = generate_secure_key().unwrap();
        assert!(
            key.chars().all(|c| c.is_ascii_hexdigit()),
            "Key should be valid hex: {key}"
        );
    }

    #[test]
    fn secure_key_is_unique_per_call() {
        let k1 = generate_secure_key().unwrap();
        let k2 = generate_secure_key().unwrap();
        assert_ne!(k1, k2, "Two random keys should differ");
    }

    // -- read_fallback_key --

    #[test]
    fn read_fallback_key_missing_file() {
        let path = std::env::temp_dir().join("brief_test_nonexistent_key_file");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_fallback_key(&path), None);
    }

    #[test]
    fn read_fallback_key_empty_file() {
        let path = std::env::temp_dir().join("brief_test_empty_key");
        std::fs::write(&path, "").unwrap();
        assert_eq!(read_fallback_key(&path), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_fallback_key_whitespace_only() {
        let path = std::env::temp_dir().join("brief_test_ws_key");
        std::fs::write(&path, "   \n  ").unwrap();
        assert_eq!(read_fallback_key(&path), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_fallback_key_valid() {
        let path = std::env::temp_dir().join("brief_test_valid_key");
        std::fs::write(&path, "  abc123def456  \n").unwrap();
        assert_eq!(read_fallback_key(&path), Some("abc123def456".to_string()));
        let _ = std::fs::remove_file(&path);
    }

    // -- write_key_file --

    #[test]
    fn write_key_file_creates_and_writes_content() {
        let path = std::env::temp_dir().join("brief_test_write_key");
        let _ = std::fs::remove_file(&path);
        write_key_file(&path, "test_key_data").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test_key_data");
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn write_key_file_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join("brief_test_write_key_perms");
        let _ = std::fs::remove_file(&path);
        write_key_file(&path, "secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        // Check owner-only read/write bits (0o600 = 0o100600 with file type bits).
        assert_eq!(mode & 0o777, 0o600, "File should be owner-only rw");
        let _ = std::fs::remove_file(&path);
    }
}
