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
import logging
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
MODELS_DIR = os.path.join(SCRIPT_DIR, "models")
HF_CACHE = os.path.join(MODELS_DIR, "huggingface")

# Module-level logger; basicConfig is called in main() to avoid side-effects on import.
logger = logging.getLogger(__name__)


def _download(step: str, repo_id: str, local_dir: str | None = None) -> None:
    """Download a single model with error handling and retry guidance.

    Calls ``snapshot_download`` from huggingface_hub. Reads HF_TOKEN from the
    environment at call time (not at module import) so tests can patch it.

    Args:
        step: Human-readable progress indicator, e.g. ``"1/3"``.
        repo_id: HuggingFace repository ID to download.
        local_dir: If provided, save files to this local directory instead of
            the default HF cache location.
    """
    from huggingface_hub import snapshot_download

    hf_token = os.environ.get("HF_TOKEN")
    logger.info("%s  Downloading %s…", step, repo_id)
    try:
        kwargs: dict = {"token": hf_token}
        if local_dir:
            kwargs["local_dir"] = local_dir
        snapshot_download(repo_id, **kwargs)
        logger.info("%s  Finished downloading %s", step, repo_id)
    except Exception as e:
        logger.error("Error downloading %s: %s", repo_id, e)
        logger.error("Check your network connection and HF_TOKEN, then retry.")
        sys.exit(1)


def main() -> None:
    """Download Whisper, pyannote diarization, and segmentation models."""
    logging.basicConfig(
        level=logging.INFO,
        stream=sys.stderr,
        format="%(asctime)s %(levelname)-8s %(name)s: %(message)s",
        datefmt="%H:%M:%S",
    )

    os.makedirs(MODELS_DIR, exist_ok=True)

    # Redirect all HuggingFace / pyannote caches into our local models directory
    # so every artefact ends up in one portable tree.
    os.environ["HF_HOME"] = HF_CACHE
    os.environ["PYANNOTE_CACHE"] = HF_CACHE

    hf_token = os.environ.get("HF_TOKEN")
    if not hf_token:
        logger.error("HF_TOKEN environment variable required.")
        logger.error("Get your token at https://huggingface.co/settings/tokens")
        sys.exit(1)
        return  # Guard for tests that patch sys.exit

    logger.info("Starting model downloads to %s", MODELS_DIR)
    _download(
        "1/3",
        "guillaumekln/faster-whisper-base",
        local_dir=os.path.join(MODELS_DIR, "faster-whisper-base"),
    )
    _download("2/3", "pyannote/speaker-diarization-3.1")
    _download("3/3", "pyannote/segmentation-3.0")

    logger.info("All models downloaded to %s", MODELS_DIR)
    logger.info("You can now run the app fully offline.")


if __name__ == "__main__":
    main()
