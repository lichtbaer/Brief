#!/usr/bin/env python3
"""Local WhisperX CLI: transcribe, align, and diarize a mono WAV; print JSON to stdout.

Expected argv: ``wav_path`` [, ``language`` [, ``model_size``]]. Progress logs go to stderr so Rust can parse stdout only.
"""
import json
import os
import sys

from payload import build_success_payload

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
MODELS_DIR = os.path.join(SCRIPT_DIR, "models")
HF_CACHE = os.path.join(MODELS_DIR, "huggingface")

# Point HuggingFace + pyannote caches to bundled models so the app works
# fully offline without requiring a HuggingFace account from end users.
os.environ["HF_HOME"] = HF_CACHE
os.environ["PYANNOTE_CACHE"] = HF_CACHE
os.environ["HF_HUB_OFFLINE"] = "1"


def _patch_torch_load():
    """Allow loading pyannote checkpoints that use omegaconf globals.

    PyTorch 2.6 changed ``torch.load`` to default to ``weights_only=True``,
    which rejects ``omegaconf.ListConfig`` used in pyannote model files.
    All models are bundled locally and trusted, so ``weights_only=False`` is safe.
    """
    import torch

    _original = torch.load

    def _patched(*args, **kwargs):
        if "weights_only" not in kwargs:
            kwargs["weights_only"] = False
        return _original(*args, **kwargs)

    torch.load = _patched


def main() -> None:
    """Load WhisperX on CPU, run transcription + alignment + diarization, emit JSON segments or ``{"error": ...}``."""
    if len(sys.argv) < 2:
        print(json.dumps({"error": "No WAV path provided"}))
        sys.exit(1)

    wav_path: str = sys.argv[1]
    language: str = sys.argv[2] if len(sys.argv) > 2 else "de"
    model_size: str = sys.argv[3] if len(sys.argv) > 3 else "base"

    if not os.path.isfile(wav_path):
        print(json.dumps({"error": f"File not found: {wav_path}"}))
        sys.exit(1)

    def progress(msg: str) -> None:
        """Write a human-readable status line to stderr (does not pollute JSON stdout)."""
        print(msg, file=sys.stderr, flush=True)

    try:
        _patch_torch_load()

        # Heavy imports inside try/except so import errors are reported as JSON on stdout.
        import whisperx
        from whisperx.diarize import DiarizationPipeline

        whisper_model_dir = os.path.join(MODELS_DIR, "faster-whisper-base")

        progress("Loading WhisperX model…")
        model = whisperx.load_model(
            model_size,
            device="cpu",
            compute_type="float32",
            language=language,
            download_root=whisper_model_dir,
            local_files_only=True,
        )

        progress("Loading audio…")
        audio = whisperx.load_audio(wav_path)
        progress("Transcribing…")
        result = model.transcribe(audio, language=language)

        progress("Aligning words…")
        model_a, metadata = whisperx.load_align_model(language_code=language, device="cpu")
        result = whisperx.align(result["segments"], model_a, metadata, audio, device="cpu")

        progress("Speaker diarization…")
        diarize_model = DiarizationPipeline(device="cpu")
        diarize_segments = diarize_model(audio)
        result = whisperx.assign_word_speakers(diarize_segments, result)
        progress("Done.")

        output = build_success_payload(result["segments"], language)
        print(json.dumps(output, ensure_ascii=False))

    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)


if __name__ == "__main__":
    main()
