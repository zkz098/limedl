import { describe, it, expect, vi } from "vitest";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  formatTimestamp,
  formatTokenLabel,
  isSizeUnknown,
  progressLabel,
  progressValue,
  stateLabel,
} from "../../lib/download-format";

vi.mock("../../i18n", () => ({
  t: vi.fn((key: string) => key),
}));

describe("formatBytes", () => {
  it('returns "—" for undefined', () => {
    expect(formatBytes(undefined)).toBe("—");
  });

  it('returns "—" for NaN', () => {
    expect(formatBytes(NaN)).toBe("—");
  });

  it('returns "0 B" for 0', () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it('returns "500 B" for 500', () => {
    expect(formatBytes(500)).toBe("500 B");
  });

  it('returns "1.0 KB" for 1024', () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
  });

  it('returns "1.5 KB" for 1536', () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it('returns "1.0 MB" for 1048576', () => {
    expect(formatBytes(1048576)).toBe("1.0 MB");
  });

  it('returns "1.5 GB" for 1610612736', () => {
    expect(formatBytes(1610612736)).toBe("1.5 GB");
  });

  it('returns "1.0 TB" for 1099511627776', () => {
    expect(formatBytes(1099511627776)).toBe("1.0 TB");
  });

  it('returns "100 MB" for 100 * 1024 * 1024', () => {
    // size >= 100 uses 0 decimal places
    expect(formatBytes(100 * 1024 * 1024)).toBe("100 MB");
  });
});

describe("formatSpeed", () => {
  it('returns "—" for undefined', () => {
    expect(formatSpeed(undefined)).toBe("—");
  });

  it('returns "1.0 MB/s" for 1048576', () => {
    expect(formatSpeed(1048576)).toBe("1.0 MB/s");
  });

  it('returns "0 B/s" for 0', () => {
    expect(formatSpeed(0)).toBe("0 B/s");
  });
});

describe("formatEta", () => {
  it('returns "—" for undefined', () => {
    expect(formatEta(undefined)).toBe("—");
  });

  it('returns "59s" for 59', () => {
    expect(formatEta(59)).toBe("59s");
  });

  it('returns "1m 0s" for 60', () => {
    expect(formatEta(60)).toBe("1m 0s");
  });

  it('returns "1m 30s" for 90', () => {
    expect(formatEta(90)).toBe("1m 30s");
  });

  it('returns "1h 0s" for 3600', () => {
    // minutes=0 is omitted, seconds always shown
    expect(formatEta(3600)).toBe("1h 0s");
  });

  it('returns "2h 30m 15s" for 9015', () => {
    expect(formatEta(9015)).toBe("2h 30m 15s");
  });
});

describe("formatTimestamp", () => {
  it('returns "—" for undefined', () => {
    expect(formatTimestamp(undefined)).toBe("—");
  });

  it("returns formatted date string for a valid timestamp", () => {
    const result = formatTimestamp(1700000000000);
    expect(result).not.toBe("—");
    // Should contain some digits (year, date, or time numbers)
    expect(result).toMatch(/\d/);
  });
});

describe("stateLabel", () => {
  it("returns translated label for known state", () => {
    expect(stateLabel("completed")).toBe("states.completed");
  });

  it("returns fallback for undefined state", () => {
    expect(stateLabel(undefined)).toBe("common.unknown");
  });
});

describe("isSizeUnknown", () => {
  it("returns true when totalBytes is undefined", () => {
    expect(isSizeUnknown({ downloadedBytes: 0, totalBytes: undefined, state: "downloading" })).toBe(
      true,
    );
  });

  it("returns true when totalBytes is 0", () => {
    expect(isSizeUnknown({ downloadedBytes: 0, totalBytes: 0, state: "downloading" })).toBe(true);
  });

  it("returns false when totalBytes > 0", () => {
    expect(isSizeUnknown({ downloadedBytes: 0, totalBytes: 500, state: "downloading" })).toBe(
      false,
    );
  });
});

describe("progressValue", () => {
  it("returns 0 when totalBytes is 0 and not completed", () => {
    expect(progressValue({ downloadedBytes: 0, totalBytes: 0, state: "downloading" })).toBe(0);
  });

  it("returns 100 when state is completed and totalBytes is 0", () => {
    expect(progressValue({ downloadedBytes: 0, totalBytes: 0, state: "completed" })).toBe(100);
  });

  it("returns 50 when downloadedBytes is half of totalBytes", () => {
    expect(progressValue({ downloadedBytes: 50, totalBytes: 100, state: "downloading" })).toBe(50);
  });

  it("returns 100 when downloadedBytes equals totalBytes", () => {
    expect(progressValue({ downloadedBytes: 100, totalBytes: 100, state: "downloading" })).toBe(
      100,
    );
  });

  it("returns 100 (capped) when downloadedBytes > totalBytes", () => {
    expect(progressValue({ downloadedBytes: 200, totalBytes: 100, state: "downloading" })).toBe(
      100,
    );
  });
});

describe("progressLabel", () => {
  it('returns "100%" when completed with unknown size', () => {
    expect(progressLabel({ downloadedBytes: 0, totalBytes: 0, state: "completed" })).toBe("100%");
  });

  it("returns pending label when not completed with unknown size", () => {
    expect(progressLabel({ downloadedBytes: 0, totalBytes: 0, state: "downloading" })).toBe(
      "queue.pendingSize",
    );
  });

  it('returns "50.0%" when at half progress', () => {
    expect(progressLabel({ downloadedBytes: 50, totalBytes: 100, state: "downloading" })).toBe(
      "50.0%",
    );
  });
});

describe("formatTokenLabel", () => {
  it("returns fallback for undefined", () => {
    expect(formatTokenLabel(undefined)).toBe("common.unknown");
  });

  it("returns translated token label for known value", () => {
    expect(formatTokenLabel("bt")).toBe("tokens.bt");
  });
});
