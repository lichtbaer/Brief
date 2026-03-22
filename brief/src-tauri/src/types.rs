//! Meeting domain types (mirrors `src/types/index.ts`).

use serde::{Deserialize, Serialize};

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
