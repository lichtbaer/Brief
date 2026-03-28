//! Tauri application entry: `AppState`, database setup on startup, and command registration.
//!
//! Command handlers live in the [`commands`] module, organized by domain.

mod audio;
mod commands;
mod crypto_key;
mod defaults;
mod error;
mod export;
mod memory;
mod recovery;
mod state;
mod storage;
mod summarize;
mod templates;
mod transcribe;
mod types;

use error::AppError;
use state::AppState;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;

/// Resolves a user-supplied path to a WAV file under the OS temp directory (no path traversal).
pub(crate) fn resolve_orphan_wav_path(user_path: &Path) -> Result<PathBuf, String> {
    if let Ok(c) = user_path.canonicalize() {
        let temp = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| AppError::IoError(e.to_string()))?;
        if c.starts_with(&temp) {
            return Ok(c);
        }
        return Err(AppError::InvalidAudioPath.into());
    }
    let file_name = user_path
        .file_name()
        .ok_or(AppError::InvalidAudioPath)?;
    let temp = std::env::temp_dir();
    let candidate = temp.join(file_name);
    let temp_canon = temp
        .canonicalize()
        .map_err(|e| AppError::IoError(e.to_string()))?;
    let parent_canon = candidate
        .parent()
        .ok_or(AppError::InvalidAudioPath)?
        .canonicalize()
        .map_err(|e| AppError::IoError(e.to_string()))?;
    if parent_canon != temp_canon {
        return Err(AppError::InvalidAudioPath.into());
    }
    Ok(candidate)
}

/// Builds the Tauri app with plugins, initializes encrypted storage and recommended LLM defaults, and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            let resolver = app.path();
            let app_data = resolver
                .app_data_dir()
                .map_err(|e| format!("App data path error: {}", e))?;
            std::fs::create_dir_all(&app_data)
                .map_err(|e| format!("Failed to create app data directory: {}", e))?;

            let db_path = app_data.join("brief.db");
            let key = crypto_key::get_or_create_encryption_key(&app_data)?;

            let storage = tauri::async_runtime::block_on(async {
                let storage = Storage::new(
                    db_path
                        .to_str()
                        .ok_or_else(|| "Invalid database path".to_string())?,
                    &key,
                )
                .await?;
                let ram_gb = memory::get_available_memory_gb();
                let recommended = memory::recommended_llm_model(ram_gb);
                storage
                    .apply_recommended_llm_if_not_overridden(recommended)
                    .await?;
                Ok::<_, String>(storage)
            })?;

            app.manage(AppState {
                recordings: Mutex::new(HashMap::new()),
                processing_sessions: Mutex::new(HashSet::new()),
                storage: tokio::sync::Mutex::new(storage),
                app_data_dir: app_data.clone(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::process_meeting,
            commands::recording::check_orphaned_recordings,
            commands::recording::recover_orphaned_recording,
            commands::recording::discard_orphaned_recording,
            commands::recording::regenerate_summary,
            commands::meetings::get_meeting,
            commands::meetings::list_meetings,
            commands::meetings::search_meetings,
            commands::meetings::update_meeting_tags,
            commands::meetings::list_meetings_by_tag,
            commands::meetings::update_speaker_names,
            commands::meetings::update_meeting_title,
            commands::meetings::delete_meeting,
            commands::meetings::list_meetings_by_type,
            commands::export::export_markdown,
            commands::export::export_pdf,
            commands::export::get_audio_path,
            commands::export::export_audio,
            commands::settings::get_setting_defaults,
            commands::settings::get_app_settings_snapshot,
            commands::settings::set_llm_model,
            commands::settings::dismiss_low_ram_onboarding,
            commands::settings::get_all_settings,
            commands::settings::get_retain_audio,
            commands::settings::set_retain_audio,
            commands::settings::update_setting,
            commands::health::check_whisperx,
            commands::health::check_ollama,
            commands::audio::list_audio_devices,
            commands::audio::get_audio_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // -- resolve_orphan_wav_path --

    #[test]
    fn resolve_orphan_rejects_path_outside_temp() {
        let outside = Path::new("/etc/passwd");
        assert!(resolve_orphan_wav_path(outside).is_err());
    }

    #[test]
    fn resolve_orphan_accepts_filename_only() {
        // A bare filename should be resolved under the temp directory.
        let result = resolve_orphan_wav_path(Path::new("brief_test123.wav"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn resolve_orphan_accepts_file_in_temp_dir() {
        let temp = std::env::temp_dir();
        let test_file = temp.join("brief_resolve_test.wav");
        // Create the file so canonicalize works.
        std::fs::write(&test_file, b"test").ok();
        let result = resolve_orphan_wav_path(&test_file);
        let _ = std::fs::remove_file(&test_file);
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_orphan_rejects_traversal() {
        let traversal = std::env::temp_dir().join("../../../etc/passwd");
        assert!(resolve_orphan_wav_path(&traversal).is_err());
    }
}
