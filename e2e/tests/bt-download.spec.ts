/**
 * BitTorrent download E2E tests for NAS WebUI.
 *
 * These tests use ws-mocker to simulate the BT RPC backend (no real torrent
 * network required). They verify:
 *   - BT task creation via magnet link
 *   - Torrent file picker modal interaction
 *   - Peer / tracker / piece data display
 *   - Speed limit settings via context menu
 *   - BT runtime status display
 *
 * IMPORTANT: These are mock-level tests. Real BT download requires a
 * running torrent engine with actual peer connections.
 */

import { test, expect } from "../fixtures";
import { expectTaskVisible, expectTaskState } from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("BitTorrent download", () => {
  const TASK_ID = "test-bt-001";

  test("creates a BT download via magnet link and shows file picker", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // Open composer
    await page.getByRole("button", { name: "Add Task" }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();

    // Fill with a magnet link — the composer auto-detects it as BT kind
    const magnetLink =
      "magnet:?xt=urn:btih:08ada5a312ca1c2950cbb27f4f5b1e0e8d5a7c9b&dn=test-torrent";
    await dialog.getByPlaceholder("Paste a link or choose a torrent file").fill(magnetLink);

    // Set up interception for download.start
    const startPromise = wsMocker.waitForMethod("download.start");

    // Click start
    await dialog.getByRole("button", { name: "Start download" }).click();

    const startParams = await startPromise;
    expect(startParams).toBeDefined();
    expect(startParams).toHaveProperty("url", magnetLink);

    // Respond with a taskId
    wsMocker.respondToMethod("download.start", { taskId: { kind: "bt", id: TASK_ID } });

    // The frontend now sends bt.previewTorrent to get file listing
    const previewPromise = wsMocker.waitForMethod("bt.previewTorrent");
    wsMocker.respondToMethod("bt.previewTorrent", [
      { index: 0, path: "test-torrent/file1.mkv", size: 734_003_200 },
      { index: 1, path: "test-torrent/file2.mkv", size: 524_288_000 },
      { index: 2, path: "test-torrent/file3.mkv", size: 262_144_000 },
    ]);

    await previewPromise;

    // The file picker modal should appear with torrent files
    const filePicker = page.locator("[data-testid='bt-file-picker-body']");
    await expect(filePicker).toBeVisible({ timeout: 5000 });
    await expect(filePicker.locator("[data-testid='bt-file-list'] li")).toHaveCount(3);

    // Confirm download (Select All is default, all files are selected)
    await filePicker.getByRole("button", { name: "Start Download" }).click();

    // After confirm, the frontend calls download.status
    await wsMocker.waitForMethod("download.status");
    wsMocker.respondToMethod("download.status", makeMockSummary(TASK_ID, { kind: "bt" }));

    // Task row should appear
    await expectTaskVisible(page, TASK_ID);
  });

  test("displays BT peer and tracker data in the inspector", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    await seedDownloadTask(page, wsMocker, TASK_ID, { kind: "bt" });
    await expectTaskVisible(page, TASK_ID);

    // Send progress to set downloading state
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 5_000_000,
      speedBytesPerSecond: 2_000_000,
      peerCount: 3,
    }));

    await expectTaskState(page, TASK_ID, "downloading");

    // Click the row to select it and open the detail panel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // Register console listener early to catch any errors during data loading
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    // The BT inspector calls getBtPeers and getBtTrackers when task is BT kind
    const peersPromise = wsMocker.waitForMethod("bt.getPeers");
    wsMocker.respondToMethod("bt.getPeers", [
      {
        address: "203.0.113.42:6881",
        client: "qBittorrent 4.6",
        flags: "DU",
        downloadSpeed: 500000,
        uploadSpeed: 20000,
        progress: 0.65,
      },
      {
        address: "198.51.100.7:51413",
        client: "Transmission 3.0",
        flags: "d",
        downloadSpeed: 120000,
        uploadSpeed: 5000,
        progress: 0.30,
      },
      {
        address: "192.0.2.15:6889",
        client: "Deluge 2.1",
        flags: "U",
        downloadSpeed: 0,
        uploadSpeed: 15000,
        progress: 0.95,
      },
    ]);
    await peersPromise;

    const trackersPromise = wsMocker.waitForMethod("bt.getTrackers");
    wsMocker.respondToMethod("bt.getTrackers", [
      { url: "udp://tracker.opentrackr.org:1337/announce" },
      { url: "https://tracker.nanoha.org:443/announce" },
    ]);
    await trackersPromise;

    // Wait for Vue reactivity to settle after data loading
    await expect(page.locator(".detail-panel__body")).toBeVisible({ timeout: 5000 });

    // Verify no console errors occurred during data loading
    expect(errors).toHaveLength(0);
  });

  test("sets BT speed limit via context menu", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    await seedDownloadTask(page, wsMocker, TASK_ID, { kind: "bt" });
    await expectTaskVisible(page, TASK_ID);

    // Send progress to show the task as downloading
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_000_000,
      speedBytesPerSecond: 1_000_000,
      peerCount: 5,
    }));

    await expectTaskState(page, TASK_ID, "downloading");

    // Open the context menu by right-clicking on the task row
    const taskRow = page.locator(`[data-testid="download-row-${TASK_ID}"]`);
    await taskRow.click({ button: "right" });

    // The context menu should appear with "Set Speed Limit" option (BT tasks only)
    // The speed limit button text is from t("queue.setSpeedLimit") = "Set Speed Limit..."
    const speedLimitBtn = page.getByRole("button", { name: "Set Speed Limit..." });
    await expect(speedLimitBtn).toBeVisible({ timeout: 3000 });

    // Set up interception for bt.setSpeedLimit
    const speedLimitPromise = wsMocker.waitForMethod("bt.setSpeedLimit");

    // Click the speed limit button
    await speedLimitBtn.click();

    const speedLimitParams = await speedLimitPromise;
    expect(speedLimitParams).toHaveProperty("taskId", TASK_ID);
    expect(speedLimitParams).toHaveProperty("downloadLimitBps");
    expect(speedLimitParams).toHaveProperty("uploadLimitBps");

    // Respond with success
    wsMocker.respondToMethod("bt.setSpeedLimit", null);
  });

  test("handles BT runtime status updates via toolbar", async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();

    // The app sends bt.runtimeStatus on page load
    const runtimePromise = wsMocker.waitForMethod("bt.runtimeStatus");
    wsMocker.respondToMethod("bt.runtimeStatus", {
      connected: true,
      dhtEnabled: true,
      dhtNodes: 28,
      torrentCount: 3,
      peerCount: 15,
      uploadSpeedBytesPerSecond: 512000,
      uploadedBytes: 10_000_000,
      updatedAtMs: Date.now(),
    });
    await runtimePromise;

    // The BT status should appear in the toolbar
    await expect(page.locator("[data-testid='toolbar-bt-status']")).toBeVisible({ timeout: 5000 });

    // Should show DHT nodes count (28)
    const dhtPill = page.locator("[data-testid='toolbar-bt-dht-count']");
    await expect(dhtPill).toBeVisible();
    await expect(dhtPill).toContainText("28");

    // Verify upload speed is shown
    const uploadPill = page.locator("[data-testid='toolbar-bt-upload-speed']");
    await expect(uploadPill).toBeVisible();
  });
});
