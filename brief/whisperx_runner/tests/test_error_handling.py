"""Subprocess tests: invalid WAV path yields JSON error and exit code 1."""

import json
import subprocess
import sys
from pathlib import Path


def _runner_script() -> Path:
    return Path(__file__).resolve().parent.parent / "whisperx_runner.py"


def test_missing_wav_path_returns_error_json_and_exit_1():
    script = _runner_script()
    bogus = "/nonexistent/brief_test_audio_7c4e9d2a.wav"
    proc = subprocess.run(
        [sys.executable, str(script), bogus],
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 1
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload
    assert isinstance(payload["error"], str)
    assert len(payload["error"]) > 0
