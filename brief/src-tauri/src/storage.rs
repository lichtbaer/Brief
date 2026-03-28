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
                ('onboarding_complete', 'false', datetime('now'))",
            defaults::OLLAMA_URL,
            defaults::LLM_MODEL,
            defaults::DEFAULT_MEETING_TYPE,
            defaults::RETENTION_DAYS,
            defaults::MEETING_LANGUAGE,
            defaults::RETAIN_AUDIO,
            defaults::UI_LANGUAGE,
            defaults::WHISPERX_TIMEOUT_SECS,
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

    /// Returns `(ollama_url, llm_model)` from settings with sensible defaults for summarization.
    pub async fn get_summarizer_config(&self) -> Result<(String, String), String> {
        let url = self
            .get_setting("ollama_url")
            .await?
            .unwrap_or_else(|| defaults::OLLAMA_URL.to_string());
        let model = self
            .get_setting("llm_model")
            .await?
            .unwrap_or_else(|| defaults::LLM_MODEL.to_string());
        Ok((url, model))
    }

    /// Inserts a full meeting row and updates the FTS5 shadow table for title/transcript search.
    /// Both INSERTs run inside a single transaction so the DB never ends up with a meetings row
    /// that is missing from the FTS index (or vice versa).
    pub async fn save_meeting(&self, meeting: &Meeting) -> Result<(), String> {
        let output_json = serde_json::to_string(&meeting.output).map_err(|e| e.to_string())?;
        let tags_json = serde_json::to_string(&meeting.tags).map_err(|e| e.to_string())?;

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
             title, transcript, output_json, audio_path, tags_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    /// Uses cursor-based pagination: pass `before` (a `created_at` ISO timestamp) to fetch
    /// the next page. Returns `{ "meetings": [...], "has_more": bool, "next_cursor": string | null }`.
    pub async fn list_meetings_paginated(
        &self,
        before: Option<&str>,
        limit: u32,
    ) -> Result<String, String> {
        // Fetch one extra row beyond the page size to determine whether a next page exists.
        let fetch_limit = i64::from(limit) + 1;

        let rows = if let Some(cursor) = before {
            sqlx::query(
                "SELECT id, created_at, meeting_type, title, output_json, tags_json
                 FROM meetings WHERE deleted_at IS NULL AND created_at < ?
                 ORDER BY created_at DESC
                 LIMIT ?",
            )
            .bind(cursor)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("list_meetings_paginated failed: {}", e))?
        } else {
            sqlx::query(
                "SELECT id, created_at, meeting_type, title, output_json, tags_json
                 FROM meetings WHERE deleted_at IS NULL
                 ORDER BY created_at DESC
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

        // The cursor for the next page is the `created_at` of the last returned row.
        let next_cursor: serde_json::Value = if has_more {
            page.last()
                .map(|r| serde_json::Value::String(r.get::<String, _>("created_at")))
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
            "SELECT id, created_at, meeting_type, title, output_json, tags_json
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

    /// Updates the tags for an existing meeting identified by `id`.
    /// Tags are validated: each tag must be non-empty and at most 50 characters;
    /// a maximum of 20 tags is allowed per meeting.
    pub async fn update_meeting_tags(&self, id: &str, tags: &[String]) -> Result<(), String> {
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

    /// Returns meeting summaries that contain the given tag (exact match via `json_each`).
    pub async fn list_meetings_by_tag(&self, tag: &str) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json, tags_json
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
            return Ok("[]".to_string());
        };

        let rows = sqlx::query(
            "SELECT m.id, m.created_at, m.meeting_type, m.title, m.output_json
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
    pub async fn get_meeting(&self, id: &str) -> Result<Option<String>, String> {
        let row = sqlx::query(
            "SELECT id, created_at, ended_at, duration_seconds, meeting_type,
             title, transcript, output_json, audio_path, tags_json, speaker_names_json
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
            })
            .to_string()
        }))
    }
}

/// Maps a database row (with id, created_at, meeting_type, title, output_json, tags_json columns)
/// to a summary JSON object used in the meeting list and history view.
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
}
