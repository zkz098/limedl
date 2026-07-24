import type { TaskKind } from "../types/download";

export function detectKindFromUrl(url: string): TaskKind {
  const trimmed = url.trim().toLowerCase();
  if (trimmed.startsWith("magnet:")) return "bt";
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed.endsWith(".torrent") ? "bt" : "http";
  }
  if (/^[0-9a-f]{40}$/i.test(trimmed) || trimmed.startsWith("xt=urn:btih:")) return "bt";
  return "http";
}

export function extractFileNameFromUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) return "";
  if (trimmed.toLowerCase().startsWith("magnet:")) {
    const queryIndex = trimmed.indexOf("?");
    const query = queryIndex >= 0 ? trimmed.slice(queryIndex + 1) : "";
    const dn = new URLSearchParams(query).get("dn");
    return dn ? decodeURIComponent(dn) : "";
  }
  try {
    const parsed = new URL(trimmed);
    const segment = parsed.pathname.split("/").pop();
    return segment ? decodeURIComponent(segment) : "";
  } catch {
    return "";
  }
}
