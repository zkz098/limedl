import type { DownloadSummary } from "../types/download";

export function filterDownloads(
  downloads: DownloadSummary[],
  searchQuery: string,
  stateFilter: string,
): DownloadSummary[] {
  let list = downloads;
  const query = searchQuery.trim().toLowerCase();
  if (query) {
    list = list.filter(
      (d) => d.fileName.toLowerCase().includes(query) || d.url.toLowerCase().includes(query),
    );
  }
  if (stateFilter) {
    list = list.filter((d) => d.state === stateFilter);
  }
  return list;
}
