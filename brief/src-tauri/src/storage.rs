//! Encrypted SQLite persistence via SQLCipher (bundled).

use crate::defaults;
use crate::types::{Meeting, MeetingOutput};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Acquire;
use sqlx::Row;
use std::path::Path;

/// SQLCipher-backed meeting and settings persistence with FTS5 search index.
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    /// Opens or creates an encrypted database at `db_path`, applies migrations (including FTS backfill), and returns a ready pool.
    pub async fn new(db_path: &str, encryption_key: &str) -> Result<Self, String> {
        log::info!("Opening database at {}", db_path);
        let key_pragma = format!("'{}'", escape_key_pragma(encryption_key));

        let opts = SqliteConnectOptions::new()
            .filename(Path::new(db_path))
            .create_if_missing(true)
            .pragma("key", key_pragma);

        let pool = SqlitePoolOptions::new()
            .connect_with(opts)
            .await
            .map_err(|e| format!("Database connection failed: {}", e))?;

        let storage = Storage { pool };
        storage.run_migrations().await.map_err(|e| {
            if e.contains("file is not a database") {
                format!(
                    "Database '{}' cannot be opened with the current key. \
                     Delete the file to create a fresh database. Original error: {}",
                    db_path, e
                )
            } else {
                e
            }
        })?;
        log::debug!("Database migrations complete");
        Ok(storage)
    }

    async fn run_migrations(&self) -> Result<(), String> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS meetings (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL,
                meeting_type TEXT NOT NULL,
                title TEXT NOT NULL,
                transcript TEXT NOT NULL,
                output_json TEXT NOT NULL,
                audio_path TEXT,
                tags_json TEXT DEFAULT '[]',
                deleted_at TEXT
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Migration failed: {}", e))?;

        // Standalone FTS5 (no content=): external-content sync did not populate the inverted index
        // reliably with SQLCipher; we create the table once and backfill on first creation only.
        // Checking sqlite_master avoids a DROP + full rebuild on every startup (O(n) for large DBs).
        let fts_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meetings_fts'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("FTS check failed: {}", e))?;

        if fts_exists == 0 {
            sqlx::query(
                "CREATE VIRTUAL TABLE meetings_fts
                 USING fts5(id UNINDEXED, title, transcript)",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("FTS migration failed: {}", e))?;

            sqlx::query(
                "INSERT INTO meetings_fts(rowid, id, title, transcript)
                 SELECT rowid, id, title, transcript FROM meetings WHERE deleted_at IS NULL",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("FTS backfill failed: {}", e))?;
        }

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Settings migration failed: {}", e))?;

        // Default settings use centralised constants from `defaults.rs` — single source of truth.
        let default_settings_sql = format!(
            "INSERT OR IGNORE INTO settings VALUES
                ('ollama_url', '{}', datetime('now')),
                ('whisper_model', 'whisper', datetime('now')),
                ('llm_model', '{}', datetime('now')),
                ('default_meeting_type', '{}', datetime('now')),
                ('audio_device', 'default', datetime('now')),
                ('retention_days', '{}', datetime('now')),
                ('llm_model_user_override', '0', datetime('now')),
                ('low_ram_onboarding_dismissed', '0', datetime('now')),
                ('meeting_language', '{}', datetime('now')),
                ('retain_audio', '{}', datetime('now')),
                ('ui_language', '{}', datetime('now')),
                ('whisperx_timeout_secs', '{}', datetime('now')),
                ('ollama_timeout_secs', '{}', datetime('now')),
                ('onboarding_complete', 'false', datetime('now'))",
            defaults::OLLAMA_URL,
            defaults::LLM_MODEL,
            defaults::DEFAULT_MEETING_TYPE,
            defaults::RETENTION_DAYS,
            defaults::MEETING_LANGUAGE,
            defaults::RETAIN_AUDIO,
            defaults::UI_LANGUAGE,
            defaults::WHISPERX_TIMEOUT_SECS,
            defaults::OLLAMA_TIMEOUT_SECS,
        );
        sqlx::query(&default_settings_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Default settings failed: {}", e))?;

        // Migration: add speaker_names_json column if it doesn't exist yet (existing installs).
        // SQLite does not support `ALTER TABLE … ADD COLUMN IF NOT EXISTS`, so we check first.
        let cols: Vec<String> = sqlx::query("PRAGMA table_info(meetings)")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("PRAGMA table_info failed: {}", e))?
            .into_iter()
            .map(|r| r.get::<String, _>("name"))
            .collect();

        if !cols.contains(&"speaker_names_json".to_string()) {
            sqlx::query(
                "ALTER TABLE meetings ADD COLUMN speaker_names_json TEXT DEFAULT '{}'",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Migration failed (speaker_names_json): {}", e))?;
        }

        // Migration: add segments_json column for diarized segment persistence (speaker + timestamps).
        if !cols.contains(&"segments_json".to_string()) {
            sqlx::query(
                "ALTER TABLE meetings ADD COLUMN segments_json TEXT DEFAULT '[]'",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Migration failed (segments_json): {}", e))?;
        }

        // Add indexes for common query patterns (idempotent — IF NOT EXISTS).
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_meetings_created_at ON meetings(created_at DESC) WHERE deleted_at IS NULL",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Index migration failed (created_at): {}", e))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_meetings_meeting_type ON meetings(meeting_type) WHERE deleted_at IS NULL",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Index migration failed (meeting_type): {}", e))?;

        // Upgrades: users who already have meetings should not see first-run onboarding.
        let meeting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Migration failed: {}", e))?;
        if meeting_count > 0 {
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES ('onboarding_complete', 'true', datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Migration failed: {}", e))?;
        }

        Ok(())
    }

    /// Reads a single `settings` row by key, or `None` if missing.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to read setting: {}", e))?;
        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    /// Upserts a `settings` key/value with `updated_at` set to now.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save setting: {}", e))?;
        Ok(())
    }

    /// Returns all settings rows as a JSON object (`key` → string value).
    pub async fn get_all_settings(&self) -> Result<String, String> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("get_all_settings failed: {}", e))?;

        let map: serde_json::Map<String, serde_json::Value> = rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("key"),
                    serde_json::Value::String(r.get::<String, _>("value")),
                )
            })
            .collect();

        serde_json::to_string(&map).map_err(|e| e.to_string())
    }

    /// Applies RAM-based default for `llm_model` unless the user chose a manual override.
    pub async fn apply_recommended_llm_if_not_overridden(
        &self,
        recommended: &str,
    ) -> Result<(), String> {
        let user_override = self
            .get_setting("llm_model_user_override")
            .await?
            .unwrap_or_else(|| "0".to_string());
        if user_override == "1" {
            return Ok(());
        }
        sqlx::query(
            "UPDATE settings SET value = ?, updated_at = datetime('now') WHERE key = 'llm_model'",
        )
        .bind(recommended)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update llm_model: {}", e))?;
        Ok(())
    }

    /// Returns `(ollama_url, llm_model, ollama_timeout_secs)` from settings for summarization.
    pub async fn get_summarizer_config(&self) -> Result<(String, String, u64), String> {
        let url = self
            .get_setting("ollama_url")
            .await?
            .unwrap_or_else(|| defaults::OLLAMA_URL.to_string());
        let model = self
            .get_setting("llm_model")
            .await?
            .unwrap_or_else(|| defaults::LLM_MODEL.to_string());
        let timeout_secs: u64 = self
            .get_setting("ollama_timeout_secs")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        Ok((url, model, timeout_secs))
    }

    /// Inserts a full meeting row and updates the FTS5 shadow table for title/transcript search.
    /// Both INSERTs run inside a single transaction so the DB never ends up with a meetings row
    /// that is missing from the FTS index (or vice versa).
    pub async fn save_meeting(&self, meeting: &Meeting) -> Result<(), String> {
        log::debug!("Saving meeting: id={} type={}", meeting.id, meeting.meeting_type);
        let output_json = serde_json::to_string(&meeting.output).map_err(|e| e.to_string())?;
        let tags_json = serde_json::to_string(&meeting.tags).map_err(|e| e.to_string())?;
        let segments_json = serde_json::to_string(&meeting.segments).map_err(|e| e.to_string())?;

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| format!("Database connection failed: {}", e))?;
        let mut tx = conn
            .begin()
            .await
            .map_err(|e| format!("Transaction failed: {}", e))?;

        sqlx::query(
            "INSERT INTO meetings (id, created_at, ended_at, duration_seconds, meeting_type,
             title, transcript, output_json, audio_path, tags_json, segments_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&meeting.id)
        .bind(&meeting.created_at)
        .bind(&meeting.ended_at)
        .bind(meeting.duration_seconds as i64)
        .bind(&meeting.meeting_type)
        .bind(&meeting.title)
        .bind(&meeting.transcript)
        .bind(&output_json)
        .bind(&meeting.audio_path)
        .bind(&tags_json)
        .bind(&segments_json)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to save meeting: {}", e))?;

        let rowid: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("rowid: {}", e))?;

        sqlx::query("INSERT INTO meetings_fts(rowid, id, title, transcript) VALUES (?, ?, ?, ?)")
            .bind(rowid)
            .bind(&meeting.id)
            .bind(&meeting.title)
            .bind(&meeting.transcript)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("FTS index failed: {}", e))?;

        tx.commit()
            .await
            .map_err(|e| format!("Commit failed: {}", e))?;

        Ok(())
    }

    /// Returns meeting summaries (newest first, page size 20), without full transcript.
    /// Uses composite cursor-based pagination: `before` is `"created_at|id"` so that meetings
    /// with identical timestamps are not skipped.
    /// Returns `{ "meetings": [...], "has_more": bool, "next_cursor": string | null }`.
    pub async fn list_meetings_paginated(
        &self,
        before: Option<&str>,
        limit: u32,
    ) -> Result<String, String> {
        // Fetch one extra row beyond the page size to determine whether a next page exists.
        let fetch_limit = i64::from(limit) + 1;

        let rows = if let Some(cursor) = before {
            // Composite cursor: "created_at|id" — split to get both parts.
            let (cursor_ts, cursor_id) = if let Some(pos) = cursor.rfind('|') {
                (&cursor[..pos], &cursor[pos + 1..])
            } else {
                // Backwards compatibility: plain timestamp cursor from older frontend.
                (cursor, "")
            };
            sqlx::query(
                "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
                 FROM meetings WHERE deleted_at IS NULL
                   AND (created_at < ?1 OR (created_at = ?1 AND id < ?2))
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?3",
            )
            .bind(cursor_ts)
            .bind(cursor_id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("list_meetings_paginated failed: {}", e))?
        } else {
            sqlx::query(
                "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
                 FROM meetings WHERE deleted_at IS NULL
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?",
            )
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("list_meetings_paginated failed: {}", e))?
        };

        let has_more = rows.len() as u32 > limit;
        // Return only the requested page (drop the sentinel row).
        let page = if has_more { &rows[..limit as usize] } else { &rows[..] };

        let meetings: Vec<serde_json::Value> = page.iter().map(row_to_meeting_summary).collect();

        // Composite cursor: "created_at|id" of the last returned row.
        let next_cursor: serde_json::Value = if has_more {
            page.last()
                .map(|r| {
                    let ts = r.get::<String, _>("created_at");
                    let id = r.get::<String, _>("id");
                    serde_json::Value::String(format!("{}|{}", ts, id))
                })
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        serde_json::to_string(&serde_json::json!({
            "meetings": meetings,
            "has_more": has_more,
            "next_cursor": next_cursor,
        }))
        .map_err(|e| e.to_string())
    }

    /// Legacy helper kept for internal tests that do not require pagination metadata.
    pub async fn list_meetings(&self) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
             FROM meetings WHERE deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Soft-deletes a meeting by setting `deleted_at` and removes it from the FTS index.
    /// Soft delete preserves the row for potential future recovery; the row is invisible to
    /// all list/get queries that filter on `deleted_at IS NULL`.
    pub async fn delete_meeting(&self, id: &str) -> Result<(), String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Transaction begin failed: {}", e))?;

        let affected = sqlx::query(
            "UPDATE meetings SET deleted_at = datetime('now') WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete_meeting failed: {}", e))?
        .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }

        // Remove from FTS index so deleted meetings no longer appear in search results.
        sqlx::query("DELETE FROM meetings_fts WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("FTS delete failed: {}", e))?;

        tx.commit()
            .await
            .map_err(|e| format!("Commit failed: {}", e))?;

        Ok(())
    }

    /// Updates the title of an existing meeting and keeps the FTS index in sync.
    /// Returns an error if the meeting does not exist or the title is blank.
    pub async fn update_meeting_title(&self, id: &str, title: &str) -> Result<(), String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Meeting title must not be empty".to_string());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Transaction begin failed: {}", e))?;

        let affected = sqlx::query(
            "UPDATE meetings SET title = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(title)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("update_meeting_title failed: {}", e))?
        .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }

        // Keep the FTS index in sync so search reflects the new title immediately.
        sqlx::query(
            "UPDATE meetings_fts SET title = ? WHERE id = ?",
        )
        .bind(title)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("FTS title update failed: {}", e))?;

        tx.commit()
            .await
            .map_err(|e| format!("Commit failed: {}", e))?;

        Ok(())
    }

    /// Replaces the `output_json` for an existing meeting (used by regenerate_summary).
    /// Returns an error if the meeting is not found.
    pub async fn update_meeting_output(
        &self,
        id: &str,
        output: &crate::types::MeetingOutput,
    ) -> Result<(), String> {
        let output_json = serde_json::to_string(output).map_err(|e| e.to_string())?;
        let affected = sqlx::query(
            "UPDATE meetings SET output_json = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&output_json)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("update_meeting_output failed: {}", e))?
        .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }
        Ok(())
    }

    /// Updates the tags for an existing meeting identified by `id`.
    /// Tags are validated: each tag must be non-empty and at most 50 characters;
    /// a maximum of 20 tags is allowed per meeting.
    pub async fn update_meeting_tags(&self, id: &str, tags: &[String]) -> Result<(), String> {
        // Backend validation (defense in depth — frontend also validates).
        if tags.len() > 20 {
            return Err("Too many tags (maximum 20)".to_string());
        }
        for tag in tags {
            if tag.trim().is_empty() {
                return Err("Tag must not be empty".to_string());
            }
            if tag.len() > 50 {
                return Err(format!("Tag too long (max 50 chars): '{}'", tag));
            }
        }
        let tags_json = serde_json::to_string(tags).map_err(|e| e.to_string())?;
        let affected = sqlx::query(
            "UPDATE meetings SET tags_json = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&tags_json)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("update_meeting_tags failed: {}", e))?
        .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }
        Ok(())
    }

    /// Returns all non-deleted meeting summaries with the given `meeting_type` (newest first).
    pub async fn list_meetings_by_type(&self, meeting_type: &str) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
             FROM meetings
             WHERE deleted_at IS NULL AND meeting_type = ?
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .bind(meeting_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings_by_type failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Returns meeting summaries that contain the given tag (exact match via `json_each`).
    pub async fn list_meetings_by_tag(&self, tag: &str) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
             FROM meetings
             WHERE deleted_at IS NULL
               AND EXISTS (
                 SELECT 1 FROM json_each(tags_json) WHERE value = ?1
               )
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .bind(tag)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings_by_tag failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Updates the speaker name mapping for an existing meeting.
    /// The map keys are speaker labels (e.g. "SPEAKER_00") and values are display names.
    /// The transcript column is NOT modified — names are applied at display time only.
    pub async fn update_speaker_names(
        &self,
        id: &str,
        names: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let names_json = serde_json::to_string(names).map_err(|e| e.to_string())?;
        let affected = sqlx::query(
            "UPDATE meetings SET speaker_names_json = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&names_json)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("update_speaker_names failed: {}", e))?
        .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }
        Ok(())
    }

    /// Full-text search across meeting titles and transcripts (FTS5).
    pub async fn search_meetings(&self, query: &str) -> Result<String, String> {
        let Some(fts_query) = build_fts5_query(query) else {
            log::debug!("search_meetings: empty or unparseable query, returning empty result");
            return Ok("[]".to_string());
        };
        log::debug!("search_meetings: fts_query={:?}", fts_query);

        let rows = sqlx::query(
            "SELECT m.id, m.created_at, m.meeting_type, m.title, m.output_json, m.tags_json, m.duration_seconds
             FROM meetings_fts
             JOIN meetings m ON m.id = meetings_fts.id
             WHERE meetings_fts MATCH ?1
             AND m.deleted_at IS NULL
             ORDER BY meetings_fts.rank
             LIMIT 20",
        )
        .bind(&fts_query)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("search_meetings failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Loads one non-deleted meeting as a JSON string for the frontend, or `None` if absent.
    /// Includes `speaker_names` (a map of speaker label → display name) for display-layer substitution.
    /// Includes `segments` (diarized utterances with timestamps) for transcript navigation.
    pub async fn get_meeting(&self, id: &str) -> Result<Option<String>, String> {
        let row = sqlx::query(
            "SELECT id, created_at, ended_at, duration_seconds, meeting_type,
             title, transcript, output_json, audio_path, tags_json, speaker_names_json, segments_json
             FROM meetings WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to load meeting: {}", e))?;

        Ok(row.map(|r| {
            let output_json: String = r.get("output_json");
            let tags_json: String = r.get("tags_json");
            // speaker_names_json may be NULL for rows created before the migration.
            let speaker_names_json: Option<String> = r.get("speaker_names_json");
            // segments_json may be NULL for rows created before the migration.
            let segments_json: Option<String> = r.get("segments_json");
            json!({
                "id": r.get::<String, _>("id"),
                "created_at": r.get::<String, _>("created_at"),
                "ended_at": r.get::<String, _>("ended_at"),
                "duration_seconds": r.get::<i64, _>("duration_seconds"),
                "meeting_type": r.get::<String, _>("meeting_type"),
                "title": r.get::<String, _>("title"),
                "transcript": r.get::<String, _>("transcript"),
                "output": serde_json::from_str::<serde_json::Value>(&output_json).unwrap_or_else(|_| json!({})),
                "audio_path": r.get::<Option<String>, _>("audio_path"),
                "tags": serde_json::from_str::<serde_json::Value>(&tags_json).unwrap_or_else(|_| json!([])),
                "speaker_names": speaker_names_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                    .unwrap_or_else(|| json!({})),
                "segments": segments_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                    .unwrap_or_else(|| json!([])),
            })
            .to_string()
        }))
    }

    /// Removes audio files whose retention period has expired and clears `audio_path` in the DB.
    /// Only acts when `retain_audio` is enabled; returns the number of purged entries.
    pub async fn purge_expired_audio(&self) -> Result<u32, String> {
        let retain_audio = self
            .get_setting("retain_audio")
            .await?
            .unwrap_or_else(|| "false".to_string());

        // When retain_audio is disabled, audio is deleted immediately after processing — nothing to purge.
        if retain_audio != "true" {
            return Ok(0);
        }

        let retention_days: i64 = self
            .get_setting("retention_days")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(365);

        // Find all non-deleted meetings with stored audio whose age exceeds the retention period.
        let rows = sqlx::query(
            "SELECT id, audio_path FROM meetings
             WHERE audio_path IS NOT NULL
               AND deleted_at IS NULL
               AND CAST(julianday('now') - julianday(created_at) AS INTEGER) > ?",
        )
        .bind(retention_days)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("purge_expired_audio query failed: {}", e))?;

        let mut purged: u32 = 0;
        for row in &rows {
            let id: String = row.get("id");
            let audio_path: String = row.get("audio_path");

            // Delete the file; "not found" is tolerated (may have been manually removed).
            if let Err(e) = std::fs::remove_file(&audio_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("purge_expired_audio: could not delete {}: {}", audio_path, e);
                }
            }

            // Clear DB reference so the meeting no longer references a deleted file.
            sqlx::query("UPDATE meetings SET audio_path = NULL WHERE id = ?")
                .bind(&id)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("purge_expired_audio update failed: {}", e))?;

            purged += 1;
        }

        Ok(purged)
    }

    /// Patches the `follow_up_draft.full_text` field inside the stored `output_json` blob.
    /// Deserializes the current output, replaces only `full_text`, and re-serializes to preserve all other fields.
    pub async fn update_follow_up_draft_text(&self, id: &str, text: &str) -> Result<(), String> {
        let row = sqlx::query(
            "SELECT output_json FROM meetings WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch meeting for draft update: {}", e))?
        .ok_or_else(|| crate::error::AppError::MeetingNotFound(id.to_string()).to_string())?;

        let output_json: String = row.get("output_json");
        let mut output: serde_json::Value =
            serde_json::from_str(&output_json).map_err(|e| e.to_string())?;

        // Create the follow_up_draft object if the LLM returned an empty/null value.
        if !output["follow_up_draft"].is_object() {
            output["follow_up_draft"] = json!({});
        }
        output["follow_up_draft"]["full_text"] = serde_json::Value::String(text.to_string());

        let new_json = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        let affected =
            sqlx::query("UPDATE meetings SET output_json = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&new_json)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("update_follow_up_draft_text failed: {}", e))?
                .rows_affected();

        if affected == 0 {
            return Err(crate::error::AppError::MeetingNotFound(id.to_string()).to_string());
        }
        Ok(())
    }

    /// Returns meeting summaries whose `created_at` falls within [from_date, to_date] (inclusive).
    /// Both parameters are ISO-8601 date strings (e.g. "2024-01-15"). Results are newest-first,
    /// capped at 200 rows — pagination is not needed for typical date-range queries.
    pub async fn list_meetings_by_date_range(
        &self,
        from_date: &str,
        to_date: &str,
    ) -> Result<String, String> {
        // Append time boundaries so the full day is covered regardless of the stored timestamp precision.
        let from = format!("{}T00:00:00", from_date);
        let to = format!("{}T23:59:59", to_date);

        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
             FROM meetings
             WHERE deleted_at IS NULL
               AND created_at >= ?1
               AND created_at <= ?2
             ORDER BY created_at DESC
             LIMIT 200",
        )
        .bind(&from)
        .bind(&to)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings_by_date_range failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Aggregates all action items from every non-deleted meeting into a flat list.
    /// Each entry carries the meeting id, title, creation date, and the action item fields
    /// (description, owner, due_date, priority). Sorted by priority (high → medium → low) then
    /// by meeting date (newest first). Capped at 500 rows to avoid excessive memory use.
    pub async fn get_all_action_items(&self) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT
               m.id          AS meeting_id,
               m.title       AS meeting_title,
               m.created_at  AS meeting_created_at,
               json_extract(item.value, '$.description') AS description,
               json_extract(item.value, '$.owner')       AS owner,
               json_extract(item.value, '$.due_date')    AS due_date,
               json_extract(item.value, '$.priority')    AS priority
             FROM meetings m, json_each(m.output_json, '$.action_items') AS item
             WHERE m.deleted_at IS NULL
             ORDER BY
               CASE json_extract(item.value, '$.priority')
                 WHEN 'high'   THEN 0
                 WHEN 'medium' THEN 1
                 ELSE 2
               END,
               m.created_at DESC
             LIMIT 500",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("get_all_action_items failed: {}", e))?;

        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "meeting_id":         r.get::<String, _>("meeting_id"),
                    "meeting_title":      r.get::<String, _>("meeting_title"),
                    "meeting_created_at": r.get::<String, _>("meeting_created_at"),
                    "description":        r.get::<Option<String>, _>("description"),
                    "owner":              r.get::<Option<String>, _>("owner"),
                    "due_date":           r.get::<Option<String>, _>("due_date"),
                    "priority":           r.get::<Option<String>, _>("priority"),
                })
            })
            .collect();

        serde_json::to_string(&items).map_err(|e| e.to_string())
    }

    /// Returns meeting summaries where the given participant name appears in
    /// `output_json.participants_mentioned` (exact match via `json_each`).
    /// This lets the frontend show all meetings involving a specific person by clicking
    /// their name in the participants section of a meeting output.
    pub async fn list_meetings_by_participant(&self, name: &str) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json, duration_seconds
             FROM meetings
             WHERE deleted_at IS NULL
               AND EXISTS (
                 SELECT 1 FROM json_each(output_json, '$.participants_mentioned')
                 WHERE value = ?1
               )
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings_by_participant failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows.iter().map(row_to_meeting_summary).collect();
        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Aggregates meeting statistics for the dashboard: total counts, duration, type breakdown,
    /// total action items, and weekly meeting counts (last 12 weeks).
    pub async fn get_meeting_stats(&self) -> Result<String, String> {
        // Total meetings and cumulative duration.
        let totals_row = sqlx::query(
            "SELECT COUNT(*) as total, COALESCE(SUM(duration_seconds), 0) as total_seconds
             FROM meetings WHERE deleted_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("get_meeting_stats totals failed: {}", e))?;

        let total_meetings: i64 = totals_row.get("total");
        let total_seconds: i64 = totals_row.get("total_seconds");

        // Breakdown by meeting type — used for the type distribution bar.
        let type_rows = sqlx::query(
            "SELECT meeting_type, COUNT(*) as count
             FROM meetings WHERE deleted_at IS NULL
             GROUP BY meeting_type ORDER BY count DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("get_meeting_stats by_type failed: {}", e))?;

        let by_type: Vec<serde_json::Value> = type_rows
            .iter()
            .map(|r| {
                json!({
                    "type": r.get::<String, _>("meeting_type"),
                    "count": r.get::<i64, _>("count"),
                })
            })
            .collect();

        // Total action items via SQLite JSON function across all non-deleted meetings.
        let action_items_row = sqlx::query(
            "SELECT COALESCE(SUM(json_array_length(output_json, '$.action_items')), 0) as total
             FROM meetings WHERE deleted_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("get_meeting_stats action_items failed: {}", e))?;

        let total_action_items: i64 = action_items_row.get("total");

        // Weekly meeting counts for the last 12 weeks (sparkline data).
        let weekly_rows = sqlx::query(
            "SELECT strftime('%Y-W%W', created_at) as week, COUNT(*) as count
             FROM meetings
             WHERE deleted_at IS NULL
               AND created_at >= datetime('now', '-12 weeks')
             GROUP BY week ORDER BY week",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("get_meeting_stats weekly failed: {}", e))?;

        let weekly: Vec<serde_json::Value> = weekly_rows
            .iter()
            .map(|r| {
                json!({
                    "week": r.get::<String, _>("week"),
                    "count": r.get::<i64, _>("count"),
                })
            })
            .collect();

        serde_json::to_string(&json!({
            "total_meetings": total_meetings,
            "total_seconds": total_seconds,
            "by_type": by_type,
            "total_action_items": total_action_items,
            "weekly": weekly,
        }))
        .map_err(|e| e.to_string())
    }

    /// Soft-deletes all meetings created before the given ISO timestamp and removes them from FTS.
    /// Returns the number of meetings deleted.
    pub async fn delete_meetings_before(&self, before: &str) -> Result<u32, String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Transaction begin failed: {}", e))?;

        // Collect IDs to remove from FTS before soft-deleting.
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM meetings WHERE deleted_at IS NULL AND created_at < ?",
        )
        .bind(before)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("delete_meetings_before select failed: {}", e))?;

        if ids.is_empty() {
            return Ok(0);
        }

        let affected = sqlx::query(
            "UPDATE meetings SET deleted_at = datetime('now') WHERE deleted_at IS NULL AND created_at < ?",
        )
        .bind(before)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("delete_meetings_before failed: {}", e))?
        .rows_affected() as u32;

        // Remove from FTS index.
        for id in &ids {
            sqlx::query("DELETE FROM meetings_fts WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("FTS delete failed: {}", e))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Commit failed: {}", e))?;

        log::info!("Bulk-deleted {} meetings before {}", affected, before);
        Ok(affected)
    }
}

