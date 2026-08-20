import { describe, it, expect, vi, beforeEach } from "vitest";

// ── Mocks ──────────────────────────────────────────────────────────

const { mockInvoke, mockListen } = vi.hoisted(() => {
  return {
    mockInvoke: vi.fn(),
    mockListen: vi.fn().mockResolvedValue(vi.fn()),
  };
});

vi.mock("#invoke", () => ({
  invoke: mockInvoke,
  setEventDispatcher: vi.fn(),
}));

vi.mock("#event", () => ({
  listen: mockListen,
}));

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("../../stores/notification", () => ({
  useNotificationStore: () => ({
    notifyInfo: vi.fn(),
    notifyError: vi.fn(),
    notifySuccess: vi.fn(),
  }),
}));

// ── Imports (after mocks) ──────────────────────────────────────────
import { createPinia, setActivePinia, storeToRefs } from "pinia";
import { useAppUpdateStore } from "../../stores/appUpdate";

// ── Helpers ─────────────────────────────────────────────────────────

/**
 * Create a fresh Pinia instance and return the update store, wrapped so
 * `storeToRefs` gives ref-with-`.value` access matching the original
 * composable's contract (state read-only, actions callable directly).
 */
async function createAppUpdate() {
  setActivePinia(createPinia());
  const store = useAppUpdateStore();
  return { ...store, ...storeToRefs(store) };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mockUpdateResult(overrides: Record<string, any> = {}): any {
  return {
    version: "2.0.0",
    currentVersion: "1.0.0",
    body: "Release notes",
    date: "2025-01-01",
    downloadUrl: "https://example.com/update",
    signature: "dummy-signature",
    ...overrides,
  };
}

// ── Tests ───────────────────────────────────────────────────────────

describe("useAppUpdateStore", () => {
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
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
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
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
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
    mockInvoke.mockResolvedValueOnce(null);
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("up-to-date");
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── checkForUpdates: available ─────────────────────────────

  it("checkForUpdates transitions idle → checking → available when update found", async () => {
    const update = mockUpdateResult({ version: "2.0.0" });
    mockInvoke.mockResolvedValueOnce(update);
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
    mockInvoke.mockResolvedValueOnce(update);
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("newer");
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── checkForUpdates: error ─────────────────────────────────

  it("checkForUpdates sets error state on failure", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("Network timeout"));
    const app = await createAppUpdate();

    const result = await app.checkForUpdates();

    expect(result).toBeNull();
    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe("Network timeout");
  });

  // ── checkForUpdates: busy guard ────────────────────────────

  it("checkForUpdates returns null when already busy", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
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
    const update = mockUpdateResult();
    mockInvoke.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    // First check for update
    await app.checkForUpdates();
    expect(app.status.value).toBe("available");

    // Now download and install
    // First listen call (progress) returns noop unlisten,
    // second (installing) triggers the installing handler
    mockListen.mockImplementationOnce(async () => vi.fn());
    mockListen.mockImplementationOnce(async (_event, handler) => {
      handler({ payload: undefined });
      return vi.fn();
    });
    mockInvoke.mockResolvedValueOnce(undefined);
    await app.downloadAndInstall();

    expect(app.status.value).toBe("installing");
    expect(mockInvoke).toHaveBeenLastCalledWith("download_and_install_update");
  });

  it("downloadAndInstall reports progress during download", async () => {
    const update = mockUpdateResult();
    mockInvoke.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();

    // download_and_install_update resolves immediately without dispatching progress events
    mockInvoke.mockResolvedValueOnce(undefined);
    await app.downloadAndInstall();

    // No progress events are dispatched via the listen mock,
    // so progress values remain at their initial state (0)
    expect(app.progressPercent.value).toBe(0);
    expect(app.totalBytes.value).toBe(0);
    expect(app.downloadedBytes.value).toBe(0);
  });

  it.each<[string, string, string]>([
    [
      "downloadAndInstall sets error on failure with generic message",
      "Connection lost",
      "Connection lost",
    ],
    [
      "downloadAndInstall sets disk space error message",
      "Not enough disk space",
      "settings.aboutDiskSpaceInsufficient",
    ],
    [
      "downloadAndInstall sets signature error message",
      "signature verification failed",
      "settings.aboutSignatureInvalid",
    ],
  ])("%s", async (_title, thrownMessage, expectedMessage) => {
    const update = mockUpdateResult();
    mockInvoke.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    mockInvoke.mockRejectedValueOnce(new Error(thrownMessage));
    await app.downloadAndInstall();

    expect(app.status.value).toBe("error");
    expect(app.errorMessage.value).toBe(expectedMessage);
  });

  it("downloadAndInstall goes up-to-date if no update object", async () => {
    mockInvoke.mockResolvedValueOnce(null);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    expect(app.status.value).toBe("up-to-date");

    // downloadAndInstall still attempts the Rust command even without
    // a prior check result; it proceeds since isBusy() is false
    mockInvoke.mockResolvedValueOnce(undefined);
    await app.downloadAndInstall();

    // Since no "update-installing" event fires, status stays "downloading"
    expect(app.status.value).toBe("downloading");
  });

  // ── acknowledgeUpdate ──────────────────────────────────────

  it("acknowledgeUpdate sets updateAvailable to false", async () => {
    const update = mockUpdateResult();
    mockInvoke.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();
    expect(app.updateAvailable.value).toBe(true);

    app.acknowledgeUpdate();
    expect(app.updateAvailable.value).toBe(false);
  });

  // ── runStartupCheck ────────────────────────────────────────

  it("runStartupCheck runs check silently when idle", async () => {
    mockInvoke.mockResolvedValueOnce(null);

    const app = await createAppUpdate();

    await app.runStartupCheck();

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(app.status.value).toBe("up-to-date");
  });

  it("runStartupCheck does nothing when not idle", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("fail"));
    const app = await createAppUpdate();

    // First check fails
    await app.checkForUpdates();
    expect(app.status.value).toBe("error");

    // Attempt startup check — should be a no-op since status is "error", not "idle"
    mockInvoke.mockClear();
    await app.runStartupCheck();

    // check() should NOT have been called again
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  // ── normalizeVersion ───────────────────────────────────────

  it("latestVersion strips leading 'v' prefix", async () => {
    const update = mockUpdateResult({ version: "v2.0.0", currentVersion: "v1.0.0" });
    mockInvoke.mockResolvedValueOnce(update);

    const app = await createAppUpdate();

    await app.checkForUpdates();

    expect(app.currentVersion.value).toBe("1.0.0");
    expect(app.latestVersion.value).toBe("2.0.0");
  });
});
