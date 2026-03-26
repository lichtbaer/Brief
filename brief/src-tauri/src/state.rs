//! Shared application state managed by Tauri — separated from domain types to break the
//! `types <-> storage` circular dependency.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::audio::AudioRecorder;
use crate::storage::Storage;

/// Shared mutable state: in-memory recorders, async SQLCipher storage, and app data directory for retained audio.
pub struct AppState {
    pub recordings: Mutex<HashMap<String, AudioRecorder>>,
    /// Session IDs currently inside `process_meeting` / `recover_orphaned_recording` (temp WAV still in use).
    pub processing_sessions: Mutex<HashSet<String>>,
    pub storage: tokio::sync::Mutex<Storage>,
    pub app_data_dir: PathBuf,
}
