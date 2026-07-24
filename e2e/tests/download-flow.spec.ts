/**
 * Download flow E2E tests
 *
 * These tests verify the complete user flow through the Tauri limedl UI:
 * navigation between views, opening the download composer, URL input,
 * form validation, and error handling.
 *
 * IMPORTANT: Full download E2E (start → progress → complete → verify)
 * requires a running test file server. See `src-tauri/src/download/test_harness.rs`
 * for the server (needs Tauri build with test features).
 *
 * Prerequisites:
 *   1. Run `bun run tauri dev` in a separate terminal
 *   2. Run `bun run test:e2e` from another terminal
 *
 * Tests are serial because they share app state (navigation, dialog state).
 */

import { test, expect } from "../fixtures";

/**
 * Default mock AppSettings returned by settings.get.
 */
function makeMockSettings(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "amber",
      backgroundOpacity: "default",
      colorMode: "dark",
      showDetailInfo: true,
      showHeatmap: true,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "automatic",
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 8,
        maxThreadsPerTask: 4,
        minThreadsPerTask: 1,
        adaptiveProfile: "balanced",
      },
      chunkSizeStrategy: "adaptive",
    },
    download: {
      defaultDownloadDir: "/downloads",
      defaultMaxRetries: 3,
      defaultChecksum: "none",
      defaultUserAgent: "",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "",
      listenPort: null,
      listenPortRange: null,
      upnpEnabled: true,
      enableNatpmp: true,
      enableIpv6: false,
      enablePex: true,
      enableLsd: true,
      enableUtp: true,
      enableFastExtension: true,
      enableHolepunch: true,
      enableWebSeed: true,
      enableSuperSeeding: false,
      globalDownloadRateLimit: 0,
      globalUploadRateLimit: 0,
      preallocateMode: "none",
      encryptionMode: "enabled",
      maxDownloads: 5,
      maxSeeds: 3,
      maxTorrents: 20,
      activeLimit: 10,
    },
    logging: {
      enabled: false,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: false, port: 6800, secret: null },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: {
      bufferLimitMb: 256,
      gameModeBufferMb: 64,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 2,
      gameModeMaxParallel: 1,
      hddBufferEnabled: true,
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    ...overrides,
  };
}

test.describe("download flow", () => {
  test.describe.configure({ mode: "serial" });

  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Wait for app to be fully loaded
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test("navigate to settings page", async ({ page }) => {
    // The sidebar bottom nav buttons have aria-labels set via i18n.
    // "Settings" corresponds to t("nav.settings").
    await page.getByRole("button", { name: "Settings" }).click();

    // SettingsPage renders inside a ModalOverlay when currentView === "settings".
    // The page heading uses t("settings.title") → "Settings".
    await expect(page.getByRole("heading", { name: /^Settings$/ })).toBeVisible();
  });

  test("navigate to labs page", async ({ page }) => {
    // "Labs" corresponds to t("nav.labs").
    await page.getByRole("button", { name: "Labs" }).click();

    // LabsPage renders inside a ModalOverlay when currentView === "labs".
    // The page heading uses t("labs.title") → "Labs".
    await expect(page.getByRole("heading", { name: /^Labs$/ })).toBeVisible();
  });

  test("navigate back to home", async ({ page }) => {
    // Navigate to settings first
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: /^Settings$/ })).toBeVisible();

    // Close the Settings modal overlay first, then navigate via sidebar
    await page.locator("button.overlay-close").click();
    await expect(page.getByRole("heading", { name: /^Settings$/ })).not.toBeVisible();

    // Navigate to home via the "Home" sidebar button
    await page.getByRole("button", { name: "Home" }).click();

    // Verify the download queue appears on the home view.
    await expect(page.getByRole("heading", { name: "Task List" })).toBeVisible();
  });

  test("open download composer dialog", async ({ page }) => {
    // The TopToolbar "Add Task" button emits @add-task which sets showComposerDialog = true.
    await page.getByRole("button", { name: "Add Task" }).click();

    // UiDialog teleports to body and renders with role="dialog".
    // The dialog title uses t("dialog.newTaskTitle") → "New Download Task".
    await expect(
      page.getByRole("dialog").getByRole("heading", { name: "New Download Task" }),
    ).toBeVisible();
  });

  test("fill download URL", async ({ page }) => {
    // Open composer dialog
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "New Download Task" })).toBeVisible();

    // The URL input has placeholder t("composer.sourceUrlPlaceholder")
    // → "Paste a link or choose a torrent file"
    const urlInput = dialog.getByPlaceholder("Paste a link or choose a torrent file");
    await expect(urlInput).toBeVisible();

    // Type a URL and verify
    const testUrl = "https://example.com/file.zip";
    await urlInput.fill(testUrl);
    await expect(urlInput).toHaveValue(testUrl);
  });

  test("validate empty URL shows error", async ({ page }) => {
    // Open composer dialog
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "New Download Task" })).toBeVisible();

    // Clear URL field (should already be empty) and click "Start download"
    const urlInput = dialog.getByPlaceholder("Paste a link or choose a torrent file");
    await urlInput.fill("");

    // Click the submit button — the form has @submit.prevent="$emit('submit')"
    // which calls handleSubmitStart → submitStart → checks for empty URL.
    // submitStart sets a notification error via notifyError().
    await dialog.getByRole("button", { name: "Start download" }).click();

    // The error notification appears in NotificationToast (role="alert").
    // t("messages.startRequired") → "URL and destination directory are required."
    await expect(
      page.getByRole("alert").getByText("URL and destination directory are required."),
    ).toBeVisible({ timeout: 5000 });
  });
});

test.describe("download flow — error handling", () => {
  test("shows error toast when download.start returns JSON-RPC error", async ({
    page,
    wsMocker,
  }) => {
    // Navigate fresh so wsMocker's routeWebSocket intercepts the WebSocket
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Wait for auto-response to settings.get to complete (registered in fixtures.ts)
    // then override with test-specific settings for proper app initialization
    await wsMocker.waitForMethod("settings.get");
    wsMocker.respondToMethod("settings.get", makeMockSettings());

    // Open composer dialog
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "New Download Task" })).toBeVisible();

    // Fill a valid-looking URL (passes client-side validation)
    const urlInput = dialog.getByPlaceholder("Paste a link or choose a torrent file");
    await urlInput.fill("https://example.com/file.zip");

    // Submit the form — this will trigger download_start RPC
    await dialog.getByRole("button", { name: "Start download" }).click();

    // Wait for the download.start RPC call (use rpcMethod, not tauriName)
    await wsMocker.waitForMethod("download.start");

    // Respond with a JSON-RPC error
    wsMocker.respondWithError("download.start", -32603, "Invalid URL: not a valid download source");

    // Verify an error toast appears with the error message
    await expect(
      page.getByRole("alert").filter({ hasText: "Invalid URL: not a valid download source" }),
    ).toBeVisible({ timeout: 5000 });
  });
});
