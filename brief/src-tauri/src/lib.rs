mod audio;
mod crypto_key;
mod export;
mod memory;
mod storage;
mod summarize;
mod templates;
mod transcribe;
mod types;

use audio::AudioRecorder;
use base64::Engine as _;
use std::collections::HashMap;
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;
use types::AppSettingsSnapshot;

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
    meeting_type: String,
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
        .map(|s| format!("[{}]: {}", s.speaker, s.text))
        .collect::<Vec<_>>()
        .join("\n");

    let (ollama_url, llm_model) = {
        let storage = state.storage.lock().await;
        storage.get_summarizer_config().await?
    };

    let summarizer = summarize::Summarizer::new(Some(ollama_url), Some(llm_model));
    let output = if summarizer.check_available().await {
        let system_prompt = templates::get_system_prompt(&meeting_type);
        summarizer
            .summarize(&transcript, &system_prompt, &meeting_type)
            .await
            .unwrap_or_else(|_| crate::types::MeetingOutput::placeholder(&meeting_type))
    } else {
        crate::types::MeetingOutput::placeholder(&meeting_type)
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

    let now = chrono::Utc::now().to_rfc3339();

    let mut title: String = output.summary_short.chars().take(60).collect();
    if title.trim().is_empty() {
        title = format!(
            "Meeting {}",
            session_id.chars().take(8).collect::<String>()
        );
    }

    let meeting = crate::types::Meeting {
        id: session_id.clone(),
        created_at: now.clone(),
        ended_at: now,
        duration_seconds,
        meeting_type: meeting_type.clone(),
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

/// Export meeting as Markdown (frontend saves via system dialog).
#[tauri::command]
async fn export_markdown(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting_json = storage
        .get_meeting(&id)
        .await?
        .ok_or_else(|| format!("Meeting {} nicht gefunden", id))?;
    let meeting: serde_json::Value =
        serde_json::from_str(&meeting_json).map_err(|e| e.to_string())?;
    Ok(export::generate_markdown(&meeting))
}

/// Export meeting as PDF bytes (base64); frontend decodes and saves via dialog.
#[tauri::command]
async fn export_pdf(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting_json = storage
        .get_meeting(&id)
        .await?
        .ok_or_else(|| format!("Meeting {} nicht gefunden", id))?;
    let meeting: serde_json::Value =
        serde_json::from_str(&meeting_json).map_err(|e| e.to_string())?;
    let markdown = export::generate_markdown(&meeting);
    let bytes = export::generate_pdf(&markdown)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Returns all meetings (newest first) as JSON, without full transcripts.
#[tauri::command]
async fn list_meetings(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.list_meetings().await
}

/// Full-text search across meeting titles and transcripts (FTS5).
#[tauri::command]
async fn search_meetings(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.search_meetings(&query).await
}

#[tauri::command]
async fn get_app_settings_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<AppSettingsSnapshot, String> {
    let storage = state.storage.lock().await;
    let memory_gb = memory::get_available_memory_gb();
    let recommended_model = memory::recommended_llm_model(memory_gb).to_string();
    let llm_model = storage
        .get_setting("llm_model")
        .await?
        .unwrap_or_else(|| "llama3.1:8b".to_string());
    let llm_model_user_override = storage
        .get_setting("llm_model_user_override")
        .await?
        .unwrap_or_else(|| "0".to_string())
        == "1";
    let dismissed = storage
        .get_setting("low_ram_onboarding_dismissed")
        .await?
        .unwrap_or_else(|| "0".to_string())
        == "1";
    let show_low_ram_onboarding = memory_gb <= 8.0 && !dismissed;
    Ok(AppSettingsSnapshot {
        memory_gb,
        recommended_model,
        llm_model,
        llm_model_user_override,
        show_low_ram_onboarding,
    })
}

#[tauri::command]
async fn set_llm_model(model: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err("Modellname darf nicht leer sein".to_string());
    }
    let storage = state.storage.lock().await;
    storage.set_setting("llm_model", trimmed).await?;
    storage.set_setting("llm_model_user_override", "1").await?;
    Ok(())
}

#[tauri::command]
async fn dismiss_low_ram_onboarding(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage
        .set_setting("low_ram_onboarding_dismissed", "1")
        .await?;
    Ok(())
}

/// Get all settings as a JSON object (string values).
#[tauri::command]
async fn get_all_settings(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.get_all_settings().await
}

/// Returns true if WhisperX venv Python can import whisperx.
#[tauri::command]
async fn check_whisperx() -> Result<bool, String> {
    tokio::task::spawn_blocking(|| transcribe::Transcriber::new(None, None).check_available())
        .await
        .map_err(|e| format!("Task-Fehler: {}", e))
}

/// Returns whether Ollama responds on localhost and the RAM-based recommended model id.
#[tauri::command]
async fn check_ollama() -> Result<serde_json::Value, String> {
    let summarizer = summarize::Summarizer::new(None, None);
    let running = summarizer.check_available().await;
    Ok(serde_json::json!({
        "running": running,
        "recommended_model": memory::recommended_llm_model(memory::get_available_memory_gb()),
    }))
}

/// Update a single setting (persists immediately).
#[tauri::command]
async fn update_setting(
    key: String,
    value: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if key == "llm_model" {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("Modellname darf nicht leer sein".to_string());
        }
        let storage = state.storage.lock().await;
        storage.set_setting("llm_model", trimmed).await?;
        storage.set_setting("llm_model_user_override", "1").await?;
        return Ok(());
    }
    let storage = state.storage.lock().await;
    storage.set_setting(&key, &value).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
                let storage = Storage::new(
                    db_path
                        .to_str()
                        .ok_or_else(|| "DB-Pfad ungültig".to_string())?,
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
                storage: tokio::sync::Mutex::new(storage),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            process_meeting,
            get_meeting,
            export_markdown,
            export_pdf,
            list_meetings,
            search_meetings,
            get_app_settings_snapshot,
            set_llm_model,
            dismiss_low_ram_onboarding,
            get_all_settings,
            update_setting,
            check_whisperx,
            check_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
