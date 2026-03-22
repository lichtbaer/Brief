use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_WHISPER_MODEL: &str = "whisper";

#[derive(Serialize)]
struct OllamaTranscribeRequest {
    model: String,
    prompt: String, // base64-encoded audio
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct Transcriber {
    client: Client,
    ollama_url: String,
    whisper_model: String,
}

impl Transcriber {
    pub fn new(ollama_url: Option<String>, whisper_model: Option<String>) -> Self {
        Transcriber {
            client: Client::builder()
                .timeout(Duration::from_secs(300)) // 5 Min Timeout
                .build()
                .expect("reqwest Client"),
            ollama_url: ollama_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            whisper_model: whisper_model.unwrap_or_else(|| DEFAULT_WHISPER_MODEL.to_string()),
        }
    }

    pub async fn transcribe(&self, audio_path: &Path) -> Result<String, String> {
        let audio_bytes =
            std::fs::read(audio_path).map_err(|e| format!("WAV-Datei nicht lesbar: {}", e))?;
        let audio_b64 = general_purpose::STANDARD.encode(&audio_bytes);

        let request = OllamaTranscribeRequest {
            model: self.whisper_model.clone(),
            prompt: audio_b64,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama nicht erreichbar: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Ollama-Fehler: HTTP {}", response.status()));
        }

        let result: OllamaResponse = response
            .json()
            .await
            .map_err(|e| format!("Ollama-Antwort nicht parsbar: {}", e))?;

        if let Err(e) = std::fs::remove_file(audio_path) {
            eprintln!("WAV konnte nicht gelöscht werden: {}", e);
        }

        Ok(result.response)
    }

    pub async fn check_ollama_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.ollama_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
