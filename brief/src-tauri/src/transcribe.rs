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
    if let Ok(p) = std::env::var("BRIEF_WHISPERX_RUNNER") {
        return p;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundled = parent.join("../Resources/whisperx_runner/whisperx_runner.py");
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../whisperx_runner/whisperx_runner.py")
        .to_string_lossy()
        .to_string()
}

/// Prefer the venv created by `whisperx_runner/setup.sh` (`.venv/` next to the runner script).
fn resolve_venv_python_adjacent_to_runner(runner_script: &str) -> Option<String> {
    let runner = Path::new(runner_script);
    let whisperx_dir: PathBuf = runner.parent()?.to_path_buf();

    let unix_venv = whisperx_dir.join(".venv/bin/python");
    if unix_venv.exists() {
        return Some(unix_venv.to_string_lossy().to_string());
    }

    #[cfg(windows)]
    {
        let win_venv = whisperx_dir.join(".venv/Scripts/python.exe");
        if win_venv.exists() {
            return Some(win_venv.to_string_lossy().to_string());
        }
    }

    None
}

fn default_python_bin_for_runner(runner_script: &str) -> String {
    resolve_venv_python_adjacent_to_runner(runner_script).unwrap_or_else(|| "python3".to_string())
}

impl Transcriber {
    pub fn new(python_bin: Option<String>, runner_script: Option<String>) -> Self {
        let runner_script = runner_script.unwrap_or_else(default_runner_script);
        let python_bin = python_bin.unwrap_or_else(|| default_python_bin_for_runner(&runner_script));
        Transcriber {
            python_bin,
            runner_script,
            model_size: "base".to_string(),
            language: "de".to_string(),
        }
    }

    pub fn transcribe(&self, audio_path: &Path) -> Result<WhisperXOutput, String> {
        let audio_str = audio_path.to_str().ok_or_else(|| {
            "Audio-Pfad ist nicht als UTF-8 darstellbar".to_string()
        })?;

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
            if let Ok(err) = serde_json::from_str::<WhisperXError>(&stdout) {
                return Err(format!("WhisperX-Fehler: {}", err.error));
            }
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

        serde_json::from_str::<WhisperXOutput>(&stdout).map_err(|e| {
            format!(
                "WhisperX-Output nicht parsbar: {} — Output: {}",
                e, stdout
            )
        })
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
