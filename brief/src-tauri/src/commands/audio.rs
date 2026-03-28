//! Tauri commands for audio device enumeration and level metering.

use crate::audio::list_audio_input_devices;
use crate::error::AppError;
use crate::state::AppState;

/// Returns the names of all available audio input devices on the default CPAL host.
/// Used by the settings screen to populate the microphone selection dropdown.
#[tauri::command]
pub async fn list_audio_devices() -> Result<Vec<String>, String> {
    // Blocking CPAL call is fast (<1 ms typically), so spawn_blocking is not required.
    Ok(list_audio_input_devices())
}

/// Returns the RMS audio level [0.0, 1.0] for the currently active recording session.
/// The frontend polls this at ~5 Hz while recording to drive the level meter UI.
/// Returns 0.0 if the session is not found (silently — avoid noisy errors during UI polling).
#[tauri::command]
pub async fn get_audio_level(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<f32, String> {
    let recordings = state
        .recordings
        .lock()
        .map_err(|_| AppError::StateLocked.to_string())?;
    let rms = recordings
        .get(&session_id)
        .and_then(|r| r.last_rms.lock().ok().map(|v| *v))
        .unwrap_or(0.0);
    // Clamp to [0, 1] — raw RMS of f32 audio samples is already in [0, 1], but clamp defensively.
    Ok(rms.clamp(0.0, 1.0))
}
