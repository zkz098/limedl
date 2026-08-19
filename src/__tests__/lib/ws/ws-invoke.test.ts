import { describe, it, expect } from "vitest";
import { isNonNullObject, applyTransform, mapEventType } from "../../../lib/ws/ws-invoke";
import { WS_COMMANDS } from "../../../lib/ws/generated/ws-commands";
import { WS_EVENTS } from "../../../lib/ws/generated/ws-events";

// ---------------------------------------------------------------------------
// isNonNullObject
// ---------------------------------------------------------------------------
describe("isNonNullObject", () => {
  it("returns false for null", () => {
    expect(isNonNullObject(null)).toBe(false);
  });

  it("returns true for a plain object", () => {
    expect(isNonNullObject({})).toBe(true);
    expect(isNonNullObject({ key: "value" })).toBe(true);
  });

  it("returns true for an array (typeof array is 'object')", () => {
    expect(isNonNullObject([])).toBe(true);
    expect(isNonNullObject([1, 2, 3])).toBe(true);
  });

  it("returns false for a string", () => {
    expect(isNonNullObject("hello")).toBe(false);
    expect(isNonNullObject("")).toBe(false);
  });

  it("returns false for a number", () => {
    expect(isNonNullObject(42)).toBe(false);
    expect(isNonNullObject(0)).toBe(false);
    expect(isNonNullObject(NaN)).toBe(false);
  });

  it("returns false for undefined", () => {
    expect(isNonNullObject(undefined)).toBe(false);
  });

  it("returns false for boolean", () => {
    expect(isNonNullObject(true)).toBe(false);
    expect(isNonNullObject(false)).toBe(false);
  });

  it("returns true for Date (typeof Date is 'object')", () => {
    expect(isNonNullObject(new Date())).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// applyTransform
// ---------------------------------------------------------------------------
describe("applyTransform", () => {
  // --- No spec / no args / identity (parameterized) -------------------
  it.each<[string, string | undefined, Record<string, unknown> | undefined, "empty" | "identity"]>([
    ["returns args unchanged when spec is undefined", undefined, { foo: "bar" }, "identity"],
    ["returns empty object when args is undefined", "download_list", undefined, "empty"],
    ["returns empty object when args is undefined even without spec", undefined, undefined, "empty"],
    ["identity transform returns args unchanged", "download_list", { someField: "value" }, "identity"],
    ["identity transform preserves the same object reference", "settings_get", { a: 1, b: 2 }, "identity"],
  ])("%s", (_title, tauriName, args, mode) => {
    const spec = tauriName ? WS_COMMANDS.find((c) => c.tauriName === tauriName)! : undefined;
    const result = applyTransform(spec, args);
    if (mode === "identity") {
      if (spec) {
        expect(spec.paramTransform).toEqual({ kind: "identity" });
      }
      expect(result).toBe(args);
    } else {
      expect(result).toEqual({});
    }
  });

  // --- Rename ------------------------------------------------------------
  it("rename transform renames the matching field", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_pause")!;
    expect(spec.paramTransform).toEqual({
      kind: "rename",
      from: "downloadId",
      to: "taskId",
    });

    const result = applyTransform(spec, { downloadId: "abc-123" });
    expect(result).toEqual({ taskId: "abc-123" });
    // original field should be gone
    expect(result).not.toHaveProperty("downloadId");
  });

  it("rename transform preserves other fields alongside the renamed one", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_pause")!;

    const result = applyTransform(spec, {
      downloadId: "abc-123",
      extra: "keep-me",
    });
    expect(result).toEqual({ taskId: "abc-123", extra: "keep-me" });
  });

  it("rename transform returns args unchanged when the source field is absent", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_pause")!;

    const args = { unrelated: "value" };
    expect(applyTransform(spec, args)).toBe(args);
  });

  it("rename transform works for all rename-type commands", () => {
    const renameCommands = WS_COMMANDS.filter((c) => c.paramTransform.kind === "rename");
    expect(renameCommands.length).toBeGreaterThan(0);

    for (const spec of renameCommands) {
      if (spec.paramTransform.kind !== "rename") continue;
      const { from, to } = spec.paramTransform;
      const args = { [from]: "some-id" };
      const result = applyTransform(spec, args);
      expect(result).toEqual({ [to]: "some-id" });
      expect(result).not.toHaveProperty(from);
    }
  });

  it("rename transform does not mutate the original args object", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_pause")!;
    const args = { downloadId: "abc", extra: "keep-me" };
    const argsCopy = { ...args };

    applyTransform(spec, args);
    // original should be untouched
    expect(args).toEqual(argsCopy);
  });

  // --- UnwrapField -------------------------------------------------------
  it("unwrapField extracts the inner field object", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_start")!;
    expect(spec.paramTransform).toEqual({
      kind: "unwrapField",
      field: "request",
    });

    const result = applyTransform(spec, {
      request: { url: "https://example.com/file", filename: "test" },
    });
    expect(result).toEqual({ url: "https://example.com/file", filename: "test" });
  });

  it("unwrapField returns args unchanged when the field value is not an object", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_start")!;

    const args = { request: "string-instead-of-object" };
    expect(applyTransform(spec, args)).toBe(args);
  });

  it("unwrapField returns args unchanged when the field is missing", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_start")!;

    const args = { unrelated: "value" };
    expect(applyTransform(spec, args)).toBe(args);
  });

  it("unwrapField works for settings_save command", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "settings_save")!;
    expect(spec.paramTransform).toEqual({
      kind: "unwrapField",
      field: "settings",
    });

    const settings = { theme: "dark", lang: "en" };
    const result = applyTransform(spec, { settings });
    expect(result).toEqual(settings);
  });

  it("unwrapField does not mutate the original args", () => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === "download_start")!;
    const inner = { url: "https://example.com/file" };
    const args = { request: inner };

    applyTransform(spec, args);
    expect(args.request).toBe(inner);
  });

  // --- Edge cases --------------------------------------------------------
  it.each<[string, string]>([
    ["returns empty args object when args is undefined even for identity spec", "download_list"],
    ["returns empty args object when args is undefined for rename spec", "download_pause"],
    ["returns empty args object when args is undefined for unwrapField spec", "download_start"],
  ])("%s", (_title, tauriName) => {
    const spec = WS_COMMANDS.find((c) => c.tauriName === tauriName)!;
    expect(applyTransform(spec, undefined)).toEqual({});
  });
});

// ---------------------------------------------------------------------------
// mapEventType
// ---------------------------------------------------------------------------
describe("mapEventType", () => {
  it("maps 'updated' to 'download-updated'", () => {
    expect(mapEventType("updated", null)).toBe("download-updated");
  });

  it("maps 'progress' to 'download-progress'", () => {
    expect(mapEventType("progress", null)).toBe("download-progress");
  });

  it("maps 'aria2Notification' to 'aria2-notification'", () => {
    expect(mapEventType("aria2Notification", null)).toBe("aria2-notification");
  });

  it("maps 'cdnProgress' to 'cdn-test-progress'", () => {
    expect(mapEventType("cdnProgress", null)).toBe("cdn-test-progress");
  });

  it("maps 'cdnComplete' to 'cdn-test-complete'", () => {
    expect(mapEventType("cdnComplete", null)).toBe("cdn-test-complete");
  });

  it("maps 'warning' to 'download-warning'", () => {
    expect(mapEventType("warning", null)).toBe("download-warning");
  });

  it("returns null for an unknown event type", () => {
    expect(mapEventType("nonexistent", null)).toBeNull();
  });

  it("returns null for an empty string", () => {
    expect(mapEventType("", null)).toBeNull();
  });

  it("ignores payload parameter (payload is unused in mapping)", () => {
    // The _payload parameter is accepted but not used in the mapping logic
    const result1 = mapEventType("progress", { some: "data" });
    const result2 = mapEventType("progress", null);
    expect(result1).toBe(result2);
  });

  it("maps all known event types from WS_EVENTS without error", () => {
    for (const ev of WS_EVENTS) {
      expect(mapEventType(ev.wsType, null)).toBe(ev.tauriEventName);
    }
  });
});
