import type { DownloadSnapshot } from "../../types/download";

export interface DownloadStoreOptions {
  /** Called when a download transitions to failed (for in-app notification) */
  onDownloadFailed?: (fileName: string, reason: string) => void;
  /** Called when one or more downloads are removed from the list */
  onDownloadsRemoved?: (removedIds: string[]) => void;
}

export interface BatchActionConfig {
  actionNameValue: string;
  items: Array<{ id: string; fileName: string }>;
  apiCall: (id: string) => Promise<DownloadSnapshot>;
  successMessageKey: string;
  onSuccess: (id: string, snapshot: DownloadSnapshot) => void;
}
