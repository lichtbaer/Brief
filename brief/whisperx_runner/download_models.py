#!/usr/bin/env python3
"""Download all WhisperX models for offline bundling.

Requires ``HF_TOKEN`` env var (create at https://huggingface.co/settings/tokens).
You must also accept the license agreements for the gated pyannote models:
  - https://huggingface.co/pyannote/speaker-diarization-3.1
  - https://huggingface.co/pyannote/segmentation-3.0

Usage::

    export HF_TOKEN="hf_..."
    python download_models.py
"""
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
MODELS_DIR = os.path.join(SCRIPT_DIR, "models")
HF_CACHE = os.path.join(MODELS_DIR, "huggingface")

os.makedirs(MODELS_DIR, exist_ok=True)

# Redirect all HuggingFace / pyannote caches into our local models directory
# so every artefact ends up in one portable tree.
os.environ["HF_HOME"] = HF_CACHE
os.environ["PYANNOTE_CACHE"] = HF_CACHE

HF_TOKEN = os.environ.get("HF_TOKEN")
if not HF_TOKEN:
    print("Error: HF_TOKEN environment variable required.", file=sys.stderr)
    print("Get your token at https://huggingface.co/settings/tokens", file=sys.stderr)
    sys.exit(1)


def main():
    """Download Whisper, pyannote diarization, and segmentation models."""
    from huggingface_hub import snapshot_download

    print("1/3  Downloading faster-whisper-base model (~150 MB)…")
    snapshot_download(
        "guillaumekln/faster-whisper-base",
        local_dir=os.path.join(MODELS_DIR, "faster-whisper-base"),
        token=HF_TOKEN,
    )

    print("2/3  Downloading pyannote/speaker-diarization-3.1 pipeline…")
    snapshot_download(
        "pyannote/speaker-diarization-3.1",
        token=HF_TOKEN,
    )

    print("3/3  Downloading pyannote/segmentation-3.0 model…")
    snapshot_download(
        "pyannote/segmentation-3.0",
        token=HF_TOKEN,
    )

    print(f"\nAll models downloaded to {MODELS_DIR}")
    print("You can now run the app fully offline.")


if __name__ == "__main__":
    main()
