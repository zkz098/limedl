import { describe, it, expect, vi, beforeEach } from "vitest";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("../../composables/useNotification", () => ({
  useNotification: () => ({
    notifyInfo: vi.fn(),
    notifyError: vi.fn(),
    notifySuccess: vi.fn(),
  }),
}));

// ── Imports (after mocks) ──────────────────────────────────────────

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const mockCheck = vi.mocked(check);
const mockRelaunch = vi.mocked(relaunch);

// ── Helpers ─────────────────────────────────────────────────────────

/**
 * Re-import the module to get a fresh singleton state between tests.
 * vi.mock persists, so dependency mocks survive resetModules().
 */
async function createAppUpdate() {
  vi.resetModules();
  const fresh = await import("../../composables/useAppUpdate");
  return fresh.useAppUpdate();
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mockUpdateResult(overrides: Record<string, any> = {}): any {
  return {
    version: "2.0.0",
    currentVersion: "1.0.0",
    body: "Release notes",
    date: "2025-01-01",
    downloadAndInstall: vi.fn(),
    download: vi.fn(),
    available: true,
    rawJson: null,
    install: vi.fn(),
    close: vi.fn(),
    ...overrides,
  };
}

// ── Tests ───────────────────────────────────────────────────────────

describe("useAppUpdate", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  // ── Initial state ──────────────────────────────────────────

  it("starts with idle status and default stable channel", async () => {
    const app = await createAppUpdate();

    expect(app.status.value).toBe("idle");
    expect(app.channel.value).toBe("stable");
    expect(app.updateAvailable.value).toBe(false);
    expect(app.errorMessage.value).toBe("");
    expect(app.progressPercent.value).toBe(0);
  });

  it("reads channel from localStorage on init", async () => {
    localStorage.setItem("limedl.updateChannel", "beta");
    const app = await createAppUpdate();
    expect(app.channel.value).toBe("beta");
  });

  // ── Computed flags ─────────────────────────────────────────

  it("computed flags are false when idle", async () => {
    const app = await createAppUpdate();
    expect(app.isChecking.value).toBe(false);
    expect(app.isDownloading.value).toBe(false);
    expect(app.isInstalling.value).toBe(false);
  });

  it("computed flags reflect checking status", async () => {
    mockCheck.mockRejectedValueOnce(new Error("fail"));
    const app = await createAppUpdate();

    // Start check and peek at intermediate state before await
    const promise = app.checkForUpdates();

    // During the check, status should be "checking"
    expect(app.status.value).toBe("checking");
    expect(app.isChecking.value).toBe(true);
    expect(app.isDownloading.value).toBe(false);
    expect(app.isInstalling.value).toBe(false);

    await promise;
  });

  // ── setChannel ─────────────────────────────────────────────

  it("setChannel changes channel and persists to localStorage", async () => {
    const app = await createAppUpdate();

    app.setChannel("beta");

    expect(app.channel.value).toBe("beta");
    expect(localStorage.getItem("limedl.updateChannel")).toBe("beta");
  });

  it("setChannel resets state to idle", async () => {
    const app = await createAppUpdate();

    // Put it in a non-idle state first
    mockCheck.mockRejectedValueOnce(new Error("fail"));
    await app.checkForUpdates();
    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).not.toBe("");

    app.setChannel("beta");

    expect(app.status.value).toBe("idle");
    expect(app.errorMessage.value).toBe("");
  });

  it("setChannel back to stable works", async () => {
    const app = await createAppUpdate();

    app.setChannel("beta");
    expect(app.channel.value).toBe("beta");

    app.setChannel("stable");
    expect(app.channel.value).toBe("stable");
    expect(localStorage.getItem("limedl.updateChannel")).toBe("stable");
  });

  // ── checkForUpdates: up-to-date ────────────────────────────

  it("checkForUpdates transitions idle → checking → up-to-date when no update", async () => {
    mockCheck.mockResolvedValueOnce(null);
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("up-to-date");
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── checkForUpdates: available ─────────────────────────────

  it("checkForUpdates transitions idle → checking → available when update found", async () => {
    const update = mockUpdateResult({ version: "2.0.0" });
    mockCheck.mockResolvedValueOnce(update);
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBe(update);
    expect(app.status.value).toBe("available");
    expect(app.updateAvailable.value).toBe(true);
    expect(app.latestVersion.value).toBe("2.0.0");
    expect(app.currentVersion.value).toBe("1.0.0");
    expect(app.latestBody.value).toBe("Release notes");
    expect(app.latestDate.value).toBe("2025-01-01");
  });

  // ── checkForUpdates: newer local version ───────────────────

  it("checkForUpdates goes to 'newer' when local version is ahead", async () => {
    const update = mockUpdateResult({ currentVersion: "3.0.0", version: "2.0.0" });
    mockCheck.mockResolvedValueOnce(update);
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("newer");
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── checkForUpdates: error ─────────────────────────────────

  it("checkForUpdates sets error state on failure", async () => {
    mockCheck.mockRejectedValueOnce(new Error("Network timeout"));
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe("Network timeout");
  });

  // ── checkForUpdates: busy guard ────────────────────────────

  it("checkForUpdates returns null when already busy", async () => {
    mockCheck.mockRejectedValueOnce(new Error("fail"));
    const app = await createAppUpdate();

    // Start check but don't await — status is now "checking"
    const promise1 = app.checkForUpdates();

    // Second call while busy should return null immediately
    const result2 = await app.checkForUpdates();
    expect(result2).toBeNull();

    await promise1;
  });

  // ── downloadAndInstall ─────────────────────────────────────

  it("downloadAndInstall transitions downloading → installing", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    const update = mockUpdateResult({ downloadAndInstall });
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    // First check for update
    await app.checkForUpdates();
    expect(app.status.value).toBe("available");

    // Now download and install
    mockRelaunch.mockResolvedValueOnce(undefined);
    await app.downloadAndInstall();

    expect(app.status.value).toBe("installing");
    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(mockRelaunch).toHaveBeenCalledTimes(1);
  });

  it("downloadAndInstall reports progress during download", async () => {
    const downloadAndInstall = vi
      .fn()
      .mockImplementation(
        (
          onEvent: (event: {
            event: string;
            data: { contentLength?: number; chunkLength?: number };
          }) => void,
        ) => {
          // Simulate progress events
          onEvent({ event: "Started", data: { contentLength: 1000 } });
          onEvent({ event: "Progress", data: { chunkLength: 300 } });
          onEvent({ event: "Progress", data: { chunkLength: 200 } });
          onEvent({ event: "Finished", data: {} });
          return Promise.resolve();
        },
      );
    const update = mockUpdateResult({
      downloadAndInstall,
      version: "2.0.0",
      currentVersion: "1.0.0",
    });
    mockCheck.mockResolvedValueOnce(update);
    mockRelaunch.mockResolvedValueOnce(undefined);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    await app.downloadAndInstall();

    // After Progress events: (300+200)/1000 = 50%, but capped at 99 during download
    // Actually, Finished sets to 100
    expect(app.progressPercent.value).toBe(100);
    expect(app.totalBytes.value).toBe(1000);
    expect(app.downloadedBytes.value).toBe(500);
  });

  it("downloadAndInstall sets error on failure with generic message", async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error("Connection lost"));
    const update = mockUpdateResult({ downloadAndInstall });
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    await app.downloadAndInstall();

    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe("Connection lost");
  });

  it("downloadAndInstall sets disk space error message", async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error("Not enough disk space"));
    const update = mockUpdateResult({ downloadAndInstall });
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    await app.downloadAndInstall();

    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe("settings.aboutDiskSpaceInsufficient");
  });

  it("downloadAndInstall sets signature error message", async () => {
    const downloadAndInstall = vi
      .fn()
      .mockRejectedValue(new Error("signature verification failed"));
    const update = mockUpdateResult({ downloadAndInstall });
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    await app.downloadAndInstall();

    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe("settings.aboutSignatureInvalid");
  });

  it("downloadAndInstall goes up-to-date if no update object", async () => {
    mockCheck.mockResolvedValueOnce(null);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    expect(app.status.value).toBe("up-to-date");

    // Try to download when no update was found
    mockCheck.mockResolvedValueOnce(null); // fallback check also null
    await app.downloadAndInstall();

    expect(app.status.value).toBe("up-to-date");
  });

  // ── acknowledgeUpdate ──────────────────────────────────────

  it("acknowledgeUpdate sets updateAvailable to false", async () => {
    const update = mockUpdateResult();
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    expect(app.updateAvailable.value).toBe(true);

    app.acknowledgeUpdate();
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── runStartupCheck ────────────────────────────────────────

  it("runStartupCheck runs check silently when idle", async () => {
    mockCheck.mockResolvedValueOnce(null);

    const app = await createAppUpdate();

    await app.runStartupCheck();

    expect(mockCheck).toHaveBeenCalledTimes(1);
    expect(app.status.value).toBe("up-to-date");
  });

  it("runStartupCheck does nothing when not idle", async () => {
    mockCheck.mockRejectedValueOnce(new Error("fail"));
    const app = await createAppUpdate();

    // First check fails
    await app.checkForUpdates();
    expect(app.status.value).toBe("error");

    // Attempt startup check — should be a no-op since status is "error", not "idle"
    mockCheck.mockClear();
    await app.runStartupCheck();

    // check() should NOT have been called again
    expect(mockCheck).not.toHaveBeenCalled();
  });

  // ── normalizeVersion ───────────────────────────────────────

  it("latestVersion strips leading 'v' prefix", async () => {
    const update = mockUpdateResult({ version: "v2.0.0", currentVersion: "v1.0.0" });
    mockCheck.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();

    expect(app.currentVersion.value).toBe("1.0.0");
    expect(app.latestVersion.value).toBe("2.0.0");
  });
});
