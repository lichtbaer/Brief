//! WhisperX subprocess integration: spawn Python runner, parse JSON segments, enforce timeout.
//!
//! The Rust side avoids embedding ML; it delegates to `whisperx_runner/whisperx_runner.py` and maps stdout to [`WhisperXOutput`].

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Stable token returned to the frontend when transcription exceeds the configured timeout.
pub const TRANSCRIPTION_TIMEOUT_ERROR: &str = "BRIEF_ERR_TRANSCRIPTION_TIMEOUT";

pub const DEFAULT_WHISPERX_TIMEOUT_SECS: u64 = 900;

/// One timed utterance with a diarized speaker label and transcript text.
#[derive(Deserialize, Serialize, Clone)]
pub struct DiarizedSegment {
    pub speaker: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Successful JSON payload from the WhisperX runner (`segments` + detected `language`).
#[derive(Deserialize)]
pub struct WhisperXOutput {
    pub segments: Vec<DiarizedSegment>,
    #[allow(dead_code)]
    pub language: String,
}

#[derive(Deserialize)]
struct WhisperXError {
    error: String,
}

/// Configuration for running the external WhisperX Python script (`python`, script path, model, language, timeout).
pub struct Transcriber {
    pub python_bin: String,
    pub runner_script: String,
    pub model_size: String,
    pub language: String,
    pub timeout_secs: u64,
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
    /// Builds a transcriber: resolves runner path (`BRIEF_WHISPERX_RUNNER` or dev path next to crate), prefers `.venv` Python when present.
    pub fn new(python_bin: Option<String>, runner_script: Option<String>) -> Self {
        let runner_script = runner_script.unwrap_or_else(default_runner_script);
        let python_bin = python_bin.unwrap_or_else(|| default_python_bin_for_runner(&runner_script));
        Transcriber {
            python_bin,
            runner_script,
            model_size: "base".to_string(),
            language: "de".to_string(),
            timeout_secs: DEFAULT_WHISPERX_TIMEOUT_SECS,
        }
    }

    /// Sets the maximum wall-clock wait for the WhisperX child process (`secs` clamped to at least 1).
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs.max(1);
        self
    }

    /// Runs the WhisperX subprocess on `audio_path`, reads stdout as JSON, and returns segments or a stable timeout token ([`TRANSCRIPTION_TIMEOUT_ERROR`]).
    pub fn transcribe(&self, audio_path: &Path) -> Result<WhisperXOutput, String> {
        let audio_str = audio_path.to_str().ok_or_else(|| {
            "Audio-Pfad ist nicht als UTF-8 darstellbar".to_string()
        })?;

        let mut child = Command::new(&self.python_bin)
            .arg(&self.runner_script)
            .arg(audio_str)
            .arg(&self.language)
            .arg(&self.model_size)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("WhisperX konnte nicht gestartet werden: {}", e))?;

        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| "WhisperX stdout pipe missing".to_string())?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| "WhisperX stderr pipe missing".to_string())?;

        let stderr_handle = thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr_pipe.read_to_string(&mut s);
            s
        });

        let stdout_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });

        let timeout = Duration::from_secs(self.timeout_secs);
        let start = Instant::now();

        loop {
            if start.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(TRANSCRIPTION_TIMEOUT_ERROR.to_string());
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout_bytes = stdout_handle
                        .join()
                        .map_err(|_| "WhisperX stdout reader failed".to_string())?;
                    let stderr_str = stderr_handle.join().unwrap_or_default();

                    let stdout = String::from_utf8_lossy(&stdout_bytes);

                    if !status.success() {
                        if let Ok(err) = serde_json::from_str::<WhisperXError>(&stdout) {
                            return Err(format!("WhisperX-Fehler: {}", err.error));
                        }
                        return Err(format!(
                            "WhisperX-Fehler (Exit {}): {}",
                            status.code().unwrap_or(-1),
                            if stderr_str.is_empty() {
                                stdout.as_ref()
                            } else {
                                stderr_str.as_ref()
                            }
                        ));
                    }

                    if let Ok(err) = serde_json::from_str::<WhisperXError>(&stdout) {
                        return Err(format!("WhisperX-Fehler: {}", err.error));
                    }

                    return serde_json::from_str::<WhisperXOutput>(&stdout).map_err(|e| {
                        format!(
                            "WhisperX-Output nicht parsbar: {} — Output: {}",
                            e, stdout
                        )
                    });
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(format!("WhisperX-Prozessfehler: {}", e));
                }
            }
        }
    }

    /// Returns whether `python -c "import whisperx"` succeeds with the configured interpreter (quick env sanity check).
    pub fn check_available(&self) -> bool {
        Command::new(&self.python_bin)
            .arg("-c")
            .arg("import whisperx")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::Transcriber;

    #[test]
    fn check_available_false_when_python_binary_missing() {
        let t = Transcriber {
            python_bin: "/nonexistent/brief_test_python_9f3a2c1d".to_string(),
            runner_script: "/dev/null".to_string(),
            model_size: "base".to_string(),
            language: "de".to_string(),
            timeout_secs: 1,
        };
        assert!(!t.check_available());
    }
}
