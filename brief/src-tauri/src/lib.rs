mod audio;
mod storage;
mod transcribe;

use audio::AudioRecorder;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct AppState {
    pub recordings: Mutex<HashMap<String, AudioRecorder>>,
}

#[tauri::command]
async fn start_recording(
    meeting_type: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut recorder = AudioRecorder::new(session_id.clone(), meeting_type);
    recorder.start()?;

    state
        .recordings
        .lock()
        .map_err(|_| "Interner Zustand gesperrt".to_string())?
        .insert(session_id.clone(), recorder);

    Ok(session_id)
}

#[tauri::command]
async fn stop_recording(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut recorder = state
        .recordings
        .lock()
        .map_err(|_| "Interner Zustand gesperrt".to_string())?
        .remove(&session_id)
        .ok_or_else(|| "Session nicht gefunden".to_string())?;

    let audio_path = std::env::temp_dir().join(format!("{session_id}.wav"));

    recorder.stop_and_save(&audio_path)?;

    Ok(audio_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn process_meeting(
    session_id: String,
    audio_path: String,
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let transcriber = transcribe::Transcriber::new(None, None);

    if !transcriber.check_available() {
        return Err(
            "WhisperX ist nicht verfügbar. Bitte Python-Umgebung einrichten: cd whisperx_runner && bash setup.sh"
                .to_string(),
        );
    }

    let audio_path_buf = std::path::PathBuf::from(&audio_path);

    let result = tokio::task::spawn_blocking(move || transcriber.transcribe(&audio_path_buf))
        .await
        .map_err(|e| format!("Task-Fehler: {}", e))??;

    std::fs::remove_file(&audio_path).ok();

    Ok(serde_json::json!({
        "session_id": session_id,
        "segments": result.segments,
        "language": result.language,
        "status": "transcribed"
    })
    .to_string())
}

#[tauri::command]
async fn get_meeting(id: String) -> Result<String, String> {
    let _ = id;
    Ok("{}".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            recordings: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            process_meeting,
            get_meeting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
