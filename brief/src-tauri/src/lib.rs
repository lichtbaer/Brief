//! Tauri application entry: `AppState`, command handlers, and database setup on startup.
//!
//! Commands are thin wrappers over [`audio::AudioRecorder`], [`transcribe::Transcriber`], and [`storage::Storage`].

mod audio;
mod crypto_key;
mod export;
mod memory;
mod recovery;
mod storage;
mod summarize;
mod templates;
mod transcribe;
mod types;

use audio::AudioRecorder;
use base64::Engine as _;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use types::AppSettingsSnapshot;

/// Shared mutable state: in-memory recorders, async SQLCipher storage, and app data directory for retained audio.
pub struct AppState {
    pub recordings: Mutex<HashMap<String, AudioRecorder>>,
    /// Session IDs currently inside `process_meeting` / `recover_orphaned_recording` (temp WAV still in use).
    pub processing_sessions: Mutex<HashSet<String>>,
    pub storage: tokio::sync::Mutex<Storage>,
    pub app_data_dir: PathBuf,
}

/// Marks `session_id` as processing until dropped so orphan detection skips the temp WAV.
struct ProcessingSessionGuard<'a> {
    state: &'a AppState,
    session_id: String,
}

impl<'a> ProcessingSessionGuard<'a> {
    fn new(state: &'a AppState, session_id: String) -> Result<Self, String> {
        state
            .processing_sessions
            .lock()
            .map_err(|_| "Interner Zustand gesperrt".to_string())?
            .insert(session_id.clone());
        Ok(ProcessingSessionGuard { state, session_id })
    }
}

impl Drop for ProcessingSessionGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut g) = self.state.processing_sessions.lock() {
            g.remove(&self.session_id);
        }
    }
}

/// Resolves a user-supplied path to a WAV file under the OS temp directory (no path traversal).
fn resolve_orphan_wav_path(user_path: &Path) -> Result<PathBuf, String> {
    if let Ok(c) = user_path.canonicalize() {
        let temp = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if c.starts_with(&temp) {
            return Ok(c);
        }
        return Err("Ungültiger Audiopfad".to_string());
    }
    let file_name = user_path
        .file_name()
        .ok_or_else(|| "Ungültiger Audiopfad".to_string())?;
    let temp = std::env::temp_dir();
    let candidate = temp.join(file_name);
    let temp_canon = temp.canonicalize().map_err(|e| e.to_string())?;
    let parent_canon = candidate
        .parent()
        .ok_or_else(|| "Ungültiger Audiopfad".to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if parent_canon != temp_canon {
        return Err("Ungültiger Audiopfad".to_string());
    }
    Ok(candidate)
}

/// Starts microphone capture for a new session; returns a UUID `session_id` stored in `AppState.recordings`.
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

/// Stops the given session, writes a temp WAV path, and removes the recorder from memory.
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

    let audio_path = std::env::temp_dir().join(format!("brief_{session_id}.wav"));

    recorder.stop_and_save(&audio_path)?;

    Ok(audio_path.to_string_lossy().to_string())
}

/// Runs WhisperX + optional Ollama summarization, persists a [`Meeting`], and returns JSON (deletes or moves temp WAV per `retain_audio`).
#[tauri::command]
async fn process_meeting(
    session_id: String,
    audio_path: String,
    meeting_type: String,
    title_override: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    process_meeting_inner(session_id, audio_path, meeting_type, title_override, &state).await
}

