import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Mock i18next before importing the module-under-test so that the module-level
// i18next.init() call at import time does not fail.
vi.mock("i18next", () => {
  const mockT = vi.fn((key: string) => key);
  return {
    default: {
      init: vi.fn(),
      changeLanguage: vi.fn().mockResolvedValue(undefined),
      t: mockT,
    },
  };
});

import { normalizeLanguage, resolveInitialLanguage, setLanguage } from "../../i18n";

// ── normalizeLanguage ────────────────────────────────────────────────

describe("normalizeLanguage", () => {
  it('normalizes "zh" to "zh-CN"', () => {
    expect(normalizeLanguage("zh")).toBe("zh-CN");
  });

  it('normalizes "zh-CN" to "zh-CN"', () => {
    expect(normalizeLanguage("zh-CN")).toBe("zh-CN");
  });

  it('normalizes "zh-TW" to "zh-CN"', () => {
    expect(normalizeLanguage("zh-TW")).toBe("zh-CN");
  });

  it('normalizes "zh-HK" to "zh-CN"', () => {
    expect(normalizeLanguage("zh-HK")).toBe("zh-CN");
  });

  it('normalizes "en" to "en-US"', () => {
    expect(normalizeLanguage("en")).toBe("en-US");
  });

  it('normalizes "en-US" to "en-US"', () => {
    expect(normalizeLanguage("en-US")).toBe("en-US");
  });

  it('normalizes "en-GB" to "en-US"', () => {
    expect(normalizeLanguage("en-GB")).toBe("en-US");
  });

  it('falls back to "zh-CN" for unknown language "fr"', () => {
    expect(normalizeLanguage("fr")).toBe("zh-CN");
  });

  it('falls back to "zh-CN" for undefined', () => {
    expect(normalizeLanguage(undefined)).toBe("zh-CN");
  });

  it('falls back to "zh-CN" for null', () => {
    expect(normalizeLanguage(null)).toBe("zh-CN");
  });
});

// ── resolveInitialLanguage ───────────────────────────────────────────

describe("resolveInitialLanguage", () => {
  const STORAGE_KEY = "limedl.language";
  const originalNavigatorLanguage = navigator.language;

  beforeEach(() => {
    localStorage.clear();
    Object.defineProperty(navigator, "language", {
      value: "en-US",
      configurable: true,
      writable: true,
    });
  });

  afterEach(() => {
    Object.defineProperty(navigator, "language", {
      value: originalNavigatorLanguage,
      configurable: true,
      writable: true,
    });
  });

  it("uses localStorage value over navigator.language", () => {
    localStorage.setItem(STORAGE_KEY, "zh-CN");
    expect(resolveInitialLanguage()).toBe("zh-CN");
  });

  it("falls back to navigator.language when localStorage is empty", () => {
    expect(resolveInitialLanguage()).toBe("en-US");
  });

  it("normalizes invalid localStorage value through normalizeLanguage", () => {
    localStorage.setItem(STORAGE_KEY, "fr");
    expect(resolveInitialLanguage()).toBe("zh-CN");
  });
});

// ── setLanguage ──────────────────────────────────────────────────────

describe("setLanguage", () => {
  const STORAGE_KEY = "limedl.language";

  beforeEach(() => {
    localStorage.clear();
    document.documentElement.lang = "";
  });

  it("writes a different language to localStorage and updates document.documentElement.lang", async () => {
    // The module initializes with "en-US" (jsdom default), so switching to
    // "zh-CN" should bypass the early return, write to localStorage, and
    // update <html lang="...">.
    await setLanguage("zh-CN");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
  });
});

// ── Translation Symmetry & Key Parity ─────────────────────────────────

import zhCN from "../../i18n/zh-CN";
import enUS from "../../i18n/en-US";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function extractLeafStrings(obj: Record<string, unknown>, prefix = ""): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (isRecord(value)) {
      Object.assign(result, extractLeafStrings(value, fullKey));
    } else if (typeof value === "string") {
      result[fullKey] = value;
    }
  }
  return result;
}

function extractPlaceholders(text: string): string[] {
  const matches = text.match(/\{\{([^}]+)\}\}/g) ?? [];
  return matches.map((m) => m.replace(/[{}]/g, "").trim()).toSorted();
}

describe("Translation Resource Symmetry", () => {
  const zhMap = extractLeafStrings(zhCN.translation);
  const enMap = extractLeafStrings(enUS.translation);
  const zhKeys = Object.keys(zhMap).toSorted();
  const enKeys = Object.keys(enMap).toSorted();

  it("ensures all zh-CN keys exist in en-US", () => {
    const missingInEn = zhKeys.filter((k) => !Object.hasOwn(enMap, k));
    expect(missingInEn).toEqual([]);
  });

  it("ensures all en-US keys exist in zh-CN", () => {
    const missingInZh = enKeys.filter((k) => !Object.hasOwn(zhMap, k));
    expect(missingInZh).toEqual([]);
  });

  it("ensures matching interpolation parameters between zh-CN and en-US", () => {
    const mismatchedPlaceholders: Record<string, { zh: string[]; en: string[] }> = {};

    for (const key of zhKeys) {
      if (!Object.hasOwn(enMap, key)) continue;
      const zhVal = zhMap[key];
      const enVal = enMap[key];
      if (typeof zhVal !== "string" || typeof enVal !== "string") continue;
      const zhParams = extractPlaceholders(zhVal);
      const enParams = extractPlaceholders(enVal);
      if (JSON.stringify(zhParams) !== JSON.stringify(enParams)) {
        mismatchedPlaceholders[key] = { zh: zhParams, en: enParams };
      }
    }

    expect(mismatchedPlaceholders).toEqual({});
  });
});
