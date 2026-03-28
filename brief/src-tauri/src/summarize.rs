use rand::Rng as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::types::MeetingOutput;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_LLM_MODEL: &str = "llama3.1:8b";

/// Default number of retry attempts on transient network errors.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default initial backoff delay in milliseconds (doubles with each retry).
const DEFAULT_RETRY_BACKOFF_MS: u64 = 2000;

/// Maximum backoff cap in milliseconds to prevent excessively long waits.
const MAX_BACKOFF_MS: u64 = 30_000;

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    format: String,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageResponse,
}

#[derive(Deserialize)]
struct OllamaMessageResponse {
    content: String,
}

pub struct Summarizer {
    client: Client,
    ollama_url: String,
    model: String,
    /// Maximum number of retry attempts on transient network errors (not JSON parse errors).
    max_retries: u32,
    /// Initial backoff in milliseconds; doubles with each retry attempt.
    retry_backoff_ms: u64,
}

impl Summarizer {
    /// Creates a new Summarizer.
    /// `timeout_secs` controls the HTTP client timeout (default 300 if None).
    /// A configurable timeout is important: 300s may be too short for long meetings on slow hardware.
    pub fn new(
        ollama_url: Option<String>,
        model: Option<String>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, String> {
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(300));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;
        Ok(Summarizer {
            client,
            ollama_url: ollama_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            model: model.unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string()),
            max_retries: DEFAULT_MAX_RETRIES,
            retry_backoff_ms: DEFAULT_RETRY_BACKOFF_MS,
        })
    }

    /// Configures retry behaviour. Call this after `new()` before `summarize()`.
    /// `max_retries` is the number of additional attempts (0 = single try, no retry).
    /// `backoff_ms` is the initial delay; it doubles with each successive attempt.
    pub fn with_retry_config(mut self, max_retries: u32, backoff_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.retry_backoff_ms = backoff_ms;
        self
    }

    pub async fn check_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.ollama_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Summarize a transcript using the given template prompt, with automatic retry on transient
    /// network errors. JSON parse failures are not retried (the model returned malformed output
    /// and a second attempt is unlikely to improve the result).
    /// Returns a MeetingOutput or an error string after all retries are exhausted.
    pub async fn summarize(
        &self,
        transcript: &str,
        system_prompt: &str,
        meeting_type: &str,
    ) -> Result<MeetingOutput, String> {
        let mut last_err = String::new();

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                // Exponential backoff with ±20% jitter and a hard cap to prevent thundering-herd
                // when multiple concurrent clients retry at the same moment.
                let base_ms = self.retry_backoff_ms * (1u64 << (attempt - 1));
                let capped_ms = base_ms.min(MAX_BACKOFF_MS);
                let jitter_ms: u64 = rand::thread_rng().gen_range(0..=(capped_ms / 5));
                let delay_ms = capped_ms + jitter_ms;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            match self.attempt_summarize(transcript, system_prompt, meeting_type).await {
                Ok(output) => return Ok(output),
                Err(e) => {
                    // Do not retry JSON parse errors: the model returned bad JSON and retrying
                    // is unlikely to produce valid output from the same prompt.
                    if is_parse_error(&e) || attempt == self.max_retries {
                        return Err(e);
                    }
                    last_err = e;
                }
            }
        }

        // Unreachable in practice (loop always returns), but satisfies the compiler.
        Err(last_err)
    }

    /// Single HTTP call to Ollama — extracted so the retry loop in `summarize()` is clean.
    async fn attempt_summarize(
        &self,
        transcript: &str,
        system_prompt: &str,
        meeting_type: &str,
    ) -> Result<MeetingOutput, String> {
        // The transcript is passed as a separate user message — not embedded in the system prompt.
        // Clear delimiters mark the transcript boundary to reduce prompt-injection risk from
        // adversarial content in the meeting audio.
        let user_content = format!(
            "---BEGIN TRANSCRIPT---\n{}\n---END TRANSCRIPT---",
            transcript
        );

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            stream: false,
            format: "json".to_string(),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|_| "Ollama not reachable — is `ollama serve` running?".to_string())?;

        if !response.status().is_success() {
            return Err(format!("Ollama error: HTTP {}", response.status()));
        }

        let result: OllamaChatResponse = response
            .json()
            .await
            .map_err(|_| "Ollama response could not be parsed".to_string())?;

        parse_meeting_output(&result.message.content, meeting_type, &self.model)
    }
}

/// Returns true when the error originates from JSON parsing rather than from a network or HTTP
/// failure. Parse errors should not be retried — the model returned malformed output.
fn is_parse_error(err: &str) -> bool {
    err.contains("JSON parse error")
}