/// Shared pipeline for normal recording and crash recovery (`recover_orphaned_recording`).
/// When `title_override` is set, it becomes the meeting title instead of a summary-derived default.
async fn process_meeting_inner(
    session_id: String,
    audio_path: String,
    meeting_type: String,
    title_override: Option<String>,
    state: &AppState,
) -> Result<String, String> {
    let _guard = ProcessingSessionGuard::new(state, session_id.clone())?;

    let (whisperx_timeout_secs, meeting_language) = {
        let storage = state.storage.lock().await;
        let timeout = storage
            .get_setting("whisperx_timeout_secs")
            .await?
            .and_then(|s| s.parse().ok())
            .unwrap_or(transcribe::DEFAULT_WHISPERX_TIMEOUT_SECS);
        let lang = storage
            .get_setting("meeting_language")
            .await?
            .unwrap_or_else(|| "de".to_string());
        let lang = lang.trim().to_lowercase();
        let lang = if matches!(lang.as_str(), "de" | "en") {
            lang
        } else {
            "de".to_string()
        };
        (timeout, lang)
    };

    let transcriber = transcribe::Transcriber::new(None, None)
        .with_timeout_secs(whisperx_timeout_secs)
        .with_language(meeting_language);

    if !transcriber.check_available() {
        return Err(
            "WhisperX ist nicht verfügbar. Bitte Python-Umgebung einrichten: cd whisperx_runner && bash setup.sh"
                .to_string(),
        );
    }

    let audio_path_buf = std::path::PathBuf::from(&audio_path);
    let audio_path_for_transcribe = audio_path_buf.clone();

    let result =
        tokio::task::spawn_blocking(move || transcriber.transcribe(&audio_path_for_transcribe))
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

    let mut title: String = title_override
        .as_ref()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| output.summary_short.chars().take(60).collect());
    if title.trim().is_empty() {
        title = format!("Meeting {}", session_id.chars().take(8).collect::<String>());
    }

    let retain = {
        let storage = state.storage.lock().await;
        storage
            .get_setting("retain_audio")
            .await?
            .unwrap_or_else(|| "false".to_string())
            == "true"
    };

    let mut meeting = crate::types::Meeting {
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

    if retain {
        let audio_dir = state.app_data_dir.join("audio");
        std::fs::create_dir_all(&audio_dir)
            .map_err(|e| format!("Audio-Ordner konnte nicht angelegt werden: {}", e))?;
        let dest = audio_dir.join(format!("{session_id}.wav"));
        std::fs::rename(&audio_path_buf, &dest)
            .map_err(|e| format!("Audio verschieben fehlgeschlagen: {}", e))?;
        meeting.audio_path = Some(
            dest.to_str()
                .ok_or_else(|| "Ungültiger Audiopfad".to_string())?
                .to_string(),
        );
    }

    {
        let storage = state.storage.lock().await;
        storage.save_meeting(&meeting).await?;
    }

    if !retain {
        if let Err(e) = std::fs::remove_file(&audio_path_buf) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.to_string());
            }
        }
    }

    Ok(serde_json::to_string(&meeting).map_err(|e| e.to_string())?)
}

/// Returns metadata for at most one orphaned temp WAV (newest first) for the recovery banner.
#[tauri::command]
async fn check_orphaned_recordings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let active: HashSet<String> = state
        .recordings
        .lock()
        .map_err(|_| "Interner Zustand gesperrt".to_string())?
        .keys()
        .cloned()
        .collect();
    let processing: HashSet<String> = state
        .processing_sessions
        .lock()
        .map_err(|_| "Interner Zustand gesperrt".to_string())?
        .clone();

    let mut paths = recovery::find_orphaned_wav_files(&std::env::temp_dir(), &active, &processing);
    if paths.is_empty() {
        return Ok(vec![]);
    }
    paths.truncate(1);
    let path = paths.into_iter().next().unwrap();
    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let size_mb = metadata.len() as f64 / 1_048_576.0;
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(vec![serde_json::json!({
        "path": path.to_string_lossy(),
        "filename": filename,
        "size_mb": format!("{:.1}", size_mb),
    })])
}

