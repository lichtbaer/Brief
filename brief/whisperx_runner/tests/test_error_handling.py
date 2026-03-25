"""Subprocess tests: invalid WAV path yields JSON error and exit code 1."""

import json
import subprocess
import sys
from pathlib import Path


def _runner_script() -> Path:
    return Path(__file__).resolve().parent.parent / "whisperx_runner.py"


def _run_runner(*args: str) -> subprocess.CompletedProcess:
    """Helper: invoke whisperx_runner.py with the given arguments."""
    script = _runner_script()
    return subprocess.run(
        [sys.executable, str(script), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def test_missing_wav_path_returns_error_json_and_exit_1():
    bogus = "/nonexistent/brief_test_audio_7c4e9d2a.wav"
    proc = _run_runner(bogus)
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload
    assert isinstance(payload["error"], str)
    assert len(payload["error"]) > 0


def test_no_arguments_returns_error_json_and_exit_1():
    """Calling the runner with no arguments should produce a JSON error."""
    proc = _run_runner()
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload
    assert len(payload["error"]) > 0


def test_error_output_is_valid_json():
    """Error output should always be parseable JSON with exactly an 'error' key."""
    proc = _run_runner("/nonexistent/does_not_exist.wav")
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert set(payload.keys()) == {"error"}


def test_missing_file_error_mentions_path():
    """The error message for a missing file should reference the file path."""
    bogus = "/tmp/brief_test_nonexistent_abc123.wav"
    proc = _run_runner(bogus)
    payload = json.loads(proc.stdout.strip())
    assert bogus in payload["error"]


def test_progress_goes_to_stderr_not_stdout():
    """Progress messages should go to stderr; stdout should be pure JSON."""
    bogus = "/nonexistent/brief_test_stderr_check.wav"
    proc = _run_runner(bogus)
    # stdout is JSON.
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload
    # stdout should not contain progress strings like "Loading" or "Transcribing".
    assert "Loading" not in proc.stdout
    assert "Transcribing" not in proc.stdout


def test_language_argument_accepted():
    """Passing a language argument should not change the error behaviour for missing files."""
    bogus = "/nonexistent/brief_test_lang.wav"
    proc = _run_runner(bogus, "en")
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload


def test_model_size_argument_accepted():
    """Passing all three arguments should not change error behaviour for missing files."""
    bogus = "/nonexistent/brief_test_model.wav"
    proc = _run_runner(bogus, "de", "base")
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload
