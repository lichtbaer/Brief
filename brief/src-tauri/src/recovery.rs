//! Detects leftover temporary WAV files from a crashed session (after stop, before `process_meeting` finished).
//! Temp files use `brief_<uuid>.wav`; older builds wrote `<uuid>.wav` only.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const BRIEF_PREFIX: &str = "brief_";

/// Returns the session UUID embedded in a Brief temp WAV filename, if valid.
pub fn session_id_from_wav_filename(name: &str) -> Option<String> {
    let path = Path::new(name);
    let stem = path.file_stem()?.to_str()?;
    let id = if let Some(rest) = stem.strip_prefix(BRIEF_PREFIX) {
        rest
    } else {
        stem
    };
    uuid::Uuid::parse_str(id).ok().map(|_| id.to_string())
}

/// Lists orphan WAV paths in `temp_dir`, excluding active capture or in-flight transcription.
/// Results are sorted by modification time (newest first).
pub fn find_orphaned_wav_files(
    temp_dir: &Path,
    active_session_ids: &HashSet<String>,
    processing_session_ids: &HashSet<String>,
) -> Vec<PathBuf> {
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(temp_dir) else {
        return Vec::new();
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("wav") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(session_id) = session_id_from_wav_filename(name) else {
            continue;
        };
        if active_session_ids.contains(&session_id) || processing_session_ids.contains(&session_id)
        {
            continue;
        }
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        candidates.push((path, mtime));
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.into_iter().map(|(p, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_from_brief_name() {
        assert_eq!(
            session_id_from_wav_filename("brief_550e8400-e29b-41d4-a716-446655440000.wav"),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn session_id_from_legacy_uuid_name() {
        assert_eq!(
            session_id_from_wav_filename("550e8400-e29b-41d4-a716-446655440000.wav"),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn session_id_rejects_non_uuid() {
        assert_eq!(session_id_from_wav_filename("brief_not-a-uuid.wav"), None);
        assert_eq!(session_id_from_wav_filename("random.wav"), None);
        assert_eq!(session_id_from_wav_filename("brief_.wav"), None);
    }

    #[test]
    fn session_id_rejects_non_wav_extension() {
        // The function works on filename stem, so .mp3 still extracts — but the caller
        // (find_orphaned_wav_files) filters by extension.  We test the stem extraction.
        assert_eq!(session_id_from_wav_filename("noext"), None);
    }

    #[test]
    fn find_orphaned_excludes_active_and_processing() {
        let dir = std::env::temp_dir().join("brief_recovery_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let uuid1 = "550e8400-e29b-41d4-a716-446655440001";
        let uuid2 = "550e8400-e29b-41d4-a716-446655440002";
        let uuid3 = "550e8400-e29b-41d4-a716-446655440003";

        // Create three temp WAV files.
        std::fs::write(dir.join(format!("brief_{uuid1}.wav")), b"fake").unwrap();
        std::fs::write(dir.join(format!("brief_{uuid2}.wav")), b"fake").unwrap();
        std::fs::write(dir.join(format!("brief_{uuid3}.wav")), b"fake").unwrap();
        // Also a non-WAV file that should be ignored.
        std::fs::write(dir.join("brief_unrelated.txt"), b"text").unwrap();

        let mut active = HashSet::new();
        active.insert(uuid1.to_string());
        let mut processing = HashSet::new();
        processing.insert(uuid2.to_string());

        let orphans = find_orphaned_wav_files(&dir, &active, &processing);
        assert_eq!(orphans.len(), 1);
        assert!(orphans[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains(uuid3));

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_orphaned_returns_empty_for_nonexistent_dir() {
        let missing = PathBuf::from("/nonexistent/brief_orphan_test_9f3a");
        let orphans = find_orphaned_wav_files(&missing, &HashSet::new(), &HashSet::new());
        assert!(orphans.is_empty());
    }

    #[test]
    fn find_orphaned_ignores_non_wav_files() {
        let dir = std::env::temp_dir().join("brief_recovery_test_ext");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let uuid = "550e8400-e29b-41d4-a716-446655440009";
        std::fs::write(dir.join(format!("brief_{uuid}.mp3")), b"fake").unwrap();
        std::fs::write(dir.join(format!("brief_{uuid}.txt")), b"fake").unwrap();

        let orphans = find_orphaned_wav_files(&dir, &HashSet::new(), &HashSet::new());
        assert!(orphans.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_id_from_empty_string() {
        assert_eq!(session_id_from_wav_filename(""), None);
    }

    #[test]
    fn session_id_from_only_extension() {
        assert_eq!(session_id_from_wav_filename(".wav"), None);
    }

    #[test]
    fn session_id_preserves_case() {
        // UUIDs are case-insensitive but our function should preserve the original case.
        let lower = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(
            session_id_from_wav_filename(&format!("brief_{lower}.wav")),
            Some(lower.to_string())
        );
    }

    #[test]
    fn session_id_rejects_partial_uuid() {
        // Not enough sections for a valid UUID.
        assert_eq!(
            session_id_from_wav_filename("brief_550e8400-e29b.wav"),
            None
        );
    }

    #[test]
    fn find_orphaned_empty_directory() {
        let dir = std::env::temp_dir().join("brief_recovery_test_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let orphans = find_orphaned_wav_files(&dir, &HashSet::new(), &HashSet::new());
        assert!(orphans.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_orphaned_returns_newest_first() {
        let dir = std::env::temp_dir().join("brief_recovery_test_sort");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let uuid_old = "550e8400-e29b-41d4-a716-446655440010";
        let uuid_new = "550e8400-e29b-41d4-a716-446655440011";

        std::fs::write(dir.join(format!("brief_{uuid_old}.wav")), b"old").unwrap();
        // Small delay to ensure different mtime.
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(dir.join(format!("brief_{uuid_new}.wav")), b"new").unwrap();

        let orphans = find_orphaned_wav_files(&dir, &HashSet::new(), &HashSet::new());
        assert_eq!(orphans.len(), 2);
        // Newest should be first.
        let first_name = orphans[0].file_name().unwrap().to_str().unwrap();
        assert!(first_name.contains(uuid_new), "Newest file should be first");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
