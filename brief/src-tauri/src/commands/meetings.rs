//! Tauri commands for meeting retrieval, listing, and full-text search.

use crate::error::AppError;
use crate::storage::Storage;
use crate::state::AppState;

/// Fetches and parses a meeting as JSON value — shared helper to avoid duplicating the fetch+parse pattern.
pub async fn fetch_meeting_value(
    storage: &Storage,
    id: &str,
) -> Result<serde_json::Value, String> {
    let json = storage
        .get_meeting(id)
        .await?
        .ok_or_else(|| AppError::MeetingNotFound(id.to_string()).to_string())?;
    serde_json::from_str(&json).map_err(|e| AppError::IoError(e.to_string()).to_string())
}

/// Loads a meeting by id from the database or returns an error string if not found.
#[tauri::command]
pub async fn get_meeting(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage
        .get_meeting(&id)
        .await?
        .ok_or_else(|| AppError::MeetingNotFound(id).into())
}

/// Returns all meetings (newest first) as JSON, without full transcripts.
#[tauri::command]
pub async fn list_meetings(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.list_meetings().await
}

/// Full-text search across meeting titles and transcripts (FTS5).
#[tauri::command]
pub async fn search_meetings(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.search_meetings(&query).await
}
