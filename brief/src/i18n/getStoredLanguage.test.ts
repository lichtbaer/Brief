import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { getStoredLanguage } from "./index";

function createMockStorage(store: Record<string, string> = {}): Storage {
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => {
      store[key] = value;
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      for (const k of Object.keys(store)) delete store[k];
    },
    get length() {
      return Object.keys(store).length;
    },
    key: (_i: number) => null,
  };
}

describe("getStoredLanguage", () => {
  let savedWindow: typeof globalThis.window;
  let savedLocalStorage: typeof globalThis.localStorage;

  beforeEach(() => {
    savedWindow = globalThis.window;
    savedLocalStorage = globalThis.localStorage;
  });

  afterEach(() => {
    globalThis.window = savedWindow;
    globalThis.localStorage = savedLocalStorage;
  });

  it("returns 'de' when window is undefined", () => {
    // @ts-expect-error -- intentionally removing window for SSR test
    delete globalThis.window;
    expect(getStoredLanguage()).toBe("de");
  });

  it("returns 'de' when localStorage has no entry", () => {
    globalThis.window = {} as never;
    globalThis.localStorage = createMockStorage();
    expect(getStoredLanguage()).toBe("de");
  });

  it("returns 'en' when stored value is 'en'", () => {
    globalThis.window = {} as never;
    globalThis.localStorage = createMockStorage({ "brief.locale": "en" });
    expect(getStoredLanguage()).toBe("en");
  });

  it("returns 'de' when stored value is 'de'", () => {
    globalThis.window = {} as never;
    globalThis.localStorage = createMockStorage({ "brief.locale": "de" });
    expect(getStoredLanguage()).toBe("de");
  });

  it("returns 'de' for invalid language value", () => {
    globalThis.window = {} as never;
    globalThis.localStorage = createMockStorage({ "brief.locale": "fr" });
    expect(getStoredLanguage()).toBe("de");
  });

  it("returns 'de' for empty string value", () => {
    globalThis.window = {} as never;
    globalThis.localStorage = createMockStorage({ "brief.locale": "" });
    expect(getStoredLanguage()).toBe("de");
  });
});
