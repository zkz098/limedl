import { invoke } from "#invoke";
import { commandName } from "../ws/command-name";
import type { CdnDetail, SpeedTestCandidate } from "../../types/cdn";

export function fetchCloudflareRanges() {
  return invoke<string[]>(commandName("cdn_fetch_ranges"));
}

export function testAcceleration() {
  return invoke<void>(commandName("cdn_test"));
}

export function applyAcceleration(ip: string, speedMbps: number) {
  return invoke<void>(commandName("cdn_apply"), { ip, speedMbps });
}

export function clearAcceleration() {
  return invoke<void>(commandName("cdn_clear"));
}

export function getAccelerationStatus() {
  return invoke<string>(commandName("cdn_status"));
}

export function cancelAcceleration() {
  return invoke<void>(commandName("cdn_cancel"));
}

export function getAccelerationDetail() {
  return invoke<CdnDetail>(commandName("cdn_detail"));
}

export function getAccelerationCandidates() {
  return invoke<SpeedTestCandidate[]>(commandName("cdn_candidates"));
}