/// Maps a database row (with id, created_at, meeting_type, title, output_json, tags_json,
/// duration_seconds columns) to a summary JSON object used in the meeting list and history view.
fn row_to_meeting_summary(r: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let output_str: String = r.get("output_json");
    let output: serde_json::Value =
        serde_json::from_str(&output_str).unwrap_or_else(|_| json!({}));
    let action_items = output["action_items"].as_array();
    let action_items_count = action_items.map(|a| a.len()).unwrap_or(0);
    let tags_json: String = r.get("tags_json");
    let tags: serde_json::Value =
        serde_json::from_str(&tags_json).unwrap_or_else(|_| json!([]));
    json!({
        "id": r.get::<String, _>("id"),
        "created_at": r.get::<String, _>("created_at"),
        "meeting_type": r.get::<String, _>("meeting_type"),
        "title": r.get::<String, _>("title"),
        "summary_short": output["summary_short"],
        "action_items_count": action_items_count,
        "duration_seconds": r.get::<i64, _>("duration_seconds"),
        "tags": tags,
    })
}

fn escape_key_pragma(key: &str) -> String {
    key.replace('\'', "''")
}

/// Builds a safe FTS5 MATCH string (tokens AND-combined; alphanumeric tokens left bare for tokenizer).
fn build_fts5_query(raw: &str) -> Option<String> {
    let tokens: Vec<&str> = raw.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return None;
    }
    Some(
        tokens
            .iter()
            .map(|t| {
                if t.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    (*t).to_string()
                } else {
                    let escaped = t.replace('"', "\"\"");
                    format!("\"{}\"", escaped)
                }
            })
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

