#!/usr/bin/env python3
"""Local WhisperX CLI: transcribe, align, and diarize a mono WAV; print JSON to stdout.

Expected argv: ``wav_path`` [, ``language`` [, ``model_size``]]. Progress logs go to stderr so Rust can parse stdout only.
"""
import json
import os
import sys

from payload import build_success_payload


def main():
    """Load WhisperX on CPU, run transcription + alignment + diarization, emit JSON segments or ``{"error": ...}``."""
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Kein WAV-Pfad angegeben"}))
        sys.exit(1)

    wav_path = sys.argv[1]
    language = sys.argv[2] if len(sys.argv) > 2 else "de"
    model_size = sys.argv[3] if len(sys.argv) > 3 else "base"

    if not os.path.isfile(wav_path):
        print(json.dumps({"error": f"Datei nicht gefunden: {wav_path}"}))
        sys.exit(1)

    # Heavy imports only after argv validation so unit tests and missing-file paths fail fast.
    import whisperx
    from whisperx.diarize import DiarizationPipeline

    def progress(msg: str) -> None:
        """Write a human-readable status line to stderr (does not pollute JSON stdout)."""
        print(msg, file=sys.stderr, flush=True)

    try:
        progress("Loading WhisperX model…")
        model = whisperx.load_model(model_size, device="cpu", language=language)
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
