import { describe, expect, it } from "vitest";
import {
  isDiarizedSegment,
  isMeeting,
  TRANSCRIPTION_TIMEOUT_ERROR,
  type Meeting,
  type MeetingOutput,
} from "./index";

function minimalMeetingOutput(): MeetingOutput {
  return {
    summary_short: "s",
    topics: [],
    decisions: [],
    action_items: [],
    follow_up_draft: {},
    participants_mentioned: [],
    template_used: "t",
    model_used: "m",
    generated_at: "2020-01-01T00:00:00Z",
  };
}

function validMeeting(overrides: Partial<Meeting> = {}): Record<string, unknown> {
  return {
    id: "x",
    created_at: "a",
    ended_at: "b",
    duration_seconds: 1,
    meeting_type: "consulting",
    title: "t",
    transcript: "tr",
    output: minimalMeetingOutput(),
    audio_path: null,
    tags: [],
    ...overrides,
  };
}

describe("isMeeting", () => {
  it("accepts a valid Meeting-shaped object", () => {
    expect(isMeeting(validMeeting())).toBe(true);
  });

  it("rejects wrong meeting_type", () => {
    expect(isMeeting(validMeeting({ meeting_type: "not-a-type" as never }))).toBe(false);
  });

  it("accepts all valid meeting types", () => {
    for (const mt of ["consulting", "legal", "internal", "custom"] as const) {
      expect(isMeeting(validMeeting({ meeting_type: mt }))).toBe(true);
    }
  });

  it("accepts string audio_path", () => {
    expect(isMeeting(validMeeting({ audio_path: "/tmp/audio.wav" }))).toBe(true);
  });

  it("rejects numeric audio_path", () => {
    expect(isMeeting({ ...validMeeting(), audio_path: 123 })).toBe(false);
  });

  it("rejects null", () => {
    expect(isMeeting(null)).toBe(false);
  });

  it("rejects undefined", () => {
    expect(isMeeting(undefined)).toBe(false);
  });

  it("rejects a string", () => {
    expect(isMeeting("not-an-object")).toBe(false);
  });

  it("rejects empty object", () => {
    expect(isMeeting({})).toBe(false);
  });

  it("rejects missing id", () => {
    const m = validMeeting();
    delete m.id;
    expect(isMeeting(m)).toBe(false);
  });

  it("rejects non-number duration_seconds", () => {
    expect(isMeeting({ ...validMeeting(), duration_seconds: "60" })).toBe(false);
  });

  it("rejects non-string tags", () => {
    expect(isMeeting({ ...validMeeting(), tags: [1, 2] })).toBe(false);
  });

  it("rejects non-array tags", () => {
    expect(isMeeting({ ...validMeeting(), tags: "tag1" })).toBe(false);
  });

  it("rejects null output", () => {
    expect(isMeeting({ ...validMeeting(), output: null })).toBe(false);
  });

  it("rejects output missing required fields", () => {
    expect(isMeeting({ ...validMeeting(), output: { summary_short: "s" } })).toBe(false);
  });

  it("rejects output with non-string summary_short", () => {
    const output = { ...minimalMeetingOutput(), summary_short: 42 };
    expect(isMeeting({ ...validMeeting(), output })).toBe(false);
  });

  it("rejects output with non-array topics", () => {
    const output = { ...minimalMeetingOutput(), topics: "not-array" };
    expect(isMeeting({ ...validMeeting(), output })).toBe(false);
  });

  it("rejects output with null follow_up_draft", () => {
    const output = { ...minimalMeetingOutput(), follow_up_draft: null };
    expect(isMeeting({ ...validMeeting(), output })).toBe(false);
  });
});

describe("isDiarizedSegment", () => {
  it("accepts a valid segment", () => {
    expect(
      isDiarizedSegment({
        speaker: "SPEAKER_00",
        start: 0,
        end: 1,
        text: "hi",
      }),
    ).toBe(true);
  });

  it("rejects missing fields", () => {
    expect(isDiarizedSegment({ speaker: "A", start: 0, end: 1 })).toBe(false);
  });

  it("rejects null", () => {
    expect(isDiarizedSegment(null)).toBe(false);
  });

  it("rejects non-object", () => {
    expect(isDiarizedSegment("string")).toBe(false);
    expect(isDiarizedSegment(42)).toBe(false);
  });

  it("rejects wrong types for numeric fields", () => {
    expect(isDiarizedSegment({ speaker: "A", start: "0", end: 1, text: "hi" })).toBe(false);
    expect(isDiarizedSegment({ speaker: "A", start: 0, end: "1", text: "hi" })).toBe(false);
  });

  it("rejects non-string speaker", () => {
    expect(isDiarizedSegment({ speaker: 0, start: 0, end: 1, text: "hi" })).toBe(false);
  });

  it("accepts segment with extra fields (forward compatibility)", () => {
    expect(
      isDiarizedSegment({
        speaker: "SPEAKER_01",
        start: 0,
        end: 2,
        text: "hello",
        confidence: 0.95,
      }),
    ).toBe(true);
  });
});

describe("TRANSCRIPTION_TIMEOUT_ERROR", () => {
  it("is a non-empty string constant", () => {
    expect(typeof TRANSCRIPTION_TIMEOUT_ERROR).toBe("string");
    expect(TRANSCRIPTION_TIMEOUT_ERROR.length).toBeGreaterThan(0);
  });

  it("matches the backend constant value", () => {
    // This token must stay in sync with `transcribe::TRANSCRIPTION_TIMEOUT_ERROR` in Rust.
    expect(TRANSCRIPTION_TIMEOUT_ERROR).toBe("BRIEF_ERR_TRANSCRIPTION_TIMEOUT");
  });
});
