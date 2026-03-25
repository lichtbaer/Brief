//! Structured application error type for Tauri commands.
//!
//! Replaces scattered `Result<T, String>` error messages with categorised variants.
//! Each variant carries a human-readable message; `Display` produces the string that
//! Tauri forwards to the frontend.  The `From<AppError> for String` impl lets `?`
//! propagate through functions that still return `Result<T, String>`.

use std::fmt;

/// Categorised error for Tauri command handlers.
///
/// Variants cover the major failure modes of the app (I/O, database, audio,
/// transcription, validation, …).  Adding a new variant here automatically
/// makes it usable with `?` in any `Result<T, String>` context.
#[derive(Debug)]
pub enum AppError {
    /// Mutex / internal-state lock failure.
    StateLocked,
    /// Active recording session not found.
    SessionNotFound(String),
    /// No microphone / input device available.
    NoMicrophone,
    /// Audio file missing or inaccessible.
    AudioNotFound(String),
    /// Path traversal or otherwise unsafe audio path.
    InvalidAudioPath,
    /// WhisperX environment not set up.
    WhisperxUnavailable,
    /// Transcription exceeded the configured timeout (stable token reused by frontend).
    TranscriptionTimeout,
    /// Generic transcription subprocess failure.
    TranscriptionFailed(String),
    /// Ollama / LLM summarisation failure.
    SummarizationFailed(String),
    /// SQLite / SQLCipher persistence failure.
    DatabaseError(String),
    /// Filesystem I/O failure.
    IoError(String),
    /// Input validation failure (e.g. empty model name, out-of-range timeout).
    ValidationError(String),
    /// Meeting not found in the database.
    MeetingNotFound(String),
    /// Tokio / thread join failure.
    TaskError(String),
    /// User cancelled a dialog (not a real error, but needs to propagate).
    Cancelled,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateLocked => write!(f, "Internal state lock failed"),
            Self::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            Self::NoMicrophone => write!(f, "No microphone found"),
            Self::AudioNotFound(p) => write!(f, "Audio file not found: {p}"),
            Self::InvalidAudioPath => write!(f, "Invalid audio path"),
            Self::WhisperxUnavailable => write!(
                f,
                "WhisperX is not available. Please set up the Python environment: cd whisperx_runner && bash setup.sh"
            ),
            // Stable token consumed by the frontend to show the localised timeout message.
            Self::TranscriptionTimeout => {
                write!(f, "{}", crate::transcribe::TRANSCRIPTION_TIMEOUT_ERROR)
            }
            Self::TranscriptionFailed(msg) => write!(f, "Transcription failed: {msg}"),
            Self::SummarizationFailed(msg) => write!(f, "Summarization failed: {msg}"),
            Self::DatabaseError(msg) => write!(f, "Database error: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::ValidationError(msg) => write!(f, "{msg}"),
            Self::MeetingNotFound(id) => write!(f, "Meeting not found: {id}"),
            Self::TaskError(msg) => write!(f, "Task error: {msg}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Allows `?` propagation from `AppError` into `Result<T, String>` return types
/// (which Tauri commands require).
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_correctly() {
        assert_eq!(
            AppError::SessionNotFound("abc".into()).to_string(),
            "Session not found: abc"
        );
        assert_eq!(AppError::StateLocked.to_string(), "Internal state lock failed");
        assert_eq!(AppError::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn converts_to_string() {
        let s: String = AppError::NoMicrophone.into();
        assert_eq!(s, "No microphone found");
    }

    #[test]
    fn all_variants_display_non_empty() {
        // Ensure every variant produces a non-empty Display string,
        // so the frontend never receives a blank error message.
        let variants: Vec<AppError> = vec![
            AppError::StateLocked,
            AppError::SessionNotFound("id-1".into()),
            AppError::NoMicrophone,
            AppError::AudioNotFound("/tmp/test.wav".into()),
            AppError::InvalidAudioPath,
            AppError::WhisperxUnavailable,
            AppError::TranscriptionTimeout,
            AppError::TranscriptionFailed("some detail".into()),
            AppError::SummarizationFailed("connection refused".into()),
            AppError::DatabaseError("table not found".into()),
            AppError::IoError("permission denied".into()),
            AppError::ValidationError("must not be empty".into()),
            AppError::MeetingNotFound("id-2".into()),
            AppError::TaskError("join failed".into()),
            AppError::Cancelled,
        ];
        for v in variants {
            let s = v.to_string();
            assert!(!s.is_empty(), "Display for {:?} must not be empty", s);
        }
    }

    #[test]
    fn transcription_timeout_matches_backend_token() {
        // The frontend relies on this exact string to show the localised timeout message.
        let s = AppError::TranscriptionTimeout.to_string();
        assert_eq!(s, crate::transcribe::TRANSCRIPTION_TIMEOUT_ERROR);
    }

    #[test]
    fn validation_error_shows_inner_message_only() {
        // ValidationError should NOT prefix with "Validation error:" — it forwards the
        // message as-is so the frontend can display it directly.
        let msg = "Timeout must be a positive number (seconds)";
        assert_eq!(AppError::ValidationError(msg.into()).to_string(), msg);
    }

    #[test]
    fn from_apperror_into_result_err() {
        // Simulate what Tauri command handlers do: return Err(AppError::X.into()).
        fn example() -> Result<(), String> {
            Err(AppError::MeetingNotFound("m-42".into()).into())
        }
        let err = example().unwrap_err();
        assert!(err.contains("m-42"));
        assert!(err.contains("Meeting not found"));
    }

    #[test]
    fn display_all_parameterized_variants_include_context() {
        // Variants that carry a String parameter should include it in Display output.
        let pairs: Vec<(AppError, &str)> = vec![
            (AppError::SessionNotFound("sess-99".into()), "sess-99"),
            (AppError::AudioNotFound("/tmp/x.wav".into()), "/tmp/x.wav"),
            (AppError::TranscriptionFailed("timeout".into()), "timeout"),
            (AppError::SummarizationFailed("refused".into()), "refused"),
            (AppError::DatabaseError("locked".into()), "locked"),
            (AppError::IoError("not found".into()), "not found"),
            (AppError::MeetingNotFound("id-abc".into()), "id-abc"),
            (AppError::TaskError("panicked".into()), "panicked"),
        ];
        for (err, expected_fragment) in pairs {
            let s = err.to_string();
            assert!(
                s.contains(expected_fragment),
                "Display of {:?} should contain '{}', got '{}'",
                expected_fragment,
                expected_fragment,
                s
            );
        }
    }

    #[test]
    fn display_special_chars_in_error_message() {
        let msg = "path contains \"quotes\" and 'apostrophes'";
        let err = AppError::IoError(msg.into());
        let s = err.to_string();
        assert!(s.contains(msg));
    }

    #[test]
    fn display_empty_inner_message() {
        let err = AppError::TranscriptionFailed(String::new());
        let s = err.to_string();
        assert!(s.contains("Transcription failed:"));
    }
}
