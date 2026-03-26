//! Tauri commands for health checks: WhisperX and Ollama availability.

use crate::error::AppError;
use crate::memory;
use crate::summarize;
use crate::transcribe;

/// Returns true if WhisperX venv Python can import whisperx.
#[tauri::command]
pub async fn check_whisperx() -> Result<bool, String> {
    tokio::task::spawn_blocking(|| transcribe::Transcriber::new(None, None).check_available())
        .await
        .map_err(|e| AppError::TaskError(e.to_string()).into())
}

/// Returns whether Ollama responds on localhost and the RAM-based recommended model id.
#[tauri::command]
pub async fn check_ollama() -> Result<serde_json::Value, String> {
    let summarizer = summarize::Summarizer::new(None, None)?;
    let running = summarizer.check_available().await;
    Ok(serde_json::json!({
        "running": running,
        "recommended_model": memory::recommended_llm_model(memory::get_available_memory_gb()),
    }))
}
