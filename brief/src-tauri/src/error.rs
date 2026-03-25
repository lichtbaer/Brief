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
}
