"""Pure helpers for WhisperX runner stdout JSON (testable without loading ML)."""

from typing import Any


def build_success_payload(segments: list[dict[str, Any]], language: str) -> dict[str, Any]:
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
