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
#[derive(Deserialize, Serialize, Clone, Debug)]
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
    // Priority 1: explicit override for development/testing — takes precedence over all other paths.
    if let Ok(p) = std::env::var("BRIEF_WHISPERX_RUNNER") {
        log::info!("WhisperX runner: using BRIEF_WHISPERX_RUNNER override: {p}");
        return p;
    }
    // Priority 2: bundled app resources — the production path for packaged macOS/Linux builds.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundled = parent.join("../Resources/whisperx_runner/whisperx_runner.py");
            if bundled.exists() {
                let p = bundled.to_string_lossy().to_string();
                log::info!("WhisperX runner: using bundled resource: {p}");
                return p;
            }
        }
    }
    // Priority 3: source-tree fallback for `cargo run` / unit tests outside the packaged bundle.
    let dev_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../whisperx_runner/whisperx_runner.py")
        .to_string_lossy()
        .to_string();
    log::info!("WhisperX runner: using dev fallback path: {dev_path}");
    dev_path
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
        let python_bin =
            python_bin.unwrap_or_else(|| default_python_bin_for_runner(&runner_script));
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

    /// Sets the WhisperX `language` argument (ISO 639-1 code, e.g. `de`, `en`). Loaded from `meeting_language` in app settings, not from UI locale.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        let s = language.into();
        let trimmed = s.trim();
        self.language = if trimmed.is_empty() {
            "de".to_string()
        } else {
            trimmed.to_string()
        };
        self
    }

    /// Runs the WhisperX subprocess on `audio_path`, reads stdout as JSON, and returns segments or a stable timeout token ([`TRANSCRIPTION_TIMEOUT_ERROR`]).
    pub fn transcribe(&self, audio_path: &Path) -> Result<WhisperXOutput, String> {
        let audio_str = audio_path
            .to_str()
            .ok_or_else(|| "Audio path is not valid UTF-8".to_string())?;

        log::info!(
            "Starting transcription: audio={} language={} model={} timeout={}s",
            audio_str,
            self.language,
            self.model_size,
            self.timeout_secs
        );

        let mut child = Command::new(&self.python_bin)
            .arg(&self.runner_script)
            .arg(audio_str)
            .arg(&self.language)
            .arg(&self.model_size)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("WhisperX process could not be started: {}", e))?;

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
            // Forward each stderr line as a debug log so progress messages appear in app logs.
            for line in s.lines() {
                log::debug!("whisperx stderr: {}", line);
            }
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
                log::warn!(
                    "Transcription timed out after {}s for audio={}",
                    self.timeout_secs,
                    audio_str
                );
                if let Err(e) = child.kill() {
                    log::error!("Failed to kill timed-out WhisperX process: {}", e);
                }
                if let Err(e) = child.wait() {
                    log::error!("Failed to reap timed-out WhisperX process: {}", e);
                }
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
                            return Err(format!("WhisperX error: {}", err.error));
                        }
                        return Err(format!(
                            "WhisperX error (exit code {}): stdout={} stderr={}",
                            status.code().unwrap_or(-1),
                            stdout,
                            stderr_str
                        ));
                    }

                    if let Ok(err) = serde_json::from_str::<WhisperXError>(&stdout) {
                        return Err(format!("WhisperX error: {}", err.error));
                    }

                    let elapsed = start.elapsed().as_secs_f32();
                    log::info!("Transcription finished in {:.1}s", elapsed);
                    return serde_json::from_str::<WhisperXOutput>(&stdout).map_err(|e| {
                        format!("WhisperX output could not be parsed: {} — raw: {}", e, stdout)
                    });
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(e) => {
                    if let Err(kill_err) = child.kill() {
                        log::error!("Failed to kill WhisperX process after error: {}", kill_err);
                    }
                    if let Err(wait_err) = child.wait() {
                        log::error!("Failed to reap WhisperX process after error: {}", wait_err);
                    }
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(format!("WhisperX process error: {}", e));
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
    use super::{Transcriber, WhisperXError, WhisperXOutput, TRANSCRIPTION_TIMEOUT_ERROR};

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

    #[test]
    fn with_language_empty_defaults_to_de() {
        let t = Transcriber::new(None, None).with_language("");
        assert_eq!(t.language, "de");
    }

    #[test]
    fn with_language_trims_whitespace() {
        let t = Transcriber::new(None, None).with_language("  en  ");
        assert_eq!(t.language, "en");
    }

    #[test]
    fn with_language_accepts_any_code() {
        let t = Transcriber::new(None, None).with_language("fr");
        assert_eq!(t.language, "fr");
    }

    #[test]
    fn with_timeout_secs_clamps_zero_to_one() {
        let t = Transcriber::new(None, None).with_timeout_secs(0);
        assert_eq!(t.timeout_secs, 1);
    }

    #[test]
    fn with_timeout_secs_preserves_valid_value() {
        let t = Transcriber::new(None, None).with_timeout_secs(600);
        assert_eq!(t.timeout_secs, 600);
    }

    // -- JSON output parsing --

    #[test]
    fn parse_success_json_output() {
        // Verify that the WhisperXOutput deserializer correctly maps the canonical success payload.
        let json = r#"{
            "segments": [
                {"speaker":"SPEAKER_00","start":0.0,"end":1.5,"text":"Hello world"},
                {"speaker":"SPEAKER_01","start":1.5,"end":3.0,"text":"Goodbye"}
            ],
            "language": "en"
        }"#;
        let out: WhisperXOutput = serde_json::from_str(json).expect("should parse");
        assert_eq!(out.segments.len(), 2);
        assert_eq!(out.segments[0].speaker, "SPEAKER_00");
        assert_eq!(out.segments[0].text, "Hello world");
        assert!((out.segments[1].start - 1.5).abs() < f64::EPSILON);
        assert_eq!(out.language, "en");
    }

    #[test]
    fn parse_error_json_distinguishes_from_success() {
        // A WhisperXError payload must deserialize as an error, not as WhisperXOutput.
        let error_json = r#"{"error": "File not found: /tmp/missing.wav"}"#;
        let as_success = serde_json::from_str::<WhisperXOutput>(error_json);
        // Missing `segments` field means this should fail to parse as WhisperXOutput.
        assert!(
            as_success.is_err(),
            "error JSON must not parse as WhisperXOutput"
        );

        let as_error = serde_json::from_str::<WhisperXError>(error_json);
        assert!(as_error.is_ok());
        assert_eq!(as_error.unwrap().error, "File not found: /tmp/missing.wav");
    }

    #[test]
    fn transcription_timeout_error_constant_is_stable() {
        // The token must remain stable; the frontend matches it by string equality.
        assert_eq!(
            TRANSCRIPTION_TIMEOUT_ERROR,
            "BRIEF_ERR_TRANSCRIPTION_TIMEOUT"
        );
    }
}
