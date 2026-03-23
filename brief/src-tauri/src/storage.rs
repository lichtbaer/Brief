//! Encrypted SQLite persistence via SQLCipher (bundled).

use crate::types::{Meeting, MeetingOutput};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
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
            .map_err(|e| format!("DB-Verbindung fehlgeschlagen: {}", e))?;

        let storage = Storage { pool };
        storage.run_migrations().await.map_err(|e| {
            if e.contains("file is not a database") {
                format!(
                    "Die Datenbank '{}' kann mit dem aktuellen Schlüssel nicht geöffnet werden. \
                     Bitte lösche die Datei, damit eine neue angelegt wird. Ursprungsfehler: {}",
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
        .map_err(|e| format!("Migration fehlgeschlagen: {}", e))?;

        // Standalone FTS5 (no content=): external-content sync did not populate the inverted index
        // reliably with SQLCipher; we backfill from `meetings` on startup and upsert on save.
        sqlx::query("DROP TABLE IF EXISTS meetings_fts")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("FTS-Migration fehlgeschlagen: {}", e))?;
        sqlx::query(
            "CREATE VIRTUAL TABLE meetings_fts
             USING fts5(id UNINDEXED, title, transcript)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("FTS-Migration fehlgeschlagen: {}", e))?;
        sqlx::query(
            "INSERT INTO meetings_fts(rowid, id, title, transcript)
             SELECT rowid, id, title, transcript FROM meetings WHERE deleted_at IS NULL",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("FTS-Backfill fehlgeschlagen: {}", e))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Settings-Migration fehlgeschlagen: {}", e))?;

        sqlx::query(
            "INSERT OR IGNORE INTO settings VALUES
                ('ollama_url', 'http://localhost:11434', datetime('now')),
                ('whisper_model', 'whisper', datetime('now')),
                ('llm_model', 'llama3.1:8b', datetime('now')),
                ('default_meeting_type', 'consulting', datetime('now')),
                ('audio_device', 'default', datetime('now')),
                ('retention_days', '365', datetime('now')),
                ('llm_model_user_override', '0', datetime('now')),
                ('low_ram_onboarding_dismissed', '0', datetime('now'))",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Default-Settings fehlgeschlagen: {}", e))?;

        sqlx::query(
            "INSERT OR IGNORE INTO settings VALUES
                ('meeting_language', 'de', datetime('now')),
                ('retain_audio', 'false', datetime('now')),
                ('ui_language', 'de', datetime('now')),
                ('whisperx_timeout_secs', '900', datetime('now'))",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Default-Settings fehlgeschlagen: {}", e))?;

        sqlx::query(
            "INSERT OR IGNORE INTO settings VALUES
                ('onboarding_complete', 'false', datetime('now'))",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Default-Settings fehlgeschlagen: {}", e))?;

        // Upgrades: users who already have meetings should not see first-run onboarding.
        let meeting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Migration fehlgeschlagen: {}", e))?;
        if meeting_count > 0 {
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES ('onboarding_complete', 'true', datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            )
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Migration fehlgeschlagen: {}", e))?;
        }

        Ok(())
    }

    /// Reads a single `settings` row by key, or `None` if missing.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Setting lesen fehlgeschlagen: {}", e))?;
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
        .map_err(|e| format!("Setting speichern fehlgeschlagen: {}", e))?;
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
        .map_err(|e| format!("llm_model aktualisieren fehlgeschlagen: {}", e))?;
        Ok(())
    }

    /// Returns `(ollama_url, llm_model)` from settings with sensible defaults for summarization.
    pub async fn get_summarizer_config(&self) -> Result<(String, String), String> {
        let url = self
            .get_setting("ollama_url")
            .await?
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let model = self
            .get_setting("llm_model")
            .await?
            .unwrap_or_else(|| "llama3.1:8b".to_string());
        Ok((url, model))
    }

    /// Inserts a full meeting row and updates the FTS5 shadow table for title/transcript search.
    pub async fn save_meeting(&self, meeting: &Meeting) -> Result<(), String> {
        let output_json = serde_json::to_string(&meeting.output).map_err(|e| e.to_string())?;
        let tags_json = serde_json::to_string(&meeting.tags).map_err(|e| e.to_string())?;

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
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Meeting speichern fehlgeschlagen: {}", e))?;

        let rowid: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("rowid: {}", e))?;

        sqlx::query("INSERT INTO meetings_fts(rowid, id, title, transcript) VALUES (?, ?, ?, ?)")
            .bind(rowid)
            .bind(&meeting.id)
            .bind(&meeting.title)
            .bind(&meeting.transcript)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("FTS-Index fehlgeschlagen: {}", e))?;

        Ok(())
    }

    /// Returns meeting summaries (newest first), without full transcript.
    pub async fn list_meetings(&self) -> Result<String, String> {
        let rows = sqlx::query(
            "SELECT id, created_at, meeting_type, title, output_json
             FROM meetings WHERE deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("list_meetings failed: {}", e))?;

        let meetings: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let output_str: String = r.get("output_json");
                let output: serde_json::Value =
                    serde_json::from_str(&output_str).unwrap_or_else(|_| json!({}));
                let action_items = output["action_items"].as_array();
                let action_items_count = action_items.map(|a| a.len()).unwrap_or(0);
                json!({
                    "id": r.get::<String, _>("id"),
                    "created_at": r.get::<String, _>("created_at"),
                    "meeting_type": r.get::<String, _>("meeting_type"),
                    "title": r.get::<String, _>("title"),
                    "summary_short": output["summary_short"],
                    "action_items_count": action_items_count,
                })
            })
            .collect();

        serde_json::to_string(&meetings).map_err(|e| e.to_string())
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

        let meetings: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let output_str: String = r.get("output_json");
                let output: serde_json::Value =
                    serde_json::from_str(&output_str).unwrap_or_else(|_| json!({}));
                let action_items = output["action_items"].as_array();
                let action_items_count = action_items.map(|a| a.len()).unwrap_or(0);
                json!({
                    "id": r.get::<String, _>("id"),
                    "created_at": r.get::<String, _>("created_at"),
                    "meeting_type": r.get::<String, _>("meeting_type"),
                    "title": r.get::<String, _>("title"),
                    "summary_short": output["summary_short"],
                    "action_items_count": action_items_count,
                })
            })
            .collect();

        serde_json::to_string(&meetings).map_err(|e| e.to_string())
    }

    /// Loads one non-deleted meeting as a JSON string for the frontend, or `None` if absent.
    pub async fn get_meeting(&self, id: &str) -> Result<Option<String>, String> {
        let row = sqlx::query(
            "SELECT id, created_at, ended_at, duration_seconds, meeting_type,
             title, transcript, output_json, audio_path, tags_json
             FROM meetings WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Meeting laden fehlgeschlagen: {}", e))?;

        Ok(row.map(|r| {
            let output_json: String = r.get("output_json");
            let tags_json: String = r.get("tags_json");
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
            })
            .to_string()
        }))
    }
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

    let duration_seconds = if segments.is_empty() {
        0
    } else {
        let start = segments.first().map(|s| s.start).unwrap_or(0.0).max(0.0);
        let end = segments.last().map(|s| s.end).unwrap_or(0.0).max(0.0);
        ((end - start).max(0.0).ceil() as u32).max(1)
    };

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
}
