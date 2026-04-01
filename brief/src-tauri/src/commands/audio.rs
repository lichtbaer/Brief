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

/// Audio level and status returned by `get_audio_level`.
#[derive(serde::Serialize)]
pub struct AudioLevelStatus {
    /// RMS audio level [0.0, 1.0].
    pub level: f32,
    /// True when the recording buffer has hit the ~4 hour cap and frames are being dropped.
    pub buffer_overflow: bool,
}

/// Returns the RMS audio level [0.0, 1.0] and buffer overflow status for the active recording.
/// The frontend polls this at ~5 Hz while recording to drive the level meter UI.
/// Returns level 0.0 / no overflow if the session is not found (avoid noisy errors during polling).
#[tauri::command]
pub async fn get_audio_level(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<AudioLevelStatus, String> {
    let recordings = state
        .recordings
        .lock()
        .map_err(|_| AppError::StateLocked.to_string())?;
    let (rms, overflow) = recordings
        .get(&session_id)
        .map(|r| {
            let level = r.last_rms.lock().ok().map(|v| *v).unwrap_or(0.0);
            let overflow = r.buffer_overflow.lock().ok().map(|v| *v).unwrap_or(false);
            (level, overflow)
        })
        .unwrap_or((0.0, false));
    // Clamp to [0, 1] — raw RMS of f32 audio samples is already in [0, 1], but clamp defensively.
    Ok(AudioLevelStatus {
        level: rms.clamp(0.0, 1.0),
        buffer_overflow: overflow,
    })
}
