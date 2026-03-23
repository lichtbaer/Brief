"""Pure helpers for WhisperX runner stdout JSON (testable without loading ML)."""


def build_success_payload(segments, language: str) -> dict:
    """Build the JSON-serializable object emitted on successful transcription (segments + language)."""
    return {
        "segments": [
            {
                "speaker": seg.get("speaker", "SPEAKER_00"),
                "start": seg["start"],
                "end": seg["end"],
                "text": seg["text"].strip(),
            }
            for seg in segments
        ],
        "language": language,
    }
