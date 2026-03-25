"""Edge case tests for WhisperX payload builder (no ML dependencies)."""

import json

from payload import build_success_payload


def test_negative_timestamps():
    """Negative start/end values should pass through unchanged."""
    segs = [{"speaker": "A", "start": -1.0, "end": 0.5, "text": "early"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["start"] == -1.0


def test_start_greater_than_end():
    """Invalid time range should pass through (validation is caller's job)."""
    segs = [{"speaker": "A", "start": 5.0, "end": 2.0, "text": "reversed"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["start"] == 5.0
    assert out["segments"][0]["end"] == 2.0


def test_zero_duration_segment():
    """Segment with start == end should be valid."""
    segs = [{"speaker": "A", "start": 1.0, "end": 1.0, "text": "instant"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["start"] == out["segments"][0]["end"]


def test_whitespace_only_text_becomes_empty():
    """Text that is only whitespace should become empty after strip."""
    segs = [{"speaker": "A", "start": 0.0, "end": 1.0, "text": "   \t  "}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["text"] == ""


def test_very_long_text():
    """Long text should pass through without truncation."""
    long_text = "word " * 5000
    segs = [{"speaker": "A", "start": 0.0, "end": 300.0, "text": long_text}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["text"] == long_text.strip()


def test_special_chars_in_speaker():
    """Speaker names with special characters should pass through."""
    segs = [{"speaker": "Dr. Müller-Schmidt (Extern)", "start": 0.0, "end": 1.0, "text": "hi"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["speaker"] == "Dr. Müller-Schmidt (Extern)"


def test_float_precision():
    """Very precise float values should be preserved."""
    segs = [{"speaker": "A", "start": 0.123456789, "end": 0.987654321, "text": "precise"}]
    out = build_success_payload(segs, "de")
    assert out["segments"][0]["start"] == 0.123456789


def test_many_segments():
    """Large number of segments should work correctly."""
    segs = [
        {"speaker": f"S{i % 3}", "start": float(i), "end": float(i + 1), "text": f"Segment {i}"}
        for i in range(500)
    ]
    out = build_success_payload(segs, "de")
    assert len(out["segments"]) == 500
    assert out["segments"][0]["text"] == "Segment 0"
    assert out["segments"][499]["text"] == "Segment 499"


def test_json_roundtrip_with_edge_values():
    """Ensure all edge case values survive JSON serialization."""
    segs = [
        {"speaker": "A", "start": 0.0, "end": 0.001, "text": 'Quotes "inside" text'},
        {"start": 0.001, "end": 0.002, "text": "Newline\nin text"},
    ]
    out = build_success_payload(segs, "en")
    serialized = json.dumps(out, ensure_ascii=False)
    restored = json.loads(serialized)
    assert restored["segments"][0]["text"] == 'Quotes "inside" text'
    assert restored["segments"][1]["text"] == "Newline\nin text"
    assert restored["segments"][1]["speaker"] == "SPEAKER_00"


def test_empty_language_code():
    """Empty language code should be preserved (caller handles validation)."""
    out = build_success_payload([], "")
    assert out["language"] == ""
