import { describe, expect, it } from "vitest";
import type { Meeting } from "../types";
import { formatTime, initialState, recordingReducer } from "./RecordingView";

// -- formatTime --

describe("formatTime", () => {
  it("formats zero seconds", () => {
    expect(formatTime(0)).toBe("00:00");
  });

  it("formats seconds below one minute", () => {
    expect(formatTime(5)).toBe("00:05");
    expect(formatTime(59)).toBe("00:59");
  });

  it("formats exact minute boundary", () => {
    expect(formatTime(60)).toBe("01:00");
  });

  it("formats minutes and seconds", () => {
    expect(formatTime(61)).toBe("01:01");
    expect(formatTime(125)).toBe("02:05");
  });

  it("formats over one hour", () => {
    expect(formatTime(3600)).toBe("60:00");
    expect(formatTime(3661)).toBe("61:01");
  });
});

// -- recordingReducer --

const fakeMeeting: Meeting = {
  id: "test-id",
  created_at: "2024-01-01T00:00:00Z",
  ended_at: "2024-01-01T01:00:00Z",
  duration_seconds: 3600,
  meeting_type: "consulting",
  title: "Test Meeting",
  transcript: "Hello world",
  output: {
    summary_short: "Summary",
    topics: [],
    decisions: [],
    action_items: [],
    follow_up_draft: {},
    participants_mentioned: [],
    template_used: "consulting",
    model_used: "test",
    generated_at: "2024-01-01T00:00:00Z",
  },
  audio_path: null,
  tags: [],
};

describe("recordingReducer", () => {
  it("START_RECORDING resets state and sets sessionId + recording status", () => {
    const result = recordingReducer(initialState, {
      type: "START_RECORDING",
      sessionId: "abc-123",
    });
    expect(result.status).toBe("recording");
    expect(result.sessionId).toBe("abc-123");
    expect(result.error).toBeNull();
    expect(result.meeting).toBeNull();
    expect(result.processingStep).toBeNull();
  });

  it("START_RECORDING clears previous error state", () => {
    const errorState = {
      ...initialState,
      status: "error" as const,
      error: "previous error",
    };
    const result = recordingReducer(errorState, {
      type: "START_RECORDING",
      sessionId: "new-session",
    });
    expect(result.error).toBeNull();
    expect(result.status).toBe("recording");
  });

  it("START_PROCESSING transitions to processing with transcribing step", () => {
    const recordingState = {
      ...initialState,
      status: "recording" as const,
      sessionId: "abc",
    };
    const result = recordingReducer(recordingState, { type: "START_PROCESSING" });
    expect(result.status).toBe("processing");
    expect(result.sessionId).toBeNull();
    expect(result.error).toBeNull();
    expect(result.processingStep).toBe("transcribing");
  });

  it("PROCESSING_DONE stores meeting and clears processing step", () => {
    const processingState = {
      ...initialState,
      status: "processing" as const,
      processingStep: "summarizing" as const,
    };
    const result = recordingReducer(processingState, {
      type: "PROCESSING_DONE",
      meeting: fakeMeeting,
    });
    expect(result.status).toBe("done");
    expect(result.meeting).toBe(fakeMeeting);
    expect(result.processingStep).toBeNull();
  });

  it("ERROR sets error message and clears sessionId + processingStep", () => {
    const processingState = {
      ...initialState,
      status: "processing" as const,
      sessionId: "active",
      processingStep: "transcribing" as const,
    };
    const result = recordingReducer(processingState, {
      type: "ERROR",
      error: "Something went wrong",
    });
    expect(result.status).toBe("error");
    expect(result.error).toBe("Something went wrong");
    expect(result.sessionId).toBeNull();
    expect(result.processingStep).toBeNull();
  });

  it("SET_PROCESSING_STEP updates step without changing other state", () => {
    const state = {
      ...initialState,
      status: "processing" as const,
      processingStep: "transcribing" as const,
    };
    const result = recordingReducer(state, {
      type: "SET_PROCESSING_STEP",
      step: "summarizing",
    });
    expect(result.processingStep).toBe("summarizing");
    expect(result.status).toBe("processing");
  });

  it("CLEAR_SESSION only nulls sessionId", () => {
    const state = {
      ...initialState,
      status: "recording" as const,
      sessionId: "abc",
      error: null,
    };
    const result = recordingReducer(state, { type: "CLEAR_SESSION" });
    expect(result.sessionId).toBeNull();
    expect(result.status).toBe("recording");
  });

  it("RESET returns to initialState", () => {
    const doneState = {
      ...initialState,
      status: "done" as const,
      meeting: fakeMeeting,
    };
    const result = recordingReducer(doneState, { type: "RESET" });
    expect(result).toEqual(initialState);
  });

  it("full lifecycle: idle → recording → processing → done → reset", () => {
    let state = initialState;
    state = recordingReducer(state, { type: "START_RECORDING", sessionId: "s1" });
    expect(state.status).toBe("recording");

    state = recordingReducer(state, { type: "START_PROCESSING" });
    expect(state.status).toBe("processing");

    state = recordingReducer(state, { type: "SET_PROCESSING_STEP", step: "summarizing" });
    expect(state.processingStep).toBe("summarizing");

    state = recordingReducer(state, { type: "PROCESSING_DONE", meeting: fakeMeeting });
    expect(state.status).toBe("done");

    state = recordingReducer(state, { type: "RESET" });
    expect(state).toEqual(initialState);
  });

  it("full lifecycle: idle → recording → error → reset", () => {
    let state = initialState;
    state = recordingReducer(state, { type: "START_RECORDING", sessionId: "s2" });
    state = recordingReducer(state, { type: "ERROR", error: "mic failed" });
    expect(state.status).toBe("error");
    expect(state.error).toBe("mic failed");

    state = recordingReducer(state, { type: "RESET" });
    expect(state).toEqual(initialState);
  });
});
