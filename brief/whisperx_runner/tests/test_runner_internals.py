"""Unit tests for whisperx_runner.py internals (no ML dependencies required)."""

import importlib
import json
import logging
import sys
import types
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

# Ensure the package directory is importable
_PKG_DIR = str(Path(__file__).resolve().parent.parent)
if _PKG_DIR not in sys.path:
    sys.path.insert(0, _PKG_DIR)


# ---------------------------------------------------------------------------
# Helper: import a fresh copy of whisperx_runner without cached state
# ---------------------------------------------------------------------------

def _import_runner():
    """Return a freshly imported whisperx_runner module."""
    if "whisperx_runner" in sys.modules:
        del sys.modules["whisperx_runner"]
    return importlib.import_module("whisperx_runner")


# ---------------------------------------------------------------------------
# Tests for _patch_torch_load()
# ---------------------------------------------------------------------------

def test_patch_torch_load_adds_weights_only_false():
    """_patch_torch_load() makes torch.load default to weights_only=False."""
    runner = _import_runner()

    # Build a fake torch module with a load function that records its kwargs
    recorded: list[dict] = []

    def fake_load(*args, **kwargs):
        recorded.append(kwargs)
        return None

    fake_torch = types.ModuleType("torch")
    fake_torch.load = fake_load

    with patch.dict(sys.modules, {"torch": fake_torch}):
        runner._patch_torch_load()
        # Call the patched version without weights_only — it should inject False
        fake_torch.load("some_file.pt")

    assert len(recorded) == 1
    assert recorded[0]["weights_only"] is False


def test_patch_torch_load_preserves_existing_weights_only():
    """_patch_torch_load() does NOT override weights_only when already specified."""
    runner = _import_runner()

    recorded: list[dict] = []

    def fake_load(*args, **kwargs):
        recorded.append(kwargs)
        return None

    fake_torch = types.ModuleType("torch")
    fake_torch.load = fake_load

    with patch.dict(sys.modules, {"torch": fake_torch}):
        runner._patch_torch_load()
        fake_torch.load("model.pt", weights_only=True)

    assert recorded[0]["weights_only"] is True


# ---------------------------------------------------------------------------
# Tests for logging setup and stderr/stdout separation
# ---------------------------------------------------------------------------

def test_logging_progress_goes_to_stderr_not_stdout(tmp_path):
    """Progress messages emitted by the runner must appear on stderr, not stdout."""
    from tests.conftest import run_script
    script = Path(__file__).resolve().parent.parent / "whisperx_runner.py"
    proc = run_script(script, "/nonexistent/path.wav")

    # stdout must remain pure JSON
    payload = json.loads(proc.stdout.strip())
    assert "error" in payload

    # stderr should contain the error log line (from logger.error)
    assert "ERROR" in proc.stderr or "error" in proc.stderr.lower() or len(proc.stderr) > 0


def test_no_progress_in_stdout_on_error():
    """Even when an error occurs, stdout must contain *only* a JSON object."""
    from tests.conftest import run_script
    script = Path(__file__).resolve().parent.parent / "whisperx_runner.py"
    proc = run_script(script, "/no/such/file.wav")

    # The entire stdout must be valid JSON — nothing else
    payload = json.loads(proc.stdout.strip())
    assert isinstance(payload, dict)


# ---------------------------------------------------------------------------
# Tests for main() with fully mocked ML stack
# ---------------------------------------------------------------------------

def _build_mock_whisperx(language: str = "de"):
    """Return a mock whisperx module that produces a minimal transcript."""
    mock_wx = MagicMock()

    fake_model = MagicMock()
    fake_model.transcribe.return_value = {
        "segments": [
            {"start": 0.0, "end": 1.0, "text": " Hallo Welt", "speaker": "SPEAKER_00"},
        ]
    }
    mock_wx.load_model.return_value = fake_model

    # load_audio returns a fake numpy-like array
    mock_wx.load_audio.return_value = [0.0] * 100

    # load_align_model returns (model, metadata)
    mock_wx.load_align_model.return_value = (MagicMock(), {"language": language})

    # align returns same structure with segments
    mock_wx.align.return_value = {
        "segments": [
            {"start": 0.0, "end": 1.0, "text": " Hallo Welt", "speaker": "SPEAKER_00"},
        ]
    }

    # assign_word_speakers returns final result
    mock_wx.assign_word_speakers.return_value = {
        "segments": [
            {"start": 0.0, "end": 1.0, "text": " Hallo Welt", "speaker": "SPEAKER_00"},
        ]
    }

    return mock_wx


