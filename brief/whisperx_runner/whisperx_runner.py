#!/usr/bin/env python3
import sys
import json
import whisperx
from whisperx.diarize import DiarizationPipeline


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Kein WAV-Pfad angegeben"}))
        sys.exit(1)

    wav_path = sys.argv[1]
    language = sys.argv[2] if len(sys.argv) > 2 else "de"
    model_size = sys.argv[3] if len(sys.argv) > 3 else "base"

    def progress(msg: str) -> None:
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

        output = {
            "segments": [
                {
                    "speaker": seg.get("speaker", "SPEAKER_00"),
                    "start": seg["start"],
                    "end": seg["end"],
                    "text": seg["text"].strip(),
                }
                for seg in result["segments"]
            ],
            "language": language,
        }
        print(json.dumps(output, ensure_ascii=False))

    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)


if __name__ == "__main__":
    main()
