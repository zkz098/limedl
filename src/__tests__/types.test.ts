import { describe, it, expect } from "vitest";
import type { CdnAccelerationSettings } from "../types/settings";
import type { CdnDetail, SpeedTestCandidate, CdnTestProgress } from "../lib/tauri/cdn-api";

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

describe("CdnDetail", () => {
  it("has all required fields including new phase/progress/candidates", () => {
    const detail: CdnDetail = {
      state: "Ready",
      activeIp: "104.16.0.1",
      activeSpeedMbps: 50.5,
      phase: null,
      phaseProgress: null,
      candidates: [],
      defaultNode: null,
    };
    expect(detail.state).toBe("Ready");
    expect(detail.activeIp).toBe("104.16.0.1");
    expect(detail.phase).toBeNull();
    expect(detail.candidates).toEqual([]);
  });

  it("represents a testing state with phase progress", () => {
    const detail: CdnDetail = {
      state: "Testing",
      activeIp: null,
      activeSpeedMbps: null,
      phase: "Screening",
      phaseProgress: { current: 30, total: 45 },
      candidates: [],
      defaultNode: null,
    };
    expect(detail.phase).toBe("Screening");
    expect(detail.phaseProgress?.current).toBe(30);
  });
});

describe("SpeedTestCandidate", () => {
  it("represents a successful candidate", () => {
    const candidate: SpeedTestCandidate = {
      ip: "104.16.0.1",
      tcpLatencyMs: 12.5,
      throughputMbps: 45.3,
      error: null,
    };
    expect(candidate.ip).toBe("104.16.0.1");
    expect(candidate.throughputMbps).toBe(45.3);
  });

  it("represents a failed candidate", () => {
    const candidate: SpeedTestCandidate = {
      ip: "172.64.0.1",
      tcpLatencyMs: 150.0,
      throughputMbps: null,
      error: "connection reset",
    };
    expect(candidate.throughputMbps).toBeNull();
    expect(candidate.error).toBe("connection reset");
  });
});

describe("CdnTestProgress", () => {
  it("represents a screening progress event", () => {
    const progress: CdnTestProgress = {
      phase: "screening",
      current: 20,
      total: 45,
    };
    expect(progress.phase).toBe("screening");
    expect(progress.total).toBe(45);
  });
});