def test_main_success_path_emits_json_to_stdout(tmp_path, capsys):
    """main() with a mocked whisperx stack prints valid JSON with segments+language to stdout."""
    runner = _import_runner()

    wav_path = tmp_path / "test.wav"
    wav_path.write_bytes(b"RIFF")  # Existence check only; whisperx.load_audio is mocked

    mock_wx = _build_mock_whisperx("de")

    # DiarizationPipeline is a class: DiarizationPipeline(device="cpu") → instance,
    # then instance(audio) → diarize_segments list.
    diarize_instance = MagicMock()
    diarize_instance.return_value = []  # calling the instance returns an empty segment list

    mock_diarize_pipeline = MagicMock(return_value=diarize_instance)

    fake_whisperx_diarize = types.ModuleType("whisperx.diarize")
    fake_whisperx_diarize.DiarizationPipeline = mock_diarize_pipeline

    with patch.object(sys, "argv", ["whisperx_runner.py", str(wav_path), "de", "base"]):
        with patch.dict(sys.modules, {
            "torch": MagicMock(),
            "whisperx": mock_wx,
            "whisperx.diarize": fake_whisperx_diarize,
        }):
            # Prevent sys.exit from terminating the test process
            with patch.object(sys, "exit"):
                runner.main()

    captured = capsys.readouterr()
    payload = json.loads(captured.out.strip())
    assert "segments" in payload
    assert "language" in payload
    assert payload["language"] == "de"
    assert len(payload["segments"]) == 1
    assert payload["segments"][0]["text"] == "Hallo Welt"


def test_main_import_error_emits_json_error(tmp_path, capsys):
    """main() catches ImportError from missing whisperx and emits JSON error to stdout."""
    runner = _import_runner()

    wav_path = tmp_path / "test.wav"
    wav_path.write_bytes(b"RIFF")

    with patch.object(sys, "argv", ["whisperx_runner.py", str(wav_path)]):
        # Simulate whisperx not being installed
        with patch.dict(sys.modules, {"whisperx": None, "whisperx.diarize": None}):
            with patch.object(sys, "exit"):
                runner.main()

    captured = capsys.readouterr()
    payload = json.loads(captured.out.strip())
    assert "error" in payload
    assert isinstance(payload["error"], str)
    assert len(payload["error"]) > 0


def test_main_logs_transcription_info_on_success(tmp_path, caplog):
    """main() emits INFO-level log messages during a successful transcription run."""
    runner = _import_runner()

    wav_path = tmp_path / "test.wav"
    wav_path.write_bytes(b"RIFF")

    mock_wx = _build_mock_whisperx("en")
    diarize_instance2 = MagicMock()
    diarize_instance2.return_value = []
    fake_whisperx_diarize = types.ModuleType("whisperx.diarize")
    fake_whisperx_diarize.DiarizationPipeline = MagicMock(return_value=diarize_instance2)

    with patch.object(sys, "argv", ["whisperx_runner.py", str(wav_path), "en"]):
        with patch.dict(sys.modules, {
            "torch": MagicMock(),
            "whisperx": mock_wx,
            "whisperx.diarize": fake_whisperx_diarize,
        }):
            with patch.object(sys, "exit"):
                with caplog.at_level(logging.INFO, logger="whisperx_runner"):
                    runner.main()

    # At least one INFO message must have been logged
    info_records = [r for r in caplog.records if r.levelno >= logging.INFO]
    assert len(info_records) >= 1
