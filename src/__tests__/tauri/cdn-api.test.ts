import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { createMockInvoke, resetTauriMocks, mockTauriCommandValue } from "../mocks/tauri-mock";
import {
  fetchCloudflareRanges,
  testAcceleration,
  applyAcceleration,
  clearAcceleration,
  getAccelerationStatus,
  cancelAcceleration,
  getAccelerationDetail,
  getAccelerationCandidates,
} from "../../lib/tauri/cdn-api";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  resetTauriMocks();
  mockInvoke.mockImplementation(createMockInvoke());
  vi.clearAllMocks();
});

describe("cdn-api", () => {
  it("fetchCloudflareRanges calls cdn_fetch_ranges", async () => {
    const ranges = ["173.245.48.0/20", "103.21.244.0/22"];
    mockTauriCommandValue("cdn_fetch_ranges", ranges);

    const result = await fetchCloudflareRanges();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_fetch_ranges");
    expect(result).toEqual(ranges);
  });

  it("testAcceleration calls cdn_test", async () => {
    mockTauriCommandValue("cdn_test", undefined);

    const result = await testAcceleration();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_test");
    expect(result).toBeUndefined();
  });

  it("applyAcceleration calls cdn_apply with ip and speedMbps", async () => {
    const ip = "1.1.1.1";
    const speedMbps = 150.5;
    mockTauriCommandValue("cdn_apply", undefined);

    const result = await applyAcceleration(ip, speedMbps);

    expect(mockInvoke).toHaveBeenCalledWith("cdn_apply", { ip, speedMbps });
    expect(result).toBeUndefined();
  });

  it("clearAcceleration calls cdn_clear", async () => {
    mockTauriCommandValue("cdn_clear", undefined);

    const result = await clearAcceleration();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_clear");
    expect(result).toBeUndefined();
  });

  it("getAccelerationStatus calls cdn_status", async () => {
    const status = "active";
    mockTauriCommandValue("cdn_status", status);

    const result = await getAccelerationStatus();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_status");
    expect(result).toBe(status);
  });

  it("cancelAcceleration calls cdn_cancel", async () => {
    mockTauriCommandValue("cdn_cancel", undefined);

    const result = await cancelAcceleration();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_cancel");
    expect(result).toBeUndefined();
  });

  it("getAccelerationDetail calls cdn_detail", async () => {
    const detail = { ip: "1.1.1.1", speedMbps: 100, latencyMs: 10 };
    mockTauriCommandValue("cdn_detail", detail);

    const result = await getAccelerationDetail();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_detail");
    expect(result).toEqual(detail);
  });

  it("getAccelerationCandidates calls cdn_candidates", async () => {
    const candidates = [
      { ip: "1.1.1.1", speedMbps: 200, latencyMs: 5 },
      { ip: "1.0.0.1", speedMbps: 180, latencyMs: 8 },
    ];
    mockTauriCommandValue("cdn_candidates", candidates);

    const result = await getAccelerationCandidates();

    expect(mockInvoke).toHaveBeenCalledWith("cdn_candidates");
    expect(result).toEqual(candidates);
  });
});
