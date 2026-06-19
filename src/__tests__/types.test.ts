import { describe, it, expect } from "vitest";
import type { CdnAccelerationSettings } from "../types/settings";

describe("CdnAccelerationSettings", () => {
  it("validates default shape", () => {
    const defaults: CdnAccelerationSettings = {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    };
    expect(defaults.enabled).toBe(false);
    expect(defaults.activeIp).toBeNull();
    expect(defaults.activeSpeedMbps).toBeNull();
    expect(defaults.lastTestAtMs).toBeNull();
    expect(defaults.lastError).toBeNull();
  });

  it("validates populated shape", () => {
    const settings: CdnAccelerationSettings = {
      enabled: true,
      activeIp: "192.168.1.100",
      activeSpeedMbps: 45.5,
      lastTestAtMs: 1700000000000,
      lastError: null,
    };
    expect(settings.enabled).toBe(true);
    expect(settings.activeIp).toBe("192.168.1.100");
    expect(settings.activeSpeedMbps).toBeCloseTo(45.5);
    expect(settings.lastTestAtMs).toBe(1700000000000);
  });
});
