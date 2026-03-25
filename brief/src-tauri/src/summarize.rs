use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::types::MeetingOutput;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_LLM_MODEL: &str = "llama3.1:8b";

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
}

impl Summarizer {
    pub fn new(ollama_url: Option<String>, model: Option<String>) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("HTTP-Client-Fehler: {}", e))?;
        Ok(Summarizer {
            client,
            ollama_url: ollama_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            model: model.unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string()),
        })
    }

    pub async fn check_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.ollama_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Summarize a transcript using the given template prompt.
    /// Returns a MeetingOutput or an error string.
    pub async fn summarize(
        &self,
        transcript: &str,
        system_prompt: &str,
        meeting_type: &str,
    ) -> Result<MeetingOutput, String> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: format!("TRANSCRIPT:\n{}", transcript),
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
            .map_err(|e| format!("Ollama not reachable: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Ollama error: HTTP {}", response.status()));
        }

        let result: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| format!("Ollama response not parsable: {}", e))?;

        parse_meeting_output(&result.message.content, meeting_type, &self.model)
    }
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
}
