//! Centralised setting defaults — single source of truth for both Rust backend and
//! the React frontend (exposed via `get_setting_defaults` command).

use serde::Serialize;

/// Default Ollama API endpoint (localhost, no external calls).
pub const OLLAMA_URL: &str = "http://localhost:11434";

/// Default LLM model for meeting summarization.
pub const LLM_MODEL: &str = "llama3.1:8b";

/// Default meeting type shown in selectors.
pub const DEFAULT_MEETING_TYPE: &str = "consulting";

/// Default meeting language for WhisperX transcription.
pub const MEETING_LANGUAGE: &str = "de";

/// Whether to keep WAV files after processing (default: off for privacy).
pub const RETAIN_AUDIO: &str = "false";

/// Default data retention period in days (1 year).
pub const RETENTION_DAYS: &str = "365";

/// Default UI language.
pub const UI_LANGUAGE: &str = "de";

/// Default WhisperX subprocess timeout in seconds (15 minutes).
pub const WHISPERX_TIMEOUT_SECS: &str = "900";

/// Heuristic threshold (seconds) after which the processing step hint switches
/// from "transcribing" to "summarizing" in the frontend.
pub const PROCESSING_STEP_HINT_SECS: u64 = 8;

/// All persisted settings with their default values, serialisable so the frontend
/// can retrieve them via a single Tauri command instead of duplicating the list.
#[derive(Serialize, Clone, Debug)]
pub struct SettingDefaults {
    pub ollama_url: &'static str,
    pub llm_model: &'static str,
    pub default_meeting_type: &'static str,
    pub meeting_language: &'static str,
    pub retain_audio: &'static str,
    pub retention_days: &'static str,
    pub ui_language: &'static str,
    pub whisperx_timeout_secs: &'static str,
    /// Exposed so the frontend step-hint timer uses the same constant.
    pub processing_step_hint_secs: u64,
}

/// Canonical defaults instance.
pub const DEFAULTS: SettingDefaults = SettingDefaults {
    ollama_url: OLLAMA_URL,
    llm_model: LLM_MODEL,
    default_meeting_type: DEFAULT_MEETING_TYPE,
    meeting_language: MEETING_LANGUAGE,
    retain_audio: RETAIN_AUDIO,
    retention_days: RETENTION_DAYS,
    ui_language: UI_LANGUAGE,
    whisperx_timeout_secs: WHISPERX_TIMEOUT_SECS,
    processing_step_hint_secs: PROCESSING_STEP_HINT_SECS,
};
