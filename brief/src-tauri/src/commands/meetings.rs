//! Tauri commands for meeting retrieval, listing, full-text search, tags, and speaker names.

use std::collections::HashMap;

use crate::error::AppError;
use crate::state::AppState;
use crate::storage::Storage;

/// Fetches and parses a meeting as JSON value — shared helper to avoid duplicating the fetch+parse pattern.
pub async fn fetch_meeting_value(storage: &Storage, id: &str) -> Result<serde_json::Value, String> {
    let json = storage
        .get_meeting(id)
        .await?
        .ok_or_else(|| AppError::MeetingNotFound(id.to_string()).to_string())?;
    serde_json::from_str(&json).map_err(|e| AppError::IoError(e.to_string()).to_string())
}

/// Loads a meeting by id from the database or returns an error string if not found.
#[tauri::command]
pub async fn get_meeting(id: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
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
    storage.list_meetings_paginated(before.as_deref(), 20).await
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

/// Soft-deletes a meeting by id. The row is kept in the DB with `deleted_at` set so it no
/// longer appears in list/search but could be recovered in a future admin feature.
#[tauri::command]
pub async fn delete_meeting(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage.delete_meeting(&id).await
}

/// Updates the title of an existing meeting (max 200 chars). Keeps the FTS index in sync.
#[tauri::command]
pub async fn update_meeting_title(
    id: String,
    title: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Cap title length to prevent excessively long titles in the UI and DB.
    if title.trim().len() > 200 {
        return Err("Meeting title too long (max 200 characters)".to_string());
    }
    let storage = state.storage.lock().await;
    storage.update_meeting_title(&id, &title).await
}

/// Returns meeting summaries for a given meeting type (exact match).
#[tauri::command]
pub async fn list_meetings_by_type(
    meeting_type: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.list_meetings_by_type(&meeting_type).await
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

/// Removes audio files whose configured retention period has expired.
/// Intended to run on app startup and can be invoked on demand.
/// Returns the number of audio files purged.
#[tauri::command]
pub async fn enforce_audio_retention(state: tauri::State<'_, AppState>) -> Result<u32, String> {
    let storage = state.storage.lock().await;
    storage.purge_expired_audio().await
}

/// Persists a user-edited follow-up email draft text for an existing meeting.
/// Only `full_text` is patched inside `output_json` — all other output fields remain unchanged.
#[tauri::command]
pub async fn update_follow_up_draft(
    id: String,
    text: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage.update_follow_up_draft_text(&id, &text).await
}

/// Returns aggregated meeting statistics: total count, total duration, type breakdown,
/// action item count, and weekly meeting counts (last 12 weeks).
#[tauri::command]
pub async fn get_meeting_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.get_meeting_stats().await
}

/// Soft-deletes all meetings created before the given ISO timestamp.
/// Returns the count of deleted meetings.
#[tauri::command]
pub async fn delete_meetings_before(
    before: String,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let storage = state.storage.lock().await;
    storage.delete_meetings_before(&before).await
}

/// Returns meeting summaries whose `created_at` falls within the given date range (inclusive).
/// Both `from_date` and `to_date` are ISO-8601 date strings ("YYYY-MM-DD").
/// Results are newest-first, capped at 200 rows; pagination is disabled for date queries.
#[tauri::command]
pub async fn list_meetings_by_date_range(
    from_date: String,
    to_date: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage
        .list_meetings_by_date_range(&from_date, &to_date)
        .await
}

/// Returns a flat list of all action items across all non-deleted meetings, sorted by priority
/// then recency. Each entry includes the source meeting id and title for provenance.
#[tauri::command]
pub async fn get_all_action_items(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.get_all_action_items().await
}

/// Returns meeting summaries where the given participant name appears in `participants_mentioned`.
/// Enables the participant-based history filter in the frontend.
#[tauri::command]
pub async fn list_meetings_by_participant(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.list_meetings_by_participant(&name).await
}

/// Re-generates the Ollama summary for all non-deleted meetings matching `meeting_type`
/// (or all meetings when `meeting_type` is `None`). Progress is sequential to avoid
/// saturating the local Ollama instance. Returns `{ "regenerated": u32, "errors": u32 }`.
/// Concurrent calls are independent — the caller should disable the button while running.
#[tauri::command]
pub async fn bulk_regenerate_meetings(
    meeting_type: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Collect the IDs of meetings to re-summarize without holding the storage lock for the
    // entire duration of the Ollama calls (which can take minutes for many meetings).
    let meeting_ids: Vec<(String, String)> = {
        let storage = state.storage.lock().await;
        let raw = if let Some(ref mt) = meeting_type {
            storage.list_meetings_by_type(mt).await?
        } else {
            // list_meetings() returns a plain JSON array (legacy helper, up to 100 rows).
            storage.list_meetings().await?
        };
        let meetings: Vec<serde_json::Value> =
            serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        meetings
            .into_iter()
            .filter_map(|m| {
                let id = m["id"].as_str()?.to_string();
                let mt = m["meeting_type"].as_str()?.to_string();
                Some((id, mt))
            })
            .collect()
    };

    let (ollama_url, llm_model, ollama_timeout_secs) = {
        let storage = state.storage.lock().await;
        storage.get_summarizer_config().await?
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

    let mut regenerated: u32 = 0;
    let mut errors: u32 = 0;

    // Cap the total operation time so a hung Ollama instance cannot block indefinitely.
    // 30 minutes is generous even for 100+ long meetings at ~5–10 s each.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30 * 60);

    for (id, mt) in &meeting_ids {
        if tokio::time::Instant::now() >= deadline {
            log::warn!(
                "bulk_regenerate_meetings: 30-minute deadline reached after {}/{} meetings",
                regenerated + errors,
                meeting_ids.len()
            );
            break;
        }

        // Load transcript and optional custom template per meeting.
        let (transcript, custom_template) = {
            let storage = state.storage.lock().await;
            let json_opt = storage.get_meeting(id).await?;
            let json = match json_opt {
                Some(j) => j,
                None => {
                    errors += 1;
                    continue;
                }
            };
            let meeting: crate::types::Meeting = match serde_json::from_str(&json) {
                Ok(m) => m,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            let custom = if mt == "custom" {
                storage
                    .get_setting("custom_prompt_template")
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };
            (meeting.transcript, custom)
        };

        let system_prompt =
            crate::templates::get_system_prompt_with_custom(mt, custom_template.as_deref());

        // Per-meeting timeout of 10 minutes — consistent with the Ollama request timeout.
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let per_meeting_timeout = remaining.min(std::time::Duration::from_secs(10 * 60));

        let summarize_result = tokio::time::timeout(
            per_meeting_timeout,
            summarizer.summarize(&transcript, &system_prompt, mt),
        )
        .await;

        match summarize_result {
            Ok(Ok(new_output)) => {
                let storage = state.storage.lock().await;
                match storage.update_meeting_output(id, &new_output).await {
                    Ok(_) => regenerated += 1,
                    Err(e) => {
                        log::warn!(
                            "bulk_regenerate_meetings: DB update failed for {}: {}",
                            id,
                            e
                        );
                        errors += 1;
                    }
                }
            }
            Ok(Err(e)) => {
                log::warn!(
                    "bulk_regenerate_meetings: summarization failed for {}: {}",
                    id,
                    e
                );
                errors += 1;
            }
            Err(_elapsed) => {
                log::warn!(
                    "bulk_regenerate_meetings: per-meeting timeout exceeded for {}",
                    id
                );
                errors += 1;
            }
        }
    }

    serde_json::to_string(&serde_json::json!({
        "regenerated": regenerated,
        "errors": errors,
    }))
    .map_err(|e| e.to_string())
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
