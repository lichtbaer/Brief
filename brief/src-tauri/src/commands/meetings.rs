//! Tauri commands for meeting retrieval, listing, full-text search, tags, and speaker names.

use std::collections::HashMap;

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

/// Returns paginated meeting summaries (newest first) without full transcripts.
/// Pass `before` (a `created_at` ISO timestamp) as the cursor to load the next page.
/// Returns `{ "meetings": [...], "has_more": bool, "next_cursor": string | null }`.
#[tauri::command]
pub async fn list_meetings(
    before: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage
        .list_meetings_paginated(before.as_deref(), 20)
        .await
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

/// Updates the tags for a meeting. Tags are validated: non-empty, max 50 chars each, max 20 total.
#[tauri::command]
pub async fn update_meeting_tags(
    id: String,
    tags: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Validate tags before persisting: trim, check length, limit count.
    if tags.len() > 20 {
        return Err("Too many tags (maximum 20)".to_string());
    }
    let validated: Vec<String> = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();

    for tag in &validated {
        if tag.len() > 50 {
            return Err(format!("Tag too long (max 50 chars): '{}'", tag));
        }
    }

    let storage = state.storage.lock().await;
    storage.update_meeting_tags(&id, &validated).await
}

/// Returns meeting summaries that contain the given tag (exact match).
#[tauri::command]
pub async fn list_meetings_by_tag(
    tag: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.list_meetings_by_tag(&tag).await
}

/// Persists the speaker label → display name mapping for a meeting.
/// The transcript text is not modified — names are applied at the display layer only,
/// so FTS search continues to match original speaker labels.
#[tauri::command]
pub async fn update_speaker_names(
    id: String,
    names: HashMap<String, String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage.update_speaker_names(&id, &names).await
}