fn parse_meeting_output(
    json_str: &str,
    meeting_type: &str,
    model_used: &str,
) -> Result<MeetingOutput, String> {
    let cleaned = json_str
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let value: serde_json::Value = serde_json::from_str(cleaned).map_err(|e| {
        let preview_len = cleaned.len().min(200);
        let preview = &cleaned[..preview_len];
        format!("JSON parse error: {} — Raw: {}", e, preview)
    })?;

    let now = chrono::Utc::now().to_rfc3339();

    let follow_up = value
        .get("follow_up_draft")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    Ok(MeetingOutput {
        summary_short: value["summary_short"].as_str().unwrap_or("").to_string(),
        topics: value["topics"].as_array().cloned().unwrap_or_default(),
        decisions: value["decisions"].as_array().cloned().unwrap_or_default(),
        action_items: value["action_items"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        follow_up_draft: follow_up,
        participants_mentioned: value["participants_mentioned"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        template_used: meeting_type.to_string(),
        model_used: model_used.to_string(),
        generated_at: now,
        template_version: crate::templates::TEMPLATE_VERSION.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_meeting_output;

    #[test]
    fn parse_strips_json_fence() {
        let raw = r#"```json
{"summary_short":"Hello","topics":[],"decisions":[],"action_items":[],"follow_up_draft":{},"participants_mentioned":[]}
```"#;
        let out = parse_meeting_output(raw, "consulting", "llama3.1:8b").unwrap();
        assert_eq!(out.summary_short, "Hello");
        assert!(out.topics.is_empty());
    }

    #[test]
    fn parse_partial_fields_use_defaults() {
        let raw = r#"{"summary_short":"Only summary"}"#;
        let out = parse_meeting_output(raw, "internal", "m").unwrap();
        assert_eq!(out.summary_short, "Only summary");
        assert!(out.topics.is_empty());
        assert!(out.decisions.is_empty());
        assert_eq!(out.template_used, "internal");
        assert_eq!(out.model_used, "m");
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let raw = "not valid json at all";
        let result = parse_meeting_output(raw, "consulting", "m");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("JSON parse error"), "Got: {err}");
    }

    #[test]
    fn parse_empty_json_object() {
        let raw = "{}";
        let out = parse_meeting_output(raw, "legal", "llama3.1:8b").unwrap();
        assert_eq!(out.summary_short, "");
        assert!(out.topics.is_empty());
        assert!(out.decisions.is_empty());
        assert!(out.action_items.is_empty());
        assert!(out.participants_mentioned.is_empty());
        assert_eq!(out.template_used, "legal");
        assert_eq!(out.model_used, "llama3.1:8b");
        // generated_at should be a non-empty RFC3339 timestamp.
        assert!(!out.generated_at.is_empty());
    }

    #[test]
    fn parse_strips_triple_backtick_without_json_tag() {
        let raw = r#"```
{"summary_short":"Backtick only"}
```"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.summary_short, "Backtick only");
    }

    #[test]
    fn parse_extracts_participants() {
        let raw = r#"{"summary_short":"","participants_mentioned":["Alice","Bob",""]}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        // Empty strings are filtered by the frontend, but the parser preserves them.
        assert_eq!(out.participants_mentioned.len(), 3);
        assert_eq!(out.participants_mentioned[0], "Alice");
    }

    #[test]
    fn parse_preserves_follow_up_draft() {
        let raw = r#"{"summary_short":"s","follow_up_draft":{"subject":"Re: Meeting","full_text":"Hi all"}}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.follow_up_draft["subject"], "Re: Meeting");
        assert_eq!(out.follow_up_draft["full_text"], "Hi all");
    }

    #[test]
    fn parse_with_action_items() {
        let raw = r#"{"summary_short":"s","action_items":[{"description":"Send report","owner":"Alice","due_date":"2025-04-01","priority":"high"}]}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.action_items.len(), 1);
        assert_eq!(out.action_items[0]["description"], "Send report");
        assert_eq!(out.action_items[0]["owner"], "Alice");
        assert_eq!(out.action_items[0]["priority"], "high");
    }

    #[test]
    fn parse_with_whitespace_around_json() {
        let raw = r#"

  {"summary_short":"Padded"}

"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.summary_short, "Padded");
    }

    #[test]
    fn parse_null_summary_short_becomes_empty() {
        let raw = r#"{"summary_short":null}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.summary_short, "");
    }

    #[test]
    fn parse_unicode_emoji_in_participants() {
        let raw = r#"{"summary_short":"s","participants_mentioned":["Ünsal 🧑‍💻","José"]}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.participants_mentioned.len(), 2);
        assert!(out.participants_mentioned[0].contains("Ünsal"));
    }

    #[test]
    fn parse_generated_at_is_rfc3339() {
        let raw = r#"{"summary_short":"s"}"#;
        let out = parse_meeting_output(raw, "internal", "m").unwrap();
        assert!(out.generated_at.contains('T'), "Should be RFC3339");
        assert!(!out.generated_at.is_empty());
    }

    #[test]
    fn parse_model_and_template_passthrough() {
        let raw = r#"{"summary_short":"s"}"#;
        let out = parse_meeting_output(raw, "legal", "llama3.1:70b").unwrap();
        assert_eq!(out.template_used, "legal");
        assert_eq!(out.model_used, "llama3.1:70b");
    }

    #[test]
    fn parse_topics_with_nested_objects() {
        let raw = r#"{"summary_short":"s","topics":[{"title":"T1","summary":"S1","extra_field":"ignored"}]}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.topics.len(), 1);
        assert_eq!(out.topics[0]["title"], "T1");
    }

    #[test]
    fn parse_decisions_with_context() {
        let raw = r#"{"summary_short":"s","decisions":[{"description":"D1","context":"Approved by CEO"}]}"#;
        let out = parse_meeting_output(raw, "consulting", "m").unwrap();
        assert_eq!(out.decisions.len(), 1);
        assert_eq!(out.decisions[0]["context"], "Approved by CEO");
    }

    // -- is_parse_error --

    #[test]
    fn is_parse_error_detects_json_parse_string() {
        assert!(is_parse_error("JSON parse error: unexpected token"));
        assert!(is_parse_error("JSON parse error — Raw: {bad}"));
    }

    #[test]
    fn is_parse_error_ignores_network_errors() {
        assert!(!is_parse_error("Ollama not reachable — is `ollama serve` running?"));
        assert!(!is_parse_error("Ollama error: HTTP 503"));
        assert!(!is_parse_error("connection refused"));
    }

    // -- with_retry_config builder --

    #[test]
    fn with_retry_config_overrides_defaults() {
        let s = super::Summarizer::new(None, None)
            .unwrap()
            .with_retry_config(5, 500);
        assert_eq!(s.max_retries, 5);
        assert_eq!(s.retry_backoff_ms, 500);
    }
}
