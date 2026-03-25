import { describe, expect, it } from "vitest";
import { DEFAULTS, mergeSettings } from "./SettingsView";

describe("mergeSettings", () => {
  it("returns all defaults for empty input", () => {
    const result = mergeSettings({});
    expect(result).toEqual(DEFAULTS);
  });

  it("overrides individual keys while keeping defaults for the rest", () => {
    const result = mergeSettings({ ollama_url: "http://custom:9999" });
    expect(result.ollama_url).toBe("http://custom:9999");
    expect(result.llm_model).toBe(DEFAULTS.llm_model);
    expect(result.meeting_language).toBe(DEFAULTS.meeting_language);
  });

  it("overrides multiple keys", () => {
    const result = mergeSettings({
      llm_model: "mistral:7b",
      retain_audio: "true",
    });
    expect(result.llm_model).toBe("mistral:7b");
    expect(result.retain_audio).toBe("true");
    expect(result.ollama_url).toBe(DEFAULTS.ollama_url);
  });

  it("returns values unchanged when all defaults provided", () => {
    const result = mergeSettings({ ...DEFAULTS });
    expect(result).toEqual(DEFAULTS);
  });

  it("ignores unknown keys", () => {
    const result = mergeSettings({ foo: "bar" } as Record<string, string>);
    expect(result).toEqual(DEFAULTS);
    expect((result as unknown as Record<string, unknown>).foo).toBeUndefined();
  });
});
