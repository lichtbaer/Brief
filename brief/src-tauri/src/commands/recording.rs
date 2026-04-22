//! Tauri commands for recording lifecycle: start, stop, and process (WhisperX + Ollama).

use crate::audio::AudioRecorder;
use crate::error::AppError;
use crate::state::AppState;
use std::collections::HashSet;

/// RAII guard that marks a `session_id` as processing until dropped, so orphan detection skips the temp WAV.
struct ProcessingSessionGuard<'a> {
    state: &'a AppState,
    session_id: String,
}

impl<'a> ProcessingSessionGuard<'a> {
    fn new(state: &'a AppState, session_id: String) -> Result<Self, String> {
        state
            .processing_sessions
            .lock()
            .map_err(|_| AppError::StateLocked)?
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

/// Starts microphone capture for a new session; returns a UUID `session_id` stored in `AppState.recordings`.
#[tauri::command]
pub async fn start_recording(
    meeting_type: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Read the preferred audio device from settings; "default" or missing means use system default.
    let audio_device: Option<String> = {
        let storage = state.storage.lock().await;
        storage
            .get_setting("audio_device")
            .await
            .ok()
            .flatten()
            .filter(|d| !d.is_empty() && d != "default")
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let mut recorder = AudioRecorder::new(session_id.clone(), meeting_type);
    recorder.start_with_device(audio_device.as_deref())?;

    state
        .recordings
        .lock()
        .map_err(|_| AppError::StateLocked)?
        .insert(session_id.clone(), recorder);

    Ok(session_id)
}

/// Stops the given session, writes a temp WAV path, and removes the recorder from memory.
/// The `.remove()` inside the lock is atomic — concurrent `stop_recording` calls for the same
/// session are safe: the first caller wins and subsequent callers receive `SessionNotFound`.
#[tauri::command]
pub async fn stop_recording(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Atomic remove under the lock: only one caller can win the race for a given session_id.
    let mut recorder = {
        let mut guard = state.recordings.lock().map_err(|_| AppError::StateLocked)?;
        guard
            .remove(&session_id)
            .ok_or(AppError::SessionNotFound(session_id.clone()))?
        // Lock released here — stop_and_save runs without holding the global lock.
    };

    let audio_path = std::env::temp_dir().join(format!("brief_{session_id}.wav"));

    recorder.stop_and_save(&audio_path)?;

    Ok(audio_path.to_string_lossy().to_string())
}

/// Runs WhisperX + optional Ollama summarization, persists a [`Meeting`], and returns JSON (deletes or moves temp WAV per `retain_audio`).
#[tauri::command]
pub async fn process_meeting(
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
pub async fn process_meeting_inner(
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
            .unwrap_or(crate::transcribe::DEFAULT_WHISPERX_TIMEOUT_SECS);
        let lang = storage
            .get_setting("meeting_language")
            .await?
            .unwrap_or_else(|| "de".to_string());
        let lang = lang.trim().to_lowercase();
        let lang = if matches!(lang.as_str(), "de" | "en") {
            lang
        } else {
            log::warn!(
                "Unsupported meeting language '{}' — falling back to 'de'. \
                 Configure a supported language (de, en) in settings.",
                lang
            );
            "de".to_string()
        };
        (timeout, lang)
    };

    let transcriber = crate::transcribe::Transcriber::new(None, None)
        .with_timeout_secs(whisperx_timeout_secs)
        .with_language(meeting_language);

    if !transcriber.check_available() {
        return Err(AppError::WhisperxUnavailable.into());
    }

    let audio_path_buf = std::path::PathBuf::from(&audio_path);
    let audio_path_for_transcribe = audio_path_buf.clone();

    // `Transcriber::transcribe` already performs internal retries; map stable timeout / other errors
    // to `AppError` for consistent Tauri error strings and frontend handling.
    let result =
        tokio::task::spawn_blocking(move || transcriber.transcribe(&audio_path_for_transcribe))
            .await
            .map_err(|e| AppError::TaskError(e.to_string()).to_string())?
            .map_err(|e| {
                if e == crate::transcribe::TRANSCRIPTION_TIMEOUT_ERROR {
                    AppError::TranscriptionTimeout.to_string()
                } else {
                    AppError::TranscriptionFailed(e).to_string()
                }
            })?;

    let transcript = result
        .segments
        .iter()
        .map(|s| format!("[{}]: {}", s.speaker, s.text))
        .collect::<Vec<_>>()
        .join("\n");

    // Read Ollama config and optional custom prompt template in a single storage lock.
    let (ollama_url, llm_model, ollama_timeout_secs, custom_template) = {
        let storage = state.storage.lock().await;
        let (url, model, timeout) = storage.get_summarizer_config().await?;
        // Only read the custom template when meeting_type is "custom" — avoids unnecessary DB query.
        let custom = if meeting_type == "custom" {
            storage
                .get_setting("custom_prompt_template")
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        (url, model, timeout, custom)
    };

    // Retry up to 3 times with 2 s / 4 s / 8 s backoff on transient network failures.
    // JSON parse errors are never retried (see `Summarizer::summarize`).
    let summarizer = crate::summarize::Summarizer::new(
        Some(ollama_url),
        Some(llm_model),
        Some(ollama_timeout_secs),
    )?
    .with_retry_config(3, 2000);
    let output = if summarizer.check_available().await {
        let system_prompt = crate::templates::get_system_prompt_with_custom(
            &meeting_type,
            custom_template.as_deref(),
        );
        summarizer
            .summarize(&transcript, &system_prompt, &meeting_type)
            .await
            .unwrap_or_else(|e| {
                // Summarization failed after all retries — log so production failures are visible.
                // The meeting is still saved with a placeholder, which is the intended degraded behaviour.
                log::warn!(
                    "Summarization failed for session {} (type={}): {} — saving placeholder output",
                    session_id,
                    meeting_type,
                    e
                );
                crate::types::MeetingOutput::placeholder(&meeting_type)
            })
    } else {
        crate::types::MeetingOutput::placeholder(&meeting_type)
    };

    let duration_seconds = crate::types::calculate_duration_seconds(&result.segments);

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
        // Persist diarized segments so the frontend can render a timestamped transcript.
        segments: result.segments.clone(),
        speaker_names: std::collections::HashMap::new(),
    };

    if retain {
        let audio_dir = state.app_data_dir.join("audio");
        std::fs::create_dir_all(&audio_dir)
            .map_err(|e| AppError::IoError(format!("Failed to create audio directory: {e}")))?;
        let dest = audio_dir.join(format!("{session_id}.wav"));
        std::fs::rename(&audio_path_buf, &dest)
            .map_err(|e| AppError::IoError(format!("Failed to move audio file: {e}")))?;
        meeting.audio_path = Some(dest.to_str().ok_or(AppError::InvalidAudioPath)?.to_string());
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

    Ok(serde_json::to_string(&meeting).map_err(|e| AppError::IoError(e.to_string()))?)
}

/// Returns metadata for at most one orphaned temp WAV (newest first) for the recovery banner.
#[tauri::command]
pub async fn check_orphaned_recordings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let active: HashSet<String> = state
        .recordings
        .lock()
        .map_err(|_| AppError::StateLocked)?
        .keys()
        .cloned()
        .collect();
    let processing: HashSet<String> = state
        .processing_sessions
        .lock()
        .map_err(|_| AppError::StateLocked)?
        .clone();

    let mut paths =
        crate::recovery::find_orphaned_wav_files(&std::env::temp_dir(), &active, &processing);
    if paths.is_empty() {
        return Ok(vec![]);
    }
    paths.truncate(1);
    // ok_or_else because truncate guarantees one element at runtime, but the compiler cannot
    // verify that invariant — explicit error handling is required by CLAUDE.md rule 2.
    let path = paths.into_iter().next().ok_or_else(|| {
        AppError::IoError("No orphaned WAV path found after truncate".to_string())
    })?;
    let metadata = std::fs::metadata(&path).map_err(|e| AppError::IoError(e.to_string()))?;
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

/// Transcribes an orphaned temp WAV and saves a new meeting (uses persisted default meeting type and a dated recovery title).
#[tauri::command]
pub async fn recover_orphaned_recording(
    audio_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let path = std::path::PathBuf::from(&audio_path);
    let canonical = crate::resolve_orphan_wav_path(&path)?;
    if !canonical.is_file() {
        return Err(AppError::AudioNotFound(audio_path).into());
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let title = format!("Recovered meeting {}", date_str);

    // Use user's preferred default meeting type instead of hardcoded "consulting".
    let meeting_type = {
        let storage = state.storage.lock().await;
        storage
            .get_setting("default_meeting_type")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "consulting".to_string())
    };

    process_meeting_inner(
        new_id,
        canonical.to_string_lossy().to_string(),
        meeting_type,
        Some(title),
        &state,
    )
    .await
}

/// Re-runs the Ollama summarizer on an existing meeting's stored transcript.
/// Useful when the initial summary was poor or the user wants a different meeting type applied.
/// Returns the full updated meeting JSON or an error if Ollama is unreachable.
#[tauri::command]
pub async fn regenerate_summary(
    id: String,
    meeting_type: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Load the existing meeting to get its transcript.
    let meeting_json = {
        let storage = state.storage.lock().await;
        storage
            .get_meeting(&id)
            .await?
            .ok_or_else(|| crate::error::AppError::MeetingNotFound(id.clone()).to_string())?
    };

    let meeting: crate::types::Meeting =
        serde_json::from_str(&meeting_json).map_err(|e| e.to_string())?;

    let (ollama_url, llm_model, ollama_timeout_secs, custom_template) = {
        let storage = state.storage.lock().await;
        let (url, model, timeout) = storage.get_summarizer_config().await?;
        let custom = if meeting_type == "custom" {
            storage
                .get_setting("custom_prompt_template")
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        (url, model, timeout, custom)
    };

    let summarizer = crate::summarize::Summarizer::new(
        Some(ollama_url),
        Some(llm_model),
        Some(ollama_timeout_secs),
    )?
    .with_retry_config(3, 2000);

    if !summarizer.check_available().await {
        return Err("Ollama not reachable — is `ollama serve` running?".to_string());
    }

    let system_prompt =
        crate::templates::get_system_prompt_with_custom(&meeting_type, custom_template.as_deref());
    let new_output = summarizer
        .summarize(&meeting.transcript, &system_prompt, &meeting_type)
        .await
        .map_err(|e| e.to_string())?;

    {
        let storage = state.storage.lock().await;
        storage.update_meeting_output(&id, &new_output).await?;
    }

    // Return the refreshed meeting (segments preserved from the original load).
    let updated_meeting = crate::types::Meeting {
        output: new_output,
        meeting_type: meeting_type.clone(),
        ..meeting
    };

    serde_json::to_string(&updated_meeting).map_err(|e| e.to_string())
}

/// Deletes an orphaned temp WAV after explicit user confirmation (never silent).
#[tauri::command]
pub async fn discard_orphaned_recording(audio_path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&audio_path);
    let canonical = crate::resolve_orphan_wav_path(&path)?;
    std::fs::remove_file(&canonical).map_err(|e| AppError::IoError(e.to_string()).into())
}
