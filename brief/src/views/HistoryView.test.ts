import { describe, expect, it } from "vitest";
import { formatMeetingDate } from "./HistoryView";

describe("formatMeetingDate", () => {
  it("formats ISO date with de-DE locale", () => {
    const result = formatMeetingDate("2024-03-15T14:30:00Z", "de-DE");
    // German format: DD.MM.YYYY, HH:MM
    expect(result).toContain("15");
    expect(result).toContain("03");
    expect(result).toContain("2024");
  });

  it("formats ISO date with en-GB locale", () => {
    const result = formatMeetingDate("2024-03-15T14:30:00Z", "en-GB");
    expect(result).toContain("15");
    expect(result).toContain("03");
    expect(result).toContain("2024");
  });

  it("handles midnight correctly", () => {
    const result = formatMeetingDate("2024-01-01T00:00:00Z", "de-DE");
    expect(result).toContain("01");
    expect(result).toContain("2024");
  });

  it("handles end of year date", () => {
    const result = formatMeetingDate("2024-12-31T23:59:00Z", "de-DE");
    expect(result).toContain("31");
    expect(result).toContain("12");
    expect(result).toContain("2024");
  });

  it("handles ISO date with timezone offset", () => {
    const result = formatMeetingDate("2024-06-15T10:00:00+02:00", "en-GB");
    expect(result).toContain("2024");
    expect(result).toContain("15");
  });
});
