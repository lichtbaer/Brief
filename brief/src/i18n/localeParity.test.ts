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

function loadLocale(lang: string): unknown {
  const p = join(__dirname, `locales/${lang}/common.json`);
  return JSON.parse(readFileSync(p, "utf8")) as unknown;
}

describe("i18n de/en key parity", () => {
  const de = loadLocale("de");
  const en = loadLocale("en");
  const dePaths = new Set(collectLeafPaths(de, ""));
  const enPaths = new Set(collectLeafPaths(en, ""));

  it("every leaf key path in de/common.json exists in en/common.json", () => {
    const missing = [...dePaths].filter((p) => !enPaths.has(p));
    expect(missing, `Missing in en: ${missing.join(", ")}`).toEqual([]);
  });

  it("every leaf key path in en/common.json exists in de/common.json", () => {
    const missing = [...enPaths].filter((p) => !dePaths.has(p));
    expect(missing, `Missing in de: ${missing.join(", ")}`).toEqual([]);
  });

  it("both locales have the same number of keys", () => {
    expect(dePaths.size).toBe(enPaths.size);
  });

  it("leaf values are all strings (no nested objects at leaf level)", () => {
    function checkLeafValues(obj: unknown, prefix: string): void {
      if (obj === null || typeof obj !== "object" || Array.isArray(obj)) {
        expect(typeof obj, `${prefix} should be a string`).toBe("string");
        return;
      }
      for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
        checkLeafValues(v, prefix ? `${prefix}.${k}` : k);
      }
    }
    checkLeafValues(de, "de");
    checkLeafValues(en, "en");
  });

  it("interpolation placeholders match between locales", () => {
    // Extract {{placeholders}} from each leaf value and ensure they match.
    const placeholderRe = /\{\{(\w+)\}\}/g;

    function extractPlaceholders(obj: unknown, prefix: string): Map<string, string[]> {
      const map = new Map<string, string[]>();
      if (typeof obj === "string") {
        const matches = [...obj.matchAll(placeholderRe)].map((m) => m[1]);
        if (matches.length > 0) {
          map.set(prefix, matches.sort());
        }
        return map;
      }
      if (obj !== null && typeof obj === "object" && !Array.isArray(obj)) {
        for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
          for (const [path, placeholders] of extractPlaceholders(v, prefix ? `${prefix}.${k}` : k)) {
            map.set(path, placeholders);
          }
        }
      }
      return map;
    }

    const dePlaceholders = extractPlaceholders(de, "");
    const enPlaceholders = extractPlaceholders(en, "");

    for (const [path, dePh] of dePlaceholders) {
      const enPh = enPlaceholders.get(path);
      expect(enPh, `en.${path} missing placeholders ${dePh.join(", ")}`).toBeDefined();
      expect(enPh?.sort(), `Placeholder mismatch at ${path}`).toEqual(dePh);
    }
  });

  it("no locale key is an empty string", () => {
    function checkNonEmpty(obj: unknown, prefix: string): void {
      if (typeof obj === "string") {
        expect(obj.length, `${prefix} should not be empty`).toBeGreaterThan(0);
        return;
      }
      if (obj !== null && typeof obj === "object" && !Array.isArray(obj)) {
        for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
          checkNonEmpty(v, prefix ? `${prefix}.${k}` : k);
        }
      }
    }
    checkNonEmpty(de, "de");
    checkNonEmpty(en, "en");
  });
});
