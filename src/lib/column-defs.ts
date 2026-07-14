export type ColumnKey =
  | "file"
  | "size"
  | "downloaded"
  | "status"
  | "progress"
  | "speed"
  | "uploadSpeed"
  | "seeds"
  | "eta";

export const VALID_COLUMN_KEYS: ColumnKey[] = [
  "file",
  "size",
  "downloaded",
  "status",
  "progress",
  "speed",
  "uploadSpeed",
  "seeds",
  "eta",
];

export const DEFAULT_VISIBLE_COLUMNS: ColumnKey[] = [
  "file",
  "size",
  "downloaded",
  "status",
  "progress",
  "speed",
  "eta",
];

export const VALID_COLUMN_KEY_SET = new Set<string>(VALID_COLUMN_KEYS);
