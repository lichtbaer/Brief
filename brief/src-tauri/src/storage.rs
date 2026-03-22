//! Encrypted SQLite persistence via SQLCipher (bundled).

use crate::types::{Meeting, MeetingOutput};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;

pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
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
        storage.run_migrations().await?;
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

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS meetings_fts
             USING fts5(id UNINDEXED, title, transcript, content='meetings', content_rowid='rowid')",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("FTS-Migration fehlgeschlagen: {}", e))?;

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
                ('retention_days', '365', datetime('now'))",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Default-Settings fehlgeschlagen: {}", e))?;

        Ok(())
    }

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

        sqlx::query("INSERT INTO meetings_fts (rowid, id, title, transcript) VALUES (?, ?, ?, ?)")
            .bind(rowid)
            .bind(&meeting.id)
            .bind(&meeting.title)
            .bind(&meeting.transcript)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("FTS-Index fehlgeschlagen: {}", e))?;

        Ok(())
    }

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

        let _ = std::fs::remove_file(&tmp);
    }
}
