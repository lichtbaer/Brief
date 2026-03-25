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


def _download(step: str, repo_id: str, local_dir: str | None = None) -> None:
    """Download a single model with error handling and retry guidance."""
    from huggingface_hub import snapshot_download

    print(f"{step}  Downloading {repo_id}…")
    try:
        kwargs: dict = {"token": HF_TOKEN}
        if local_dir:
            kwargs["local_dir"] = local_dir
        snapshot_download(repo_id, **kwargs)
    except Exception as e:
        print(f"Error downloading {repo_id}: {e}", file=sys.stderr)
        print("Check your network connection and HF_TOKEN, then retry.", file=sys.stderr)
        sys.exit(1)


def main() -> None:
    """Download Whisper, pyannote diarization, and segmentation models."""
    _download(
        "1/3",
        "guillaumekln/faster-whisper-base",
        local_dir=os.path.join(MODELS_DIR, "faster-whisper-base"),
    )
    _download("2/3", "pyannote/speaker-diarization-3.1")
    _download("3/3", "pyannote/segmentation-3.0")

    print(f"\nAll models downloaded to {MODELS_DIR}")
    print("You can now run the app fully offline.")


if __name__ == "__main__":
    main()
