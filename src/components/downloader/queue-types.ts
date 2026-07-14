import type { SortDirection, SortKey } from "../../types/settings";

export interface ViewOptions {
  sortKey: SortKey;
  sortDirection: SortDirection;
  compactView: boolean;
  visibleColumns: string[];
}

export interface MultiSelectState {
  multiSelectMode: boolean;
  selectedIds: Set<string>;
  removedDownloadIds: string[];
}