/// Transcribes an orphaned temp WAV and saves a new meeting (`consulting`, recovery title).
#[tauri::command]
async fn recover_orphaned_recording(
    audio_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let path = PathBuf::from(&audio_path);
    let canonical = resolve_orphan_wav_path(&path)?;
    if !canonical.is_file() {
        return Err("Audiodatei nicht gefunden".to_string());
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let title = format!("Unterbrochenes Meeting {}", date_str);
    process_meeting_inner(
        new_id,
        canonical.to_string_lossy().to_string(),
        "consulting".to_string(),
        Some(title),
        &state,
    )
    .await
}

/// Deletes an orphaned temp WAV after explicit user confirmation (never silent).
#[tauri::command]
async fn discard_orphaned_recording(audio_path: String) -> Result<(), String> {
    let path = PathBuf::from(&audio_path);
    let canonical = resolve_orphan_wav_path(&path)?;
    std::fs::remove_file(&canonical).map_err(|e| e.to_string())
}

/// Loads a meeting by id from the database or returns an error string if not found.
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

/// Returns RAM snapshot, recommended LLM model, current model, and low-RAM onboarding flags for the settings UI.
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

/// Persists the LLM model and marks `llm_model_user_override` so auto-recommendations do not overwrite it.
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

/// Persists the user's choice to hide the low-RAM onboarding hint.
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

/// Returns the on-disk path for stored meeting audio, or an error if none / missing file.
#[tauri::command]
async fn get_audio_path(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let meeting_json = storage
        .get_meeting(&id)
        .await?
        .ok_or_else(|| format!("Meeting {} nicht gefunden", id))?;
    let meeting: serde_json::Value =
        serde_json::from_str(&meeting_json).map_err(|e| e.to_string())?;
    let Some(audio_path) = meeting["audio_path"].as_str() else {
        return Err("Kein gespeichertes Audio für dieses Meeting".to_string());
    };
    let p = std::path::Path::new(audio_path);
    if !p.is_file() {
        return Err("Audiodatei nicht gefunden".to_string());
    }
    Ok(audio_path.to_string())
}

/// Opens a save dialog and copies the meeting WAV to the chosen location.
#[tauri::command]
async fn export_audio(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let (src, default_name) = {
        let storage = state.storage.lock().await;
        let meeting_json = storage
            .get_meeting(&id)
            .await?
            .ok_or_else(|| format!("Meeting {} nicht gefunden", id))?;
        let meeting: serde_json::Value =
            serde_json::from_str(&meeting_json).map_err(|e| e.to_string())?;
        let audio_path = meeting["audio_path"]
            .as_str()
            .ok_or_else(|| "Kein gespeichertes Audio für dieses Meeting".to_string())?;
        let title = meeting["title"].as_str().unwrap_or("meeting").to_string();
        Ok::<_, String>((
            PathBuf::from(audio_path),
            format!("{}.wav", safe_export_stem(title)),
        ))
    }?;

    if !src.is_file() {
        return Err("Audiodatei nicht gefunden".to_string());
    }

    let Some(dest_fp) = app
        .dialog()
        .file()
        .add_filter("WAV", &["wav"])
        .set_file_name(&default_name)
        .blocking_save_file()
    else {
        return Err("cancelled".to_string());
    };

    let dest_pb = dest_fp.into_path().map_err(|e| e.to_string())?;

    std::fs::copy(&src, &dest_pb)
        .map_err(|e| format!("Audio exportieren fehlgeschlagen: {}", e))?;

    dest_pb
        .to_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| "Ungültiger Zielpfad".to_string())
}

fn safe_export_stem(title: String) -> String {
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

/// Returns whether the user opted to keep meeting WAV files on disk after processing.
#[tauri::command]
async fn get_retain_audio(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let storage = state.storage.lock().await;
    let v = storage
        .get_setting("retain_audio")
        .await?
        .unwrap_or_else(|| "false".to_string());
    Ok(v == "true")
}

/// Persists the retain-audio toggle (`true` / `false` string in settings).
#[tauri::command]
async fn set_retain_audio(value: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage
        .set_setting("retain_audio", if value { "true" } else { "false" })
        .await
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
    if key == "whisperx_timeout_secs" {
        let trimmed = value.trim();
        let v: u64 = trimmed
            .parse()
            .map_err(|_| "Timeout muss eine positive Zahl (Sekunden) sein".to_string())?;
        if !(60..=86400).contains(&v) {
            return Err("Erlaubt: 60 bis 86400 Sekunden".to_string());
        }
        let storage = state.storage.lock().await;
        storage
            .set_setting("whisperx_timeout_secs", &v.to_string())
            .await?;
        return Ok(());
    }
    if key == "meeting_language" {
        let trimmed = value.trim().to_lowercase();
        if !matches!(trimmed.as_str(), "de" | "en") {
            return Err("Meeting-Sprache: nur \"de\" oder \"en\" erlaubt".to_string());
        }
        let storage = state.storage.lock().await;
        storage.set_setting("meeting_language", &trimmed).await?;
        return Ok(());
    }
    let storage = state.storage.lock().await;
    storage.set_setting(&key, &value).await
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
                processing_sessions: Mutex::new(HashSet::new()),
                storage: tokio::sync::Mutex::new(storage),
                app_data_dir: app_data.clone(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            process_meeting,
            check_orphaned_recordings,
            recover_orphaned_recording,
            discard_orphaned_recording,
            get_meeting,
            export_markdown,
            export_pdf,
            list_meetings,
            search_meetings,
            get_app_settings_snapshot,
            set_llm_model,
            dismiss_low_ram_onboarding,
            get_all_settings,
            get_audio_path,
            export_audio,
            get_retain_audio,
            set_retain_audio,
            update_setting,
            check_whisperx,
            check_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
