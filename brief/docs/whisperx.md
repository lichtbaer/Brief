# WhisperX & models

## Initial setup

```bash
cd brief/whisperx_runner
bash setup.sh
```

This creates a virtual environment and installs Python dependencies. The first transcription run may download a large Whisper checkpoint (on the order of ~150 MB for the default stack).

## Pyannote / Hugging Face (diarization)

Diarization models are gated on Hugging Face. One-time steps:

1. Accept the model licenses (e.g. [pyannote/speaker-diarization-3.1](https://huggingface.co/pyannote/speaker-diarization-3.1), [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0)).
2. Create a token: [huggingface.co/settings/tokens](https://huggingface.co/settings/tokens).

```bash
cd brief/whisperx_runner
source .venv/bin/activate
export HF_TOKEN="hf_..."
python download_models.py
```

Artifacts land under `whisperx_runner/models/` (~170 MB) and can ship inside the app bundle. End users do **not** need a Hugging Face account once models are bundled.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `BRIEF_WHISPERX_RUNNER` | Override path to the WhisperX runner script in development |

## Implementation notes

- The runner is invoked as a subprocess from Rust (`transcribe.rs` and related helpers).
- WhisperX expects **16 kHz mono** input; the Rust side resamples device audio accordingly (see module docs in `audio.rs`).

For Python-level behavior and edge cases, prefer the docstrings and tests under `brief/whisperx_runner/tests/`.
