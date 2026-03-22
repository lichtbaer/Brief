//! Meeting domain types (mirrors `src/types/index.ts`).

use serde::{Deserialize, Serialize};

/// Settings + memory snapshot for the React shell (onboarding, settings screen).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsSnapshot {
    pub memory_gb: f64,
    pub recommended_model: String,
    pub llm_model: String,
    pub llm_model_user_override: bool,
    pub show_low_ram_onboarding: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Meeting {
    pub id: String,
    pub created_at: String,
    pub ended_at: String,
    pub duration_seconds: u32,
    pub meeting_type: String,
    pub title: String,
    pub transcript: String,
    pub output: MeetingOutput,
    pub audio_path: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MeetingOutput {
    pub summary_short: String,
    pub topics: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub action_items: Vec<serde_json::Value>,
    pub follow_up_draft: serde_json::Value,
    pub participants_mentioned: Vec<String>,
    pub template_used: String,
    pub model_used: String,
    pub generated_at: String,
}

impl MeetingOutput {
    /// Creates a placeholder output when summarization is unavailable.
    pub fn placeholder(meeting_type: &str) -> Self {
        MeetingOutput {
            summary_short: "Summarization not available — transcript saved.".to_string(),
            topics: vec![],
            decisions: vec![],
            action_items: vec![],
            follow_up_draft: serde_json::json!({}),
            participants_mentioned: vec![],
            template_used: meeting_type.to_string(),
            model_used: "none".to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}
