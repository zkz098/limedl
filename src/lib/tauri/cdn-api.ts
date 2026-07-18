import { invoke } from "@tauri-apps/api/core";
import type { CdnDetail, SpeedTestCandidate } from "../../types/cdn";

export function fetchCloudflareRanges() {
  return invoke<string[]>("cdn_fetch_ranges");
}

export function testAcceleration() {
  return invoke<void>("cdn_test");
}

export function applyAcceleration(ip: string, speedMbps: number) {
  return invoke<void>("cdn_apply", { ip, speedMbps });
}

export function clearAcceleration() {
  return invoke<void>("cdn_clear");
}

export function getAccelerationStatus() {
  return invoke<string>("cdn_status");
}

export function cancelAcceleration() {
  return invoke<void>("cdn_cancel");
}

export function getAccelerationDetail() {
  return invoke<CdnDetail>("cdn_detail");
}

export function getAccelerationCandidates() {
  return invoke<SpeedTestCandidate[]>("cdn_candidates");
}
