//! Meeting domain types (mirrors `src/types/index.ts`).

use serde::{Deserialize, Serialize};

/// Calculates meeting duration in seconds from a slice of segments with `start` and `end` fields.
/// Returns at least 1 second if segments are present, 0 if empty.
pub fn calculate_duration_seconds(segments: &[crate::transcribe::DiarizedSegment]) -> u32 {
    if segments.is_empty() {
        return 0;
    }
    let start = segments
        .first()
        .map(|s| s.start)
        .unwrap_or(0.0)
        .max(0.0);
    let end = segments
        .last()
        .map(|s| s.end)
        .unwrap_or(0.0)
        .max(0.0);
    ((end - start).max(0.0).ceil() as u32).max(1)
}

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
    /// Prompt template version at generation time — allows detecting stale analysis when templates change.
    /// Populated from `templates::TEMPLATE_VERSION`; empty string for legacy records without this field.
    #[serde(default)]
    pub template_version: String,
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
            template_version: crate::templates::TEMPLATE_VERSION.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_stores_meeting_type() {
        let p = MeetingOutput::placeholder("legal");
        assert_eq!(p.template_used, "legal");
    }

    #[test]
    fn placeholder_model_is_none() {
        let p = MeetingOutput::placeholder("consulting");
        assert_eq!(p.model_used, "none");
    }

    #[test]
    fn placeholder_summary_contains_fallback_text() {
        let p = MeetingOutput::placeholder("internal");
        assert!(
            p.summary_short.contains("not available"),
            "Fallback summary should mention unavailability"
        );
    }

    #[test]
    fn placeholder_generated_at_is_nonempty_rfc3339() {
        let p = MeetingOutput::placeholder("consulting");
        assert!(!p.generated_at.is_empty());
        // RFC3339 contains 'T' separator and timezone.
        assert!(p.generated_at.contains('T'));
    }

    #[test]
    fn placeholder_collections_are_empty() {
        let p = MeetingOutput::placeholder("consulting");
        assert!(p.topics.is_empty());
        assert!(p.decisions.is_empty());
        assert!(p.action_items.is_empty());
        assert!(p.participants_mentioned.is_empty());
    }
}
