"""Unit tests for WhisperX runner JSON shape (no ML dependencies)."""

from payload import build_success_payload


def test_success_payload_has_segments_and_language():
    fake_segments = [
        {"speaker": "SPEAKER_00", "start": 0.0, "end": 1.0, "text": "hello"},
    ]
    out = build_success_payload(fake_segments, "de")
    assert "segments" in out
    assert "language" in out
    assert out["language"] == "de"
    assert len(out["segments"]) == 1
    seg = out["segments"][0]
    assert seg["speaker"] == "SPEAKER_00"
    assert seg["start"] == 0.0
    assert seg["end"] == 1.0
    assert seg["text"] == "hello"


def test_success_payload_defaults_missing_speaker_key():
    fake_segments = [{"start": 0.0, "end": 0.5, "text": "x"}]
    out = build_success_payload(fake_segments, "en")
    assert out["segments"][0]["speaker"] == "SPEAKER_00"
