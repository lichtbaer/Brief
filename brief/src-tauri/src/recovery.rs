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
}
