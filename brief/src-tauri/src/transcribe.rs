use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Serialize, Clone)]
pub struct DiarizedSegment {
    pub speaker: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Deserialize)]
pub struct WhisperXOutput {
    pub segments: Vec<DiarizedSegment>,
    pub language: String,
}

#[derive(Deserialize)]
struct WhisperXError {
    error: String,
}

pub struct Transcriber {
    pub python_bin: String,
    pub runner_script: String,
    pub model_size: String,
    pub language: String,
}

fn default_runner_script() -> String {
    let dev_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../whisperx_runner/whisperx_runner.py");
    if dev_path.exists() {
        return dev_path.to_string_lossy().to_string();
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent().map(|p| {
                p.join("../Resources/whisperx_runner/whisperx_runner.py")
                    .to_string_lossy()
                    .to_string()
            })
        })
        .unwrap_or_else(|| dev_path.to_string_lossy().to_string())
}

impl Transcriber {
    pub fn new(python_bin: Option<String>, runner_script: Option<String>) -> Self {
        Transcriber {
            python_bin: python_bin.unwrap_or_else(|| "python3".to_string()),
            runner_script: runner_script.unwrap_or_else(default_runner_script),
            model_size: "base".to_string(),
            language: "de".to_string(),
        }
    }

    pub fn transcribe(&self, audio_path: &Path) -> Result<WhisperXOutput, String> {
        let audio_str = audio_path
            .to_str()
            .ok_or_else(|| "Ungültiger Audio-Pfad".to_string())?;
        let output = Command::new(&self.python_bin)
            .arg(&self.runner_script)
            .arg(audio_str)
            .arg(&self.language)
            .arg(&self.model_size)
            .output()
            .map_err(|e| format!("WhisperX konnte nicht gestartet werden: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(format!(
                "WhisperX-Fehler (Exit {}): {}",
                output.status.code().unwrap_or(-1),
                if stderr.is_empty() {
                    stdout.as_ref()
                } else {
                    stderr.as_ref()
                }
            ));
        }

        if let Ok(err) = serde_json::from_str::<WhisperXError>(&stdout) {
            return Err(format!("WhisperX-Fehler: {}", err.error));
        }

        serde_json::from_str::<WhisperXOutput>(&stdout)
            .map_err(|e| format!("WhisperX-Output nicht parsbar: {} — Output: {}", e, stdout))
    }

    pub fn check_available(&self) -> bool {
        Command::new(&self.python_bin)
            .arg("-c")
            .arg("import whisperx")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
