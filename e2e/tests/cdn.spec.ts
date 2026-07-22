/**
 * CDN acceleration full-chain E2E tests for NAS WebUI.
 *
 * These tests use ws-mocker to simulate the CDN acceleration backend.
 * They verify:
 *   - Navigation to Labs page and CDN tab
 *   - Clicking "Test" triggers cdn.test RPC
 *   - Progress events update the UI
 *   - Complete event shows final IP and speed
 *   - Clicking "Apply" triggers cdn.apply RPC
 */

import { test, expect } from "../fixtures";

/**
 * Helper to create a mock CdnDetail payload matching the frontend type.
 */
function makeMockCdnDetail(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    state: "Ready",
    activeIp: "1.2.3.4",
    activeSpeedMbps: 45.67,
    phase: null,
    phaseProgress: null,
    candidates: [
      { ip: "1.2.3.4", tcpLatencyMs: 12.3, throughputMbps: 45.67, error: null },
      { ip: "5.6.7.8", tcpLatencyMs: 25.1, throughputMbps: 30.12, error: null },
    ],
    defaultNode: { ip: "198.51.100.1", tcpLatencyMs: 45.0, throughputMbps: 15.0, error: null },
    ...overrides,
  };
}

/**
 * Default mock AppSettings returned by settings.get, with CDN pre-configured.
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
    logging: { enabled: false, level: "info", filePath: "", retentionCount: null, retentionDays: null },
    aria2Rpc: { enabled: false, port: 6800, secret: null },
    cdnAcceleration: {
      enabled: true,
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
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    ...overrides,
  };
}

test.describe("CDN acceleration", () => {
  test.beforeEach(async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Mock settings.get so the app loads properly
    const settingsPromise = wsMocker.waitForMethod("settings.get");
    wsMocker.respondToMethod("settings.get", makeMockSettings());
    await settingsPromise;

    // Navigate to Labs page where CDN panel lives
    await page.getByRole("button", { name: "Labs" }).click();
    await expect(page.getByRole("heading", { name: /^Labs$/ })).toBeVisible();

    // Verify CDN panel is visible (default active tab)
    await expect(page.locator(".cdn-panel__status")).toBeVisible();
  });

  test("full CDN test flow: test, progress, result, and apply", async ({ page, wsMocker }) => {
    // The "Test and Accelerate" button should be visible
    const testButton = page.getByRole("button", { name: "Test and Accelerate" });
    await expect(testButton).toBeVisible();

    // Click "Test and Accelerate" — this triggers cdn.test RPC
    const cdnTestPromise = wsMocker.waitForMethod("cdn.test");
    await testButton.click();
    await cdnTestPromise;

    // Respond to cdn.test with success
    wsMocker.respondToMethod("cdn.test", null);

    // The CDN test starts polling via cdn.detail every 2s.
    // Respond to the first poll with an in-progress state.
    const detailPromise = wsMocker.waitForMethod("cdn.detail");
    await detailPromise;
    wsMocker.respondToMethod("cdn.detail", makeMockCdnDetail({
      state: "Testing",
      phase: "screening",
      phaseProgress: { current: 3, total: 10 },
      activeIp: null,
      activeSpeedMbps: null,
    }));

    // Send a cdnProgress event to update the UI
    wsMocker.sendEvent("cdnProgress", { phase: "screening", current: 3, total: 10 });

    // The progress section should now be visible
    await expect(page.locator(".cdn-panel__progress")).toBeVisible({ timeout: 5000 });

    // Send another progress event for measuring phase
    wsMocker.sendEvent("cdnProgress", { phase: "measuringThroughput", current: 5, total: 10 });

    // Set auto-response for subsequent cdn.detail calls so both the polling
    // interval and the cdnComplete event handler get the final result.
    // This avoids a race condition where the polling fires before or between
    // waitForMethod/respondToMethod, leaving the event handler's cdn.detail
    // call unanswered and the result panel hidden.
    wsMocker.setAutoResponse("cdn.detail", makeMockCdnDetail());

    // Send cdnComplete event — the event handler calls getAccelerationDetail()
    // which invokes cdn.detail, and the auto-response handles it.
    wsMocker.sendEvent("cdnComplete", makeMockCdnDetail());

    // Wait for the result card to appear with "Best IP" and speed info
    const resultCard = page.locator(".cdn-panel__result");
    await expect(resultCard).toBeVisible({ timeout: 5000 });

    // The result should show the active IP
    await expect(resultCard.getByText("1.2.3.4")).toBeVisible();
    // The result should show the speed (45.67 MB/s)
    await expect(resultCard.getByText("45.67 MB/s")).toBeVisible();

    // Find the "Apply" button for candidate "5.6.7.8" (not the active one)
    const candidatesSection = page.locator(".cdn-panel__candidates");
    await expect(candidatesSection).toBeVisible();

    // The candidates table rows — find the row for 5.6.7.8 and click Apply
    const candidateRow = candidatesSection.locator("tr").filter({ hasText: "5.6.7.8" });
    await expect(candidateRow).toBeVisible();
    const applyButton = candidateRow.getByRole("button", { name: "Apply" });
    await expect(applyButton).toBeVisible();

    // Clicking Apply triggers cdn.apply RPC
    const cdnApplyPromise = wsMocker.waitForMethod("cdn.apply");
    await applyButton.click();
    const applyParams = await cdnApplyPromise;
    expect(applyParams).toBeDefined();

    // Verify the apply was sent with the selected IP
    wsMocker.respondToMethod("cdn.apply", null);
  });
});
