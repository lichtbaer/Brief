import { describe, expect, it } from "vitest";
import { safeExportBaseName } from "./OutputView";

describe("safeExportBaseName", () => {
  it("returns title unchanged when no special chars", () => {
    expect(safeExportBaseName("Team Meeting")).toBe("Team Meeting");
  });

  it("returns 'meeting' for empty string", () => {
    expect(safeExportBaseName("")).toBe("meeting");
  });

  it("returns 'meeting' for whitespace-only string", () => {
    expect(safeExportBaseName("   ")).toBe("meeting");
  });

  it("replaces unsafe filename characters with dashes", () => {
    expect(safeExportBaseName("Q1/Q2")).toBe("Q1-Q2");
    expect(safeExportBaseName("file\\name")).toBe("file-name");
    expect(safeExportBaseName("report:final")).toBe("report-final");
    expect(safeExportBaseName("test|pipe")).toBe("test-pipe");
    expect(safeExportBaseName('with"quotes')).toBe("with-quotes");
    expect(safeExportBaseName("a<b>c")).toBe("a-b-c");
    expect(safeExportBaseName("what?")).toBe("what-");
    expect(safeExportBaseName("50%")).toBe("50-");
    expect(safeExportBaseName("wild*card")).toBe("wild-card");
  });

  it("preserves unicode characters", () => {
    expect(safeExportBaseName("Beratungsgespräch Ärzte")).toBe(
      "Beratungsgespräch Ärzte",
    );
  });

  it("handles long titles without truncation", () => {
    const long = "a".repeat(200);
    expect(safeExportBaseName(long)).toBe(long);
  });
});
