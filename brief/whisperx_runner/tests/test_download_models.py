"""Unit tests for download_models.py using mocks (no network / HF_TOKEN required)."""

import importlib
import logging
import os
import sys
import types
from pathlib import Path
from unittest.mock import MagicMock, call, patch


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _import_fresh(monkeypatch, hf_token: str | None = "dummy-token"):
    """Import download_models with a controlled environment.

    Because download_models previously checked HF_TOKEN at module level we
    always reload it to get a clean state.  The new version moved the check
    into main(), so a bare import is now safe.

    Args:
        monkeypatch: pytest monkeypatch fixture for env isolation.
        hf_token: Value to set for HF_TOKEN; ``None`` removes the variable.
    """
    # Isolate env changes to this test
    if hf_token is None:
        monkeypatch.delenv("HF_TOKEN", raising=False)
    else:
        monkeypatch.setenv("HF_TOKEN", hf_token)

    # Force a fresh import so module-level side effects are re-evaluated
    if "download_models" in sys.modules:
        del sys.modules["download_models"]

    # Ensure the package directory is on the path
    pkg_dir = str(Path(__file__).resolve().parent.parent)
    if pkg_dir not in sys.path:
        sys.path.insert(0, pkg_dir)

    return importlib.import_module("download_models")


# ---------------------------------------------------------------------------
# Tests for _download()
# ---------------------------------------------------------------------------

def test_download_success_calls_snapshot_download(monkeypatch):
    """_download() calls snapshot_download with the correct repo_id and token."""
    dm = _import_fresh(monkeypatch, hf_token="test-token-abc")

    mock_snapshot = MagicMock()
    # Patch the huggingface_hub module inside download_models
    fake_hf_hub = types.ModuleType("huggingface_hub")
    fake_hf_hub.snapshot_download = mock_snapshot

    with patch.dict(sys.modules, {"huggingface_hub": fake_hf_hub}):
        dm._download("1/1", "some/repo")

    mock_snapshot.assert_called_once()
    call_kwargs = mock_snapshot.call_args
    assert call_kwargs.args[0] == "some/repo"
    assert call_kwargs.kwargs["token"] == "test-token-abc"


def test_download_with_local_dir_passes_local_dir(monkeypatch, tmp_path):
    """_download() forwards local_dir to snapshot_download when provided."""
    dm = _import_fresh(monkeypatch, hf_token="tok")

    mock_snapshot = MagicMock()
    fake_hf_hub = types.ModuleType("huggingface_hub")
    fake_hf_hub.snapshot_download = mock_snapshot

    target_dir = str(tmp_path / "mymodel")

    with patch.dict(sys.modules, {"huggingface_hub": fake_hf_hub}):
        dm._download("1/1", "org/model", local_dir=target_dir)

    call_kwargs = mock_snapshot.call_args.kwargs
    assert call_kwargs["local_dir"] == target_dir


def test_download_without_local_dir_omits_local_dir(monkeypatch):
    """_download() does NOT pass local_dir when the argument is None."""
    dm = _import_fresh(monkeypatch, hf_token="tok")

    mock_snapshot = MagicMock()
    fake_hf_hub = types.ModuleType("huggingface_hub")
    fake_hf_hub.snapshot_download = mock_snapshot

    with patch.dict(sys.modules, {"huggingface_hub": fake_hf_hub}):
        dm._download("1/1", "org/model", local_dir=None)

    call_kwargs = mock_snapshot.call_args.kwargs
    assert "local_dir" not in call_kwargs


def test_download_exception_exits_with_code_1(monkeypatch):
    """_download() calls sys.exit(1) when snapshot_download raises an exception."""
    dm = _import_fresh(monkeypatch, hf_token="tok")

    mock_snapshot = MagicMock(side_effect=RuntimeError("network error"))
    fake_hf_hub = types.ModuleType("huggingface_hub")
    fake_hf_hub.snapshot_download = mock_snapshot

    with patch.dict(sys.modules, {"huggingface_hub": fake_hf_hub}):
        with patch.object(sys, "exit") as mock_exit:
            dm._download("1/1", "broken/repo")
            mock_exit.assert_called_once_with(1)


def test_download_exception_logs_error(monkeypatch, caplog):
    """_download() emits an ERROR log when snapshot_download raises."""
    dm = _import_fresh(monkeypatch, hf_token="tok")

    mock_snapshot = MagicMock(side_effect=ValueError("bad creds"))
    fake_hf_hub = types.ModuleType("huggingface_hub")
    fake_hf_hub.snapshot_download = mock_snapshot

    with patch.dict(sys.modules, {"huggingface_hub": fake_hf_hub}):
        with patch.object(sys, "exit"):
            with caplog.at_level(logging.ERROR, logger="download_models"):
                dm._download("1/1", "org/repo")

    assert any("bad creds" in r.message or "Error" in r.message for r in caplog.records)


# ---------------------------------------------------------------------------
# Tests for main()
# ---------------------------------------------------------------------------

def test_main_calls_all_three_downloads(monkeypatch):
    """main() invokes _download exactly three times with the expected repo IDs."""
    dm = _import_fresh(monkeypatch, hf_token="tok")

    calls_made: list[tuple[str, str]] = []

    def fake_download(step: str, repo_id: str, local_dir=None):
        calls_made.append((step, repo_id))

    # Patch _download on the freshly imported module
    monkeypatch.setattr(dm, "_download", fake_download)

    dm.main()

    assert len(calls_made) == 3
    repo_ids = [r for _, r in calls_made]
    assert "guillaumekln/faster-whisper-base" in repo_ids
    assert "pyannote/speaker-diarization-3.1" in repo_ids
    assert "pyannote/segmentation-3.0" in repo_ids


def test_main_missing_hf_token_exits_with_code_1(monkeypatch):
    """main() calls sys.exit(1) when HF_TOKEN is not set."""
    dm = _import_fresh(monkeypatch, hf_token=None)

    with patch.object(sys, "exit") as mock_exit:
        dm.main()
        mock_exit.assert_called_once_with(1)


def test_main_missing_hf_token_logs_error(monkeypatch, caplog):
    """main() emits an ERROR log when HF_TOKEN is missing."""
    dm = _import_fresh(monkeypatch, hf_token=None)

    with patch.object(sys, "exit"):
        with caplog.at_level(logging.ERROR, logger="download_models"):
            dm.main()

    assert any("HF_TOKEN" in r.message for r in caplog.records)
