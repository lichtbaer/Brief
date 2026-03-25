"""Unit tests for WhisperX runner JSON shape (no ML dependencies)."""

import json

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


def test_success_payload_empty_segments():
    """Empty segment list should produce valid JSON with empty array."""
    out = build_success_payload([], "de")
    assert out["segments"] == []
    assert out["language"] == "de"


def test_success_payload_strips_whitespace_from_text():
    """Text with leading/trailing whitespace should be stripped."""
    fake_segments = [{"speaker": "A", "start": 0.0, "end": 1.0, "text": "  hello world  "}]
    out = build_success_payload(fake_segments, "de")
    assert out["segments"][0]["text"] == "hello world"


def test_success_payload_preserves_speaker_name():
    """Custom speaker names (not default SPEAKER_00) should pass through."""
    fake_segments = [{"speaker": "Alice", "start": 0.0, "end": 1.0, "text": "hi"}]
    out = build_success_payload(fake_segments, "en")
    assert out["segments"][0]["speaker"] == "Alice"


def test_success_payload_multiple_segments():
    """Multiple segments maintain order and individual data."""
    segs = [
        {"speaker": "A", "start": 0.0, "end": 1.0, "text": "First"},
        {"speaker": "B", "start": 1.0, "end": 2.5, "text": "Second"},
        {"speaker": "A", "start": 2.5, "end": 4.0, "text": "Third"},
    ]
    out = build_success_payload(segs, "de")
    assert len(out["segments"]) == 3
    assert out["segments"][0]["speaker"] == "A"
    assert out["segments"][1]["speaker"] == "B"
    assert out["segments"][2]["text"] == "Third"
    assert out["segments"][1]["end"] == 2.5


def test_success_payload_is_json_serializable():
    """The output dict must be JSON-serializable (no special Python types)."""
    segs = [{"speaker": "X", "start": 0.0, "end": 1.0, "text": "test"}]
    out = build_success_payload(segs, "de")
    # Should not raise.
    serialized = json.dumps(out, ensure_ascii=False)
    # Round-trip should produce identical data.
    assert json.loads(serialized) == out


def test_success_payload_unicode_text():
    """Unicode characters (e.g. German umlauts) should pass through intact."""
    segs = [{"speaker": "S", "start": 0.0, "end": 1.0, "text": "Über Straße"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["text"] == "Über Straße"


def test_success_payload_language_preserved():
    """Various language codes should be preserved as-is."""
    for lang in ["de", "en", "fr", "es"]:
        out = build_success_payload([], lang)
        assert out["language"] == lang
