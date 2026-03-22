#!/usr/bin/env python3
import json
import sys

import whisperx
from whisperx.diarize import DiarizationPipeline


def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Kein WAV-Pfad angegeben"}))
        sys.exit(1)

    wav_path = sys.argv[1]
    language = sys.argv[2] if len(sys.argv) > 2 else "de"
    model_size = sys.argv[3] if len(sys.argv) > 3 else "base"

    try:
        # Transkription
        model = whisperx.load_model(model_size, device="cpu", language=language)
        audio = whisperx.load_audio(wav_path)
        result = model.transcribe(audio, language=language)

        # Word-level Alignment
        model_a, metadata = whisperx.load_align_model(language_code=language, device="cpu")
        result = whisperx.align(
            result["segments"], model_a, metadata, audio, device="cpu"
        )

        # Speaker Diarization
        diarize_model = DiarizationPipeline(device="cpu")
        diarize_segments = diarize_model(audio)
        result = whisperx.assign_word_speakers(diarize_segments, result)

        # Output
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
