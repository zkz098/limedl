/**
 * Shared mock setup for download store tests.
 * Import this module at the top of any test file that tests the
 * download store or its consumers to register all shared mocks.
 *
 * @example
 * ```ts
 * import "../fixtures/download-store-mocks";
 * // or
 * import { setupDownloadStoreMocks } from "../fixtures/download-store-mocks";
 * setupDownloadStoreMocks();
 * ```
 */
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../i18n", () => ({
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) {
      const serialized = JSON.stringify(options);
      return `${key} ${serialized}`;
    }
    return key;
  }),
}));

vi.mock("../../lib/tauri/download-api", () => ({
  cancelDownload: vi.fn(),
  getBtRuntimeStatus: vi.fn(),
  getDownloadStatus: vi.fn(),
  listDownloads: vi.fn(),
  openDownloadInExplorer: vi.fn(),
  pauseDownload: vi.fn(),
  purgeDownload: vi.fn(),
  removeDownload: vi.fn(),
  resumeDownload: vi.fn(),
  setBtSpeedLimit: vi.fn(),
  setPriority: vi.fn(),
  startDownload: vi.fn(),
}));

vi.mock("../../stores/notification", () => ({
  useNotificationStore: () => ({
    notifySuccess: vi.fn(),
    notifyError: vi.fn(),
    notifyInfo: vi.fn(),
    notifyWarning: vi.fn(),
    clearAll: vi.fn(),
    notify: vi.fn(),
    dismiss: vi.fn(),
    notifications: { value: [] },
  }),
}));

vi.mock("#event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  onAction: vi.fn().mockResolvedValue({ unregister: vi.fn() }),
  requestPermission: vi.fn().mockResolvedValue("granted"),
  sendNotification: vi.fn(),
}));

/**
 * Ensure shared mocks are registered by importing this module.
 * Safe to call multiple times — vi.mock is hoisted and deduplicated.
 */
export function setupDownloadStoreMocks() {
  // All vi.mock() calls are at module scope above; this function
  // exists so callers can explicitly invoke it for readability.
}
