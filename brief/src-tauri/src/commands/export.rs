//! Tauri commands for exporting meetings as Markdown, PDF, or WAV audio.

use crate::error::AppError;
use crate::types::AppState;
use base64::Engine as _;
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;

use super::meetings::fetch_meeting_value;

/// Export meeting as Markdown (frontend saves via system dialog).
#[tauri::command]
pub async fn export_markdown(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting = fetch_meeting_value(&storage, &id).await?;
    Ok(crate::export::generate_markdown(&meeting))
}

/// Export meeting as PDF bytes (base64); frontend decodes and saves via dialog.
#[tauri::command]
pub async fn export_pdf(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting = fetch_meeting_value(&storage, &id).await?;
    let markdown = crate::export::generate_markdown(&meeting);
    let bytes = crate::export::generate_pdf(&markdown)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Returns the on-disk path for stored meeting audio, or an error if none / missing file.
#[tauri::command]
pub async fn get_audio_path(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting = fetch_meeting_value(&storage, &id).await?;
    let Some(audio_path) = meeting["audio_path"].as_str() else {
        return Err(AppError::AudioNotFound(id).into());
    };
    let p = std::path::Path::new(audio_path);
    if !p.is_file() {
        return Err(AppError::AudioNotFound(audio_path.to_string()).into());
    }
    Ok(audio_path.to_string())
}

/// Opens a save dialog and copies the meeting WAV to the chosen location.
#[tauri::command]
pub async fn export_audio(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let (src, default_name) = {
        let storage = state.storage.lock().await;
        let meeting = fetch_meeting_value(&storage, &id).await?;
        let audio_path = meeting["audio_path"]
            .as_str()
            .ok_or(AppError::AudioNotFound(id))?;
        let title = meeting["title"].as_str().unwrap_or("meeting").to_string();
        Ok::<_, String>((
            PathBuf::from(audio_path),
            format!("{}.wav", safe_export_stem(title)),
        ))
    }?;

    if !src.is_file() {
        return Err(AppError::AudioNotFound(src.to_string_lossy().to_string()).into());
    }

    let Some(dest_fp) = app
        .dialog()
        .file()
        .add_filter("WAV", &["wav"])
        .set_file_name(&default_name)
        .blocking_save_file()
    else {
        return Err(AppError::Cancelled.into());
    };

    let dest_pb = dest_fp
        .into_path()
        .map_err(|e| AppError::IoError(e.to_string()))?;

    std::fs::copy(&src, &dest_pb)
        .map_err(|e| AppError::IoError(format!("Failed to export audio: {e}")))?;

    dest_pb
        .to_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| AppError::InvalidAudioPath.into())
}

/// Sanitizes a meeting title for use as a file name stem (replaces unsafe characters, truncates to 80 chars).
pub fn safe_export_stem(title: String) -> String {
    let trimmed: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '-',
            c => c,
        })
        .collect();
    let t = trimmed.trim();
    if t.is_empty() {
        "meeting".to_string()
    } else {
        t.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_export_stem_normal_title() {
        assert_eq!(safe_export_stem("Team Meeting".to_string()), "Team Meeting");
    }

    #[test]
    fn safe_export_stem_empty_returns_fallback() {
        assert_eq!(safe_export_stem("".to_string()), "meeting");
    }

    #[test]
    fn safe_export_stem_whitespace_only_returns_fallback() {
        assert_eq!(safe_export_stem("   ".to_string()), "meeting");
    }

    #[test]
    fn safe_export_stem_replaces_unsafe_chars() {
        assert_eq!(
            safe_export_stem("File/With\\Special?Chars".to_string()),
            "File-With-Special-Chars"
        );
        assert_eq!(safe_export_stem("a:b|c".to_string()), "a-b-c");
        assert_eq!(safe_export_stem("x<y>z".to_string()), "x-y-z");
    }

    #[test]
    fn safe_export_stem_truncates_at_80_chars() {
        let long = "a".repeat(100);
        let result = safe_export_stem(long);
        assert_eq!(result.len(), 80);
    }

    #[test]
    fn safe_export_stem_preserves_unicode() {
        assert_eq!(safe_export_stem("Ü Ä Ö".to_string()), "Ü Ä Ö");
    }
}
