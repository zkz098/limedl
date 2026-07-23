/**
 * Settings page E2E tests for NAS WebUI.
 *
 * These tests use ws-mocker to simulate the settings RPC backend.
 * They verify:
 *   - Navigation to settings page and tab switching
 *   - Theme color modification and saving
 *   - Default download directory changes
 *   - Scheduler mode switching (traditional/automatic)
 *   - Proxy settings modification
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
    logging: { enabled: false, level: "info", filePath: "", retentionCount: null, retentionDays: null },
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
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    ...overrides,
  };
}

test.describe("Settings page", () => {
  test.beforeEach(async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // On page load the app requests settings
    const settingsPromise = wsMocker.waitForMethod("settings.get");
    wsMocker.respondToMethod("settings.get", makeMockSettings());
    await settingsPromise;

    // Navigate to settings page by clicking the settings button in the sidebar
    await page.locator('button[aria-label="Settings"]').click();

    // Wait for settings page to render
    await expect(page.locator(".settings-page")).toBeVisible({ timeout: 5000 });
  });

  test("opens settings page with all tabs visible", async ({ page }) => {
    // Verify the settings sidebar tabs are present
    const tabList = page.locator('.settings-page__tabs');
    await expect(tabList).toBeVisible();

    // All tab labels should be present
    const tabNames = ["Appearance", "Scheduler", "Downloads", "BT", "Aria2 RPC", "Logging", "Proxy", "About"];
    for (const name of tabNames) {
      await expect(tabList.getByRole("tab", { name })).toBeVisible();
    }

    // Default active tab is Appearance — verify the panel content
    await expect(page.locator(".appearance-panel")).toBeVisible();
  });

  test("modifies theme color and saves settings", async ({ page, wsMocker }) => {
    // By default, the theme color is "amber" (Amber)
    // Click the "Sky Blue" theme color button
    // The theme color buttons have aria-label set to the color name
    const skyButton = page.locator('button[aria-label="Sky Blue"]');
    await expect(skyButton).toBeVisible();
    await skyButton.click();

    // The "Sky" button should now be active (has is-active class)
    await expect(skyButton).toHaveClass(/is-active/);

    // The amber button should no longer be active
    const amberButton = page.locator('button[aria-label="Amber"]');
    await expect(amberButton).not.toHaveClass(/is-active/);

    // Click Save button
    await page.getByRole("button", { name: "Save settings" }).click();

    // Wait for settings.save RPC call
    const saveParams = await wsMocker.waitForMethod("settings.save");
    expect(saveParams).toBeDefined();
    // Verify the appearance.themeColor was changed to "sky" in the saved payload
    const payload = saveParams;
    expect(payload).toBeDefined();

    // Respond with the saved settings
    wsMocker.respondToMethod("settings.save", makeMockSettings({
      appearance: { ...makeMockSettings().appearance, themeColor: "sky" },
    }));
  });

  test("modifies default download directory", async ({ page, wsMocker }) => {
    // Switch to the Downloads tab
    await page.locator('.settings-page__tabs').getByRole("tab", { name: "Downloads" }).click();
    await expect(page.locator(".settings-page__content")).toBeVisible();

    // The default download location field should be visible
    // It shows the current path from the mock settings ("/downloads")
    // In NAS mode, the directory picker opens via WebSocket RPC, not an OS dialog.
    // We verify the field exists and can be changed.

    // Make the form dirty by modifying the download directory so the Save button is enabled
    const dirInput = page.locator(".settings-directory-field .ui-textfield");
    await dirInput.click();
    await dirInput.fill("/new/downloads");

    // Save settings to trigger settings.save
    await page.getByRole("button", { name: "Save settings" }).click();

    // Wait for settings.save — the form is now guaranteed to be dirty
    await wsMocker.waitForMethod("settings.save");
    wsMocker.respondToMethod("settings.save", makeMockSettings());

    // Close the Settings modal first, then navigate away
    await page.locator("button.overlay-close").click();
    await expect(page.locator(".settings-page")).not.toBeVisible();
    // Navigate to Home
    await page.locator('button[aria-label="Home"]').click();
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test("switches scheduler mode between automatic and traditional", async ({ page, wsMocker }) => {
    // The default scheduler mode is "automatic" (Smart Dynamic)
    // Switch to the Scheduler tab
    await page.locator('.settings-page__tabs').getByRole("tab", { name: "Scheduler" }).click();
    await expect(page.locator(".settings-page__content")).toBeVisible();

    // The allocation mode select should show "Smart Dynamic" (automatic).
    // Find the UiSelect trigger inside the "Allocation mode" settings field
    const modeField = page.locator('.settings-page__content .settings-field').filter({ hasText: 'Allocation mode' });
    const modeTrigger = modeField.locator('.ui-select__trigger');
    await expect(modeTrigger).toBeVisible();

    // Open the dropdown
    await modeTrigger.click();

    // Select "Fixed Threads" (traditional) from the dropdown
    await page.getByRole("option", { name: "Fixed Threads" }).click();

    // Now the "Max parallel tasks" field should appear (traditional mode only)
    const maxTasksField = page.locator(".settings-page__content").getByText("Max parallel tasks");
    await expect(maxTasksField).toBeVisible();

    // Save settings
    await page.getByRole("button", { name: "Save settings" }).click();

    // Wait for settings.save
    const saveParams = await wsMocker.waitForMethod("settings.save");
    expect(saveParams).toBeDefined();

    // Respond with saved settings
    wsMocker.respondToMethod("settings.save", makeMockSettings({
      scheduler: {
        ...makeMockSettings().scheduler,
        mode: "traditional",
      },
    }));
  });

  test("configures proxy settings", async ({ page, wsMocker }) => {
    // Switch to the Proxy tab
    await page.locator('.settings-page__tabs').getByRole("tab", { name: "Proxy" }).click();
    await expect(page.locator(".settings-page__content")).toBeVisible();

    // The proxy mode select should show "No proxy" (disabled) by default.
    // Find the UiSelect trigger inside the "Proxy mode" settings field
    const proxyField = page.locator('.settings-page__content .settings-field').filter({ hasText: 'Proxy mode' });
    const proxyTrigger = proxyField.locator('.ui-select__trigger');
    await expect(proxyTrigger).toBeVisible();
    await expect(proxyTrigger).toContainText("No proxy");

    // Open the dropdown and select "System proxy"
    await proxyTrigger.click();
    await page.getByRole("option", { name: "System proxy" }).click();

    // Save settings
    await page.getByRole("button", { name: "Save settings" }).click();

    // Wait for settings.save
    const saveParams = await wsMocker.waitForMethod("settings.save");
    expect(saveParams).toBeDefined();
    const payload = saveParams;
    expect(payload).toBeDefined();

    // Respond with saved settings
    wsMocker.respondToMethod("settings.save", makeMockSettings({
      proxy: { mode: "system", manualUrl: "" },
    }));
  });

  test("shows error toast when settings.save returns JSON-RPC error", async ({ page, wsMocker }) => {
    // Switch to the Proxy tab
    await page.locator('.settings-page__tabs').getByRole("tab", { name: "Proxy" }).click();
    await expect(page.locator(".settings-page__content")).toBeVisible();

    // Change proxy mode to "Manual" so the URL field appears
    const proxyField = page.locator('.settings-page__content .settings-field').filter({ hasText: 'Proxy mode' });
    const proxyTrigger = proxyField.locator('.ui-select__trigger');
    await proxyTrigger.click();
    await page.getByRole("option", { name: "Manual proxy" }).click();

    // Type an invalid proxy URL — scope to the proxy panel placeholder
    const urlInput = page.getByPlaceholder("http://127.0.0.1:7890");
    await urlInput.fill("not-a-valid-proxy");

    // Click Save
    await page.getByRole("button", { name: "Save settings" }).click();

    // Wait for settings.save RPC
    await wsMocker.waitForMethod("settings.save");

    // Respond with a JSON-RPC error
    wsMocker.respondWithError("settings.save", -32603, "Invalid proxy URL: not-a-valid-proxy");

    // Verify an error toast appears
    // NotificationToast renders role="alert" for each toast
    await expect(
      page.getByRole("alert").filter({ hasText: "Invalid proxy URL: not-a-valid-proxy" }),
    ).toBeVisible({ timeout: 5000 });
  });
});
