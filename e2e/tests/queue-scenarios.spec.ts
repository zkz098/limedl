/**
 * Queue management E2E tests for NAS WebUI.
 *
 * Tests the download queue behavior: ordering, filtering, search,
 * sorting, reconnect resilience, rapid updates, detail panel, and empty state.
 *
 * All tests use ws-mocker to simulate the JSON-RPC backend without
 * a running limedl-server daemon.
 *
 * IMPORTANT: Sidebar category buttons use translated text (e.g. "All",
 * "Downloading", "Paused") from the en-US locale. The UiSelect sort
 * dropdown uses a custom button trigger with role="listbox" for options.
 * Search input uses placeholder "Search downloads...".
 */

import { test, expect } from "../fixtures";
import {
  expectTaskVisible,
  expectTaskState,
  expectProgressValue,
} from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("queue scenarios", () => {
  test.beforeEach(async ({ page, wsMocker: _wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test("multiple downloads show in correct order", async ({ page, wsMocker }) => {
    const TASK_IDS = ["order-001", "order-002", "order-003"];
    const now = Date.now();

    // Inject tasks directly instead of going through the composer dialog.
    const offsets = [9000, 6000, 3000];
    for (let i = 0; i < TASK_IDS.length; i++) {
      wsMocker.sendEvent(
        "updated",
        makeMockSummary(TASK_IDS[i], { createdAtMs: now - offsets[i] }),
      );
    }

    // Also pre-seed download.list so page reload / refresh would preserve data
    wsMocker.setAutoResponse(
      "download.list",
      TASK_IDS.toReversed().map((id, i) =>
        makeMockSummary(id, { createdAtMs: now - offsets[2 - i] }),
      ),
    );

    // Wait for Vue reactivity to render the tasks
    await page.waitForTimeout(500);

    // Get all visible download rows in DOM order
    const rows = page.locator("[data-testid^='download-row-']");
    await expect(rows).toHaveCount(3);

    // Default sort is added_at desc — newest first
    await expect(rows.nth(0)).toHaveAttribute("data-testid", "download-row-order-003");
    await expect(rows.nth(1)).toHaveAttribute("data-testid", "download-row-order-002");
    await expect(rows.nth(2)).toHaveAttribute("data-testid", "download-row-order-001");
  });

  test("filter by category shows correct tasks", async ({ page, wsMocker }) => {
    const TASK_IDS = ["filter-dl", "filter-paused", "filter-comp"];

    // Inject tasks directly — no composer dialog needed
    const taskConfigs: Record<string, Record<string, unknown>> = {
      "filter-dl": { state: "downloading", downloadedBytes: 2_000_000 },
      "filter-paused": { state: "paused", downloadedBytes: 5_000_000, connectionCount: 0 },
      "filter-comp": { state: "completed", downloadedBytes: 10_000_000, connectionCount: 0 },
    };

    for (const id of TASK_IDS) {
      wsMocker.sendEvent("updated", makeMockSummary(id, taskConfigs[id]));
    }

    // Also pre-seed download.list for consistency
    wsMocker.setAutoResponse(
      "download.list",
      TASK_IDS.map((id) => makeMockSummary(id, taskConfigs[id])),
    );

    await page.waitForTimeout(500);

    // All tasks visible on "All" (default) category
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();

    // Click "Downloading" category — only downloading task visible
    await page.getByRole("button", { name: "Downloading" }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).not.toBeVisible();

    // Click "Paused" category
    await page.getByRole("button", { name: "Paused" }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).not.toBeVisible();

    // Click "Completed" category
    await page.getByRole("button", { name: "Completed" }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();

    // Click "All" — all visible again
    await page.getByRole("button", { name: "All" }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();
  });

  test("search filters downloads by name", async ({ page, wsMocker }) => {
    // Inject three downloads with distinct file names via direct events
    wsMocker.sendEvent(
      "updated",
      makeMockSummary("search-alpha", { fileName: "AlphaProject.zip" }),
    );
    wsMocker.sendEvent("updated", makeMockSummary("search-beta", { fileName: "BetaRelease.iso" }));
    wsMocker.sendEvent(
      "updated",
      makeMockSummary("search-gamma", { fileName: "GammaDocument.pdf" }),
    );

    // Pre-seed download.list auto-response for consistency
    wsMocker.setAutoResponse("download.list", [
      makeMockSummary("search-alpha", { fileName: "AlphaProject.zip" }),
      makeMockSummary("search-beta", { fileName: "BetaRelease.iso" }),
      makeMockSummary("search-gamma", { fileName: "GammaDocument.pdf" }),
    ]);

    await page.waitForTimeout(500);

    // All visible initially
    await expect(page.locator("[data-testid='download-row-search-alpha']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-beta']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-gamma']")).toBeVisible();

    // Type "Alpha" in the search box
    const searchInput = page.getByPlaceholder("Search downloads...");
    await searchInput.fill("Alpha");

    // Only "AlphaProject.zip" should match (searches fileName and URL)
    await expect(page.locator("[data-testid='download-row-search-alpha']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-beta']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-gamma']")).not.toBeVisible();

    // Clear the search
    await searchInput.fill("");

    // All visible again
    await expect(page.locator("[data-testid='download-row-search-alpha']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-beta']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-gamma']")).toBeVisible();

    // Search for ".pdf" — should match GammaDocument.pdf
    await searchInput.fill(".pdf");
    await expect(page.locator("[data-testid='download-row-search-alpha']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-beta']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-search-gamma']")).toBeVisible();
  });

  test("sort by name toggles order", async ({ page, wsMocker }) => {
    // Inject three tasks with names that sort alphabetically
    wsMocker.sendEvent("updated", makeMockSummary("sort-apple", { fileName: "apple.iso" }));
    wsMocker.sendEvent("updated", makeMockSummary("sort-banana", { fileName: "banana.zip" }));
    wsMocker.sendEvent("updated", makeMockSummary("sort-cherry", { fileName: "cherry.tar.gz" }));

    // Pre-seed download.list for consistency
    wsMocker.setAutoResponse("download.list", [
      makeMockSummary("sort-apple", { fileName: "apple.iso" }),
      makeMockSummary("sort-banana", { fileName: "banana.zip" }),
      makeMockSummary("sort-cherry", { fileName: "cherry.tar.gz" }),
    ]);

    await page.waitForTimeout(500);

    // Default sort is "added_at desc", so newest first.
    // Change sort key to "name" via the sort dropdown.
    // The UiSelect trigger is a button in the .sort-control area.
    // The dropdown options have role="option" and are teleported to body.
    const sortSelect = page.locator(".sort-control__select").getByRole("button");
    await sortSelect.click();

    // The options panel is teleported to body — find by role and text
    const nameOption = page.getByRole("option", { name: "Name" });
    await expect(nameOption).toBeVisible();
    await nameOption.click();

    // Default direction is "desc", so names appear Z-A: cherry → banana → apple
    const rows = page.locator("[data-testid^='download-row-']");
    await expect(rows).toHaveCount(3);
    await expect(rows.nth(0)).toHaveAttribute("data-testid", "download-row-sort-cherry");
    await expect(rows.nth(1)).toHaveAttribute("data-testid", "download-row-sort-banana");
    await expect(rows.nth(2)).toHaveAttribute("data-testid", "download-row-sort-apple");

    // Click the sort direction toggle button (the arrow icon button next to the select)
    const sortDirectionBtn = page.locator(".sort-control").getByRole("button").last();
    await sortDirectionBtn.click();

    // Now direction is "asc", names appear A-Z: apple → banana → cherry
    await expect(rows.nth(0)).toHaveAttribute("data-testid", "download-row-sort-apple");
    await expect(rows.nth(1)).toHaveAttribute("data-testid", "download-row-sort-banana");
    await expect(rows.nth(2)).toHaveAttribute("data-testid", "download-row-sort-cherry");
  });

  test("WebSocket reconnect preserves download state", async ({ page, wsMocker }) => {
    const TASK_ID = "reconnect-001";

    // Seed a download task
    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Disconnect the WebSocket (simulates network interruption).
    // The frontend's auto-reconnect logic kicks in with exponential backoff
    // starting at 1s. routeWebSocket in Playwright re-intercepts on reconnect.
    wsMocker.disconnect();

    // Wait for the frontend to detect the disconnect and attempt reconnection.
    // After reconnecting, the frontend calls download.list to refresh state.
    await wsMocker.waitForMethod("download.list");

    // Respond with the same task data — the task should persist across the interruption.
    wsMocker.respondToMethod("download.list", [makeMockSummary(TASK_ID)]);

    // Verify the download row still appears in the queue
    await expectTaskVisible(page, TASK_ID);
    await expectTaskState(page, TASK_ID, "downloading");
  });

  test("rapid progress updates render smoothly", async ({ page, wsMocker }) => {
    const TASK_ID = "rapid-001";
    const TOTAL_BYTES = 10_000_000;

    // Collect console errors for post-test verification
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Fire 20 rapid progress events in sequence (no delays between them).
    // This stresses the Vue reactivity system and the ws-mocker event pipeline.
    for (let i = 1; i <= 20; i++) {
      wsMocker.sendEvent(
        "progress",
        makeMockProgress(TASK_ID, {
          downloadedBytes: Math.round((TOTAL_BYTES / 20) * i),
          speedBytesPerSecond: 1_000_000 + i * 100_000,
          etaSeconds: 20 - i,
        }),
      );
    }

    // Give Vue a brief moment to flush all pending reactivity updates
    await page.waitForTimeout(300);

    // The task should still be visible and in downloading state
    await expectTaskVisible(page, TASK_ID);
    await expectTaskState(page, TASK_ID, "downloading");

    // The progress bar should reflect one of the later update values (≈ 90–100%)
    await expectProgressValue(page, TASK_ID, 85);

    // Filter out benign errors unrelated to our rapid updates
    const relevantErrors = errors.filter(
      (e) => !e.includes("favicon") && !e.includes("ResizeObserver"),
    );
    expect(relevantErrors).toHaveLength(0);
  });

  test("download detail panel opens on row click", async ({ page, wsMocker }) => {
    const TASK_ID = "detail-001";

    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Send a progress event to show the task actively downloading
    wsMocker.sendEvent(
      "progress",
      makeMockProgress(TASK_ID, {
        downloadedBytes: 2_500_000,
        speedBytesPerSecond: 3_000_000,
        etaSeconds: 3,
      }),
    );

    await expectTaskState(page, TASK_ID, "downloading");

    // Click the download row to select it and open the DetailPanel
    await page.locator(`[data-testid="download-row-${TASK_ID}"]`).click();

    // The detail panel should be visible with the correct file name
    const detailPanel = page.locator(".detail-panel");
    await expect(detailPanel).toBeVisible();

    // The file name should appear in the detail panel header
    await expect(detailPanel.locator(".detail-panel__filename")).toContainText("10mb.bin");

    // The detail panel body should be visible (download inspector content)
    await expect(detailPanel.locator(".detail-panel__body")).toBeVisible();

    // The status badge should show "Downloading"
    await expect(detailPanel.locator(".detail-panel__title .ui-badge")).toContainText(
      "Downloading",
    );
  });

  test("empty state shows message when all downloads cleared", async ({ page, wsMocker }) => {
    const TASK_ID = "empty-001";

    // Seed a task first so we have something to clear
    await seedDownloadTask(page, wsMocker, TASK_ID);
    await expectTaskVisible(page, TASK_ID);

    // Override the download.list auto-response to return an empty list.
    // This simulates the server-side clearing of all tasks.
    wsMocker.setAutoResponse("download.list", []);

    // Click the Refresh button to trigger a new download.list call
    await page.getByRole("button", { name: "Refresh" }).first().click();

    // Wait for the download.list call to be received and responded to
    await wsMocker.waitForMethod("download.list");

    // The empty state should now be visible with the "No download tasks" message.
    // UiEmptyState renders <h3 class="ui-empty-state__title"> with the title text.
    await expect(page.locator(".ui-empty-state")).toBeVisible();
    await expect(page.locator(".ui-empty-state__title")).toHaveText("No download tasks");

    // The description text should also be shown
    await expect(page.locator(".ui-empty-state__description")).toHaveText(
      "Click New task on the left to start downloading.",
    );
  });
});
