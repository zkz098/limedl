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
import { expectTaskVisible, expectTaskState, expectProgressValue } from "../helpers/download-asserts";
import { seedDownloadTask, makeMockSummary, makeMockProgress } from "../helpers/task-helpers";

test.describe("queue scenarios", () => {
  test.beforeEach(async ({ page, wsMocker }) => {
    await page.goto("/");
    await expect(page.locator(".app-root")).toBeVisible();
  });

  test.skip("multiple downloads show in correct order", async ({ page, wsMocker }) => {
    const TASK_IDS = ["order-001", "order-002", "order-003"];
    const summaries: Record<string, unknown>[] = [];

    // Seed 3 downloads sequentially through the composer dialog.
    // Passing previousSummaries preserves earlier tasks across seed calls.
    for (const taskId of TASK_IDS) {
      const summary = await seedDownloadTask(page, wsMocker, taskId, {
        previousSummaries: [...summaries],
      });
      summaries.push(summary);
      // Allow composer dialog transition to complete between iterations
      await page.waitForTimeout(800);
    }

    // Override created_at timestamps via updated events to ensure deterministic
    // ordering. The app sorts by createdAtMs descending by default.
    // order-003 (newest): 3000ms ago, order-002: 6000ms ago, order-001 (oldest): 9000ms ago.
    const now = Date.now();
    for (const t of [
      { id: "order-001", createdAtMs: now - 9000 },
      { id: "order-002", createdAtMs: now - 6000 },
      { id: "order-003", createdAtMs: now - 3000 },
    ]) {
      wsMocker.sendEvent("updated", makeMockSummary(t.id, { createdAtMs: t.createdAtMs }));
    }

    // Wait for Vue reactivity to re-sort the list
    await page.waitForTimeout(500);

    // Get all visible download rows in DOM order
    const rows = page.locator("[data-testid^='download-row-']");
    await expect(rows).toHaveCount(3);

    // Default sort is added_at desc — newest first
    await expect(rows.nth(0)).toHaveAttribute("data-testid", "download-row-order-003");
    await expect(rows.nth(1)).toHaveAttribute("data-testid", "download-row-order-002");
    await expect(rows.nth(2)).toHaveAttribute("data-testid", "download-row-order-001");
  });

  test.skip("filter by category shows correct tasks", async ({ page, wsMocker }) => {
    const SUMMARIES: Record<string, unknown>[] = [];
    const TASK_IDS = ["filter-dl", "filter-paused", "filter-comp"];

    // Seed three tasks
    for (const taskId of TASK_IDS) {
      const summary = await seedDownloadTask(page, wsMocker, taskId, {
        previousSummaries: [...SUMMARIES],
      });
      SUMMARIES.push(summary);
      await page.waitForTimeout(800);
    }

    // Set distinct states via updated events
    wsMocker.sendEvent("updated", makeMockSummary("filter-dl", {
      state: "downloading",
      downloadedBytes: 2_000_000,
    }));
    wsMocker.sendEvent("updated", makeMockSummary("filter-paused", {
      state: "paused",
      downloadedBytes: 5_000_000,
      connectionCount: 0,
    }));
    wsMocker.sendEvent("updated", makeMockSummary("filter-comp", {
      state: "completed",
      downloadedBytes: 10_000_000,
      connectionCount: 0,
    }));

    await page.waitForTimeout(500);

    // All tasks visible on "All" (default) category
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();

    // Click "Downloading" category — only downloading task visible
    await page.getByRole("button", { name: "Downloading", exact: true }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).not.toBeVisible();

    // Click "Paused" category
    await page.getByRole("button", { name: "Paused", exact: true }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).not.toBeVisible();

    // Click "Completed" category
    await page.getByRole("button", { name: "Completed", exact: true }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).not.toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();

    // Click "All" — all visible again
    await page.getByRole("button", { name: "All", exact: true }).click();
    await expect(page.locator("[data-testid='download-row-filter-dl']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-paused']")).toBeVisible();
    await expect(page.locator("[data-testid='download-row-filter-comp']")).toBeVisible();
  });

  test.skip("search filters downloads by name", async ({ page, wsMocker }) => {
    const SUMMARIES: Record<string, unknown>[] = [];

    // Seed three downloads with distinct file names
    await seedDownloadTask(page, wsMocker, "search-alpha", {
      fileName: "AlphaProject.zip",
      previousSummaries: [...SUMMARIES],
    });
    SUMMARIES.push(makeMockSummary("search-alpha", { fileName: "AlphaProject.zip" }));
    await page.waitForTimeout(800);

    await seedDownloadTask(page, wsMocker, "search-beta", {
      fileName: "BetaRelease.iso",
      previousSummaries: [...SUMMARIES],
    });
    SUMMARIES.push(makeMockSummary("search-beta", { fileName: "BetaRelease.iso" }));
    await page.waitForTimeout(800);

    await seedDownloadTask(page, wsMocker, "search-gamma", {
      fileName: "GammaDocument.pdf",
      previousSummaries: [...SUMMARIES],
    });
    await page.waitForTimeout(800);

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

  test.skip("sort by name toggles order", async ({ page, wsMocker }) => {
    const SUMMARIES: Record<string, unknown>[] = [];

    // Seed three tasks with names that sort alphabetically
    await seedDownloadTask(page, wsMocker, "sort-apple", {
      fileName: "apple.iso",
      previousSummaries: [...SUMMARIES],
    });
    SUMMARIES.push(makeMockSummary("sort-apple", { fileName: "apple.iso" }));
    await page.waitForTimeout(800);

    await seedDownloadTask(page, wsMocker, "sort-banana", {
      fileName: "banana.zip",
      previousSummaries: [...SUMMARIES],
    });
    SUMMARIES.push(makeMockSummary("sort-banana", { fileName: "banana.zip" }));
    await page.waitForTimeout(800);

    await seedDownloadTask(page, wsMocker, "sort-cherry", {
      fileName: "cherry.tar.gz",
      previousSummaries: [...SUMMARIES],
    });
    await page.waitForTimeout(800);

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
      wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
        downloadedBytes: Math.round((TOTAL_BYTES / 20) * i),
        speedBytesPerSecond: 1_000_000 + i * 100_000,
        etaSeconds: 20 - i,
      }));
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
    wsMocker.sendEvent("progress", makeMockProgress(TASK_ID, {
      downloadedBytes: 2_500_000,
      speedBytesPerSecond: 3_000_000,
      etaSeconds: 3,
    }));

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
    await expect(detailPanel.locator(".detail-panel__title .ui-badge")).toContainText("Downloading");
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
