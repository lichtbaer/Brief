import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const __dirname = dirname(fileURLToPath(import.meta.url));

function collectLeafPaths(
  obj: unknown,
  prefix: string,
): string[] {
  if (obj === null || typeof obj !== "object" || Array.isArray(obj)) {
    return prefix ? [prefix] : [];
  }
  const keys = Object.keys(obj as Record<string, unknown>);
  if (keys.length === 0) {
    return prefix ? [prefix] : [];
  }
  const out: string[] = [];
  for (const k of keys) {
    const next = prefix ? `${prefix}.${k}` : k;
    out.push(...collectLeafPaths((obj as Record<string, unknown>)[k], next));
  }
  return out;
}

describe("i18n de/en key parity", () => {
  it("every leaf key path in de/common.json exists in en/common.json", () => {
    const dePath = join(__dirname, "locales/de/common.json");
    const enPath = join(__dirname, "locales/en/common.json");
    const de = JSON.parse(readFileSync(dePath, "utf8")) as unknown;
    const en = JSON.parse(readFileSync(enPath, "utf8")) as unknown;
    const dePaths = new Set(collectLeafPaths(de, ""));
    const enPaths = new Set(collectLeafPaths(en, ""));
    const missing = [...dePaths].filter((p) => !enPaths.has(p));
    expect(missing, `Missing in en: ${missing.join(", ")}`).toEqual([]);
  });
});
