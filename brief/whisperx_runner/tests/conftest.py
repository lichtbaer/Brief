"""Shared pytest fixtures for whisperx_runner tests."""

import struct
import subprocess
import sys
from pathlib import Path

import pytest


# ---------------------------------------------------------------------------
# Path helpers
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def runner_script() -> Path:
    """Absolute path to the whisperx_runner.py entry-point script."""
    return Path(__file__).resolve().parent.parent / "whisperx_runner.py"


@pytest.fixture(scope="session")
def download_script() -> Path:
    """Absolute path to the download_models.py script."""
    return Path(__file__).resolve().parent.parent / "download_models.py"


# ---------------------------------------------------------------------------
# Minimal WAV file factory
# ---------------------------------------------------------------------------

def _make_wav_bytes(num_samples: int = 4, sample_rate: int = 16000) -> bytes:
    """Return a minimal valid PCM-16 mono WAV file as bytes.

    The resulting file is small but fully spec-compliant so any library that
    validates RIFF headers will accept it.
    """
    num_channels = 1
    bits_per_sample = 16
    byte_rate = sample_rate * num_channels * bits_per_sample // 8
    block_align = num_channels * bits_per_sample // 8
    data_size = num_samples * block_align
    riff_size = 36 + data_size

    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        riff_size,
        b"WAVE",
        b"fmt ",
        16,             # PCM chunk size
        1,              # AudioFormat (PCM)
        num_channels,
        sample_rate,
        byte_rate,
        block_align,
        bits_per_sample,
        b"data",
        data_size,
    )
    # Fill with silence (zero samples)
    samples = b"\x00" * data_size
    return header + samples


@pytest.fixture
def tmp_wav_file(tmp_path: Path) -> Path:
    """Write a minimal valid WAV file to a temporary directory and return its path."""
    wav_path = tmp_path / "test_audio.wav"
    wav_path.write_bytes(_make_wav_bytes())
    return wav_path


# ---------------------------------------------------------------------------
# Subprocess helper (mirrors the one used in test_error_handling.py)
# ---------------------------------------------------------------------------

def run_script(script: Path, *args: str) -> subprocess.CompletedProcess:
    """Invoke *script* in a subprocess with the given arguments.

    Uses the same Python interpreter that is running pytest so the test
    environment's packages are available.
    """
    return subprocess.run(
        [sys.executable, str(script), *args],
        capture_output=True,
        text=True,
        check=False,
    )