/// Build a [Meeting] from WhisperX output (LLM step can fill [MeetingOutput] later).
#[allow(dead_code)] // Used by unit tests; summarization may reuse in Phase 2.
pub fn meeting_from_transcription(
    session_id: String,
    meeting_type: String,
    title: String,
    audio_path: Option<String>,
    segments: &[crate::transcribe::DiarizedSegment],
    language: &str,
) -> Meeting {
    let transcript = segments
        .iter()
        .map(|s| format!("[{}] {}", s.speaker, s.text))
        .collect::<Vec<_>>()
        .join("\n");

    let duration_seconds = crate::types::calculate_duration_seconds(segments);

    let ended = chrono::Utc::now();
    let created = if duration_seconds == 0 {
        ended
    } else {
        ended - chrono::Duration::seconds(i64::from(duration_seconds))
    };

    let summary_short = segments
        .first()
        .map(|s| s.text.chars().take(200).collect::<String>())
        .unwrap_or_default();

    let output = MeetingOutput {
        summary_short,
        topics: vec![],
        decisions: vec![],
        action_items: vec![],
        follow_up_draft: json!({}),
        participants_mentioned: vec![],
        template_used: "whisperx".to_string(),
        model_used: format!("whisperx/{}", language),
        generated_at: ended.to_rfc3339(),
        template_version: crate::templates::TEMPLATE_VERSION.to_string(),
    };

    Meeting {
        id: session_id,
        created_at: created.to_rfc3339(),
        ended_at: ended.to_rfc3339(),
        duration_seconds,
        meeting_type,
        title,
        transcript,
        output,
        audio_path,
        tags: vec![],
        segments: segments.to_vec(),
        speaker_names: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_fts5_query_quotes_and_combines_tokens() {
        assert_eq!(build_fts5_query("foo bar").as_deref(), Some("foo AND bar"));
        assert_eq!(build_fts5_query("  "), None);
        assert_eq!(
            build_fts5_query("say \"x\"").as_deref(),
            Some(r#"say AND """x""""#)
        );
    }

    #[test]
    fn build_fts5_query_single_token() {
        assert_eq!(build_fts5_query("hello").as_deref(), Some("hello"));
    }

    #[test]
    fn build_fts5_query_empty_string() {
        assert_eq!(build_fts5_query(""), None);
        assert_eq!(build_fts5_query("   "), None);
    }

    #[test]
    fn build_fts5_query_special_chars_quoted() {
        // Tokens with special characters should be double-quoted for FTS5 safety.
        assert_eq!(
            build_fts5_query("hello@world").as_deref(),
            Some("\"hello@world\"")
        );
        // Hyphenated tokens are also quoted.
        let result = build_fts5_query("hello-world").unwrap();
        assert!(result.contains('"'), "Hyphenated token should be quoted: {result}");
    }

    #[test]
    fn build_fts5_query_underscores_are_alphanumeric() {
        // Underscores should pass through as bare tokens (not quoted).
        assert_eq!(
            build_fts5_query("meeting_notes").as_deref(),
            Some("meeting_notes")
        );
    }

    #[test]
    fn build_fts5_query_unicode_tokens() {
        // German umlauts and accented chars are alphanumeric.
        assert_eq!(
            build_fts5_query("Büro Café").as_deref(),
            Some("Büro AND Café")
        );
        assert_eq!(
            build_fts5_query("Ärzte Überweisung").as_deref(),
            Some("Ärzte AND Überweisung")
        );
    }

    #[tokio::test]
    async fn migrations_idempotent_and_encrypted_roundtrip() {
        let tmp = std::env::temp_dir().join(format!(
            "brief_test_{}.db",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&tmp);
        let key = "test-key-hex-no-quotes-0123456789abcdef";

        let s1 = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();

        let m = meeting_from_transcription(
            "id-1".into(),
            "consulting".into(),
            "Title".into(),
            None,
            &[],
            "de",
        );
        s1.save_meeting(&m).await.unwrap();

        let s2 = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();
        let json = s2.get_meeting("id-1").await.unwrap().unwrap();
        assert!(json.contains("\"id\":\"id-1\""));

        let list = s2.list_meetings().await.unwrap();
        assert!(list.contains("id-1"));
        assert!(list.contains("summary_short"));

        let search = s2.search_meetings("Title").await.unwrap();
        assert!(search.contains("id-1"));

        let _ = std::fs::remove_file(&tmp);
    }

    // -- escape_key_pragma --

    #[test]
    fn escape_key_pragma_no_quotes() {
        assert_eq!(escape_key_pragma("abc123"), "abc123");
    }

    #[test]
    fn escape_key_pragma_empty_string() {
        assert_eq!(escape_key_pragma(""), "");
    }

    #[test]
    fn escape_key_pragma_single_quote() {
        assert_eq!(escape_key_pragma("it's"), "it''s");
    }

    #[test]
    fn escape_key_pragma_multiple_quotes() {
        assert_eq!(escape_key_pragma("a'b'c"), "a''b''c");
    }

    #[test]
    fn build_fts5_query_many_tokens() {
        let result = build_fts5_query("a b c d").unwrap();
        assert_eq!(result, "a AND b AND c AND d");
    }

    // -- Settings roundtrip (async) --

    #[tokio::test]
    async fn settings_roundtrip() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("brief_test_settings_{ts}.db"));
        let _ = std::fs::remove_file(&tmp);
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let s = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();

        // Set and get a setting.
        s.set_setting("test_key", "test_value").await.unwrap();
        let v = s.get_setting("test_key").await.unwrap();
        assert_eq!(v, Some("test_value".to_string()));

        // Missing key returns None.
        let missing = s.get_setting("nonexistent").await.unwrap();
        assert_eq!(missing, None);

        // Update existing key.
        s.set_setting("test_key", "updated").await.unwrap();
        let v2 = s.get_setting("test_key").await.unwrap();
        assert_eq!(v2, Some("updated".to_string()));

        // get_all_settings includes the key.
        let all = s.get_all_settings().await.unwrap();
        assert!(all.contains("test_key"));
        assert!(all.contains("updated"));

        let _ = std::fs::remove_file(&tmp);
    }

    // -- get_meeting_stats on empty database --

    #[tokio::test]
    async fn get_meeting_stats_empty_db_returns_zero_total() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("brief_test_stats_{ts}.db"));
        let _ = std::fs::remove_file(&tmp);
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let s = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();
        let stats_json = s.get_meeting_stats().await.unwrap();

        // Stats must be valid JSON containing a total field equal to 0.
        let stats: serde_json::Value = serde_json::from_str(&stats_json).unwrap();
        // An empty DB may return 0 or null for the total — both represent zero meetings.
        let total = stats["total"].as_i64().unwrap_or(0);
        assert_eq!(total, 0, "Empty DB must report total=0");

        let _ = std::fs::remove_file(&tmp);
    }

    // -- update_speaker_names roundtrip --

    #[tokio::test]
    async fn update_speaker_names_roundtrip() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("brief_test_speakers_{ts}.db"));
        let _ = std::fs::remove_file(&tmp);
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let s = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();

        // Save a meeting first.
        let m = meeting_from_transcription(
            "spk-1".into(),
            "internal".into(),
            "Speaker Test".into(),
            None,
            &[],
            "de",
        );
        s.save_meeting(&m).await.unwrap();

        // Assign display names to speaker labels.
        let mut names = std::collections::HashMap::new();
        names.insert("SPEAKER_00".to_string(), "Alice".to_string());
        names.insert("SPEAKER_01".to_string(), "Bob".to_string());
        s.update_speaker_names("spk-1", &names).await.unwrap();

        // Reload and verify the names are persisted.
        let json = s.get_meeting("spk-1").await.unwrap().unwrap();
        assert!(json.contains("Alice"), "Alice must be in stored speaker_names");
        assert!(json.contains("Bob"), "Bob must be in stored speaker_names");

        let _ = std::fs::remove_file(&tmp);
    }

    // -- search_meetings unicode query --

    #[tokio::test]
    async fn search_meetings_unicode_query_returns_match() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!("brief_test_unicode_{ts}.db"));
        let _ = std::fs::remove_file(&tmp);
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        let s = Storage::new(tmp.to_str().unwrap(), key).await.unwrap();

        // Save a meeting whose title contains German umlauts.
        let m = meeting_from_transcription(
            "uni-1".into(),
            "consulting".into(),
            "Ärztliches Gespräch".into(),
            None,
            &[],
            "de",
        );
        s.save_meeting(&m).await.unwrap();

        // Searching with umlauts must find the meeting.
        let results = s.search_meetings("Ärztliches").await.unwrap();
        assert!(
            results.contains("uni-1"),
            "Unicode FTS5 search must find the meeting by umlaut title"
        );

        let _ = std::fs::remove_file(&tmp);
    }
}
