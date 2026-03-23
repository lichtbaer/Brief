import { describe, expect, it } from "vitest";
import {
  isDiarizedSegment,
  isMeeting,
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

describe("isMeeting", () => {
  it("accepts a valid Meeting-shaped object", () => {
    const m: Meeting = {
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
    };
    expect(isMeeting(m)).toBe(true);
  });

  it("rejects wrong meeting_type", () => {
    const bad = {
      id: "x",
      created_at: "a",
      ended_at: "b",
      duration_seconds: 1,
      meeting_type: "not-a-type",
      title: "t",
      transcript: "tr",
      output: minimalMeetingOutput(),
      audio_path: null,
      tags: [],
    };
    expect(isMeeting(bad)).toBe(false);
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
});
