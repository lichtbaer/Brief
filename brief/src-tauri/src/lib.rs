mod audio;
mod crypto_key;
mod storage;
mod transcribe;
mod types;

use audio::AudioRecorder;
use std::collections::HashMap;
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;

pub struct AppState {
    pub recordings: Mutex<HashMap<String, AudioRecorder>>,
    pub storage: tokio::sync::Mutex<Storage>,
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
    state: tauri::State<'_, AppState>,
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

    let transcript = result
        .segments
        .iter()
        .map(|s| format!("[{}] {}", s.speaker, s.text))
        .collect::<Vec<_>>()
        .join("\n");

    let now = chrono::Utc::now().to_rfc3339();

    // Placeholder MeetingOutput — summarization in Phase 2 (BRIEF-005)
    let output = crate::types::MeetingOutput {
        summary_short: "Zusammenfassung folgt in Phase 2.".to_string(),
        topics: vec![],
        decisions: vec![],
        action_items: vec![],
        follow_up_draft: serde_json::json!({}),
        participants_mentioned: vec![],
        template_used: "none".to_string(),
        model_used: "whisper".to_string(),
        generated_at: now.clone(),
    };

    let duration_seconds = if result.segments.is_empty() {
        0
    } else {
        let start = result
            .segments
            .first()
            .map(|s| s.start)
            .unwrap_or(0.0)
            .max(0.0);
        let end = result
            .segments
            .last()
            .map(|s| s.end)
            .unwrap_or(0.0)
            .max(0.0);
        ((end - start).max(0.0).ceil() as u32).max(1)
    };

    let title = format!(
        "Meeting {}",
        session_id.chars().take(8).collect::<String>()
    );

    let meeting = crate::types::Meeting {
        id: session_id.clone(),
        created_at: now.clone(),
        ended_at: now.clone(),
        duration_seconds,
        meeting_type: "consulting".to_string(),
        title,
        transcript,
        output,
        audio_path: None,
        tags: vec![],
    };

    {
        let storage = state.storage.lock().await;
        storage.save_meeting(&meeting).await?;
    }

    if let Err(e) = std::fs::remove_file(&audio_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(e.to_string());
        }
    }

    Ok(serde_json::to_string(&meeting).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn get_meeting(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage
        .get_meeting(&id)
        .await?
        .ok_or_else(|| format!("Meeting {} nicht gefunden", id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let resolver = app.path();
            let app_data = resolver
                .app_data_dir()
                .map_err(|e| format!("App-Datenpfad: {}", e))?;
            std::fs::create_dir_all(&app_data)
                .map_err(|e| format!("App-Datenverzeichnis: {}", e))?;

            let db_path = app_data.join("brief.db");
            let key = crypto_key::get_or_create_encryption_key(&app_data)?;

            let storage = tauri::async_runtime::block_on(async {
                Storage::new(
                    db_path
                        .to_str()
                        .ok_or_else(|| "DB-Pfad ungültig".to_string())?,
                    &key,
                )
                .await
            })?;

            app.manage(AppState {
                recordings: Mutex::new(HashMap::new()),
                storage: tokio::sync::Mutex::new(storage),
            });

            Ok(())
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
