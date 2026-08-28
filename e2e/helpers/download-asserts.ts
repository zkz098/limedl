/**
 * Helper assertion functions for download task UI elements.
 *
 * These rely on `data-testid` attributes in the Vue components.
 */

import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";

/**
 * Asserts that a task row with the given ID is visible in the UI.
 */
export async function expectTaskVisible(page: Page, taskId: string): Promise<void> {
  const locator = page.locator(`[data-testid="download-row-${taskId}"]`);
  await expect(locator).toBeVisible();
}

/**
 * Asserts that the task status text matches the expected state.
 */
export async function expectTaskState(page: Page, taskId: string, state: string): Promise<void> {
  const locator = page.locator(
    `[data-testid="download-row-${taskId}"] [data-testid="task-status"]`,
  );
  await expect(locator).toHaveText(state, { ignoreCase: true });
}

/**
 * Asserts that the progress bar's style width is at least `minValue` percent.
 */
export async function expectProgressValue(
  page: Page,
  taskId: string,
  minValue: number,
): Promise<void> {
  const row = page.locator(`[data-testid="download-row-${taskId}"]`);
  await expect(row).toBeVisible();

  const locator = page.locator(
    `[data-testid="download-row-${taskId}"] [data-testid="task-progress-bar"]`,
  );
  await expect(locator).toBeAttached();

  await expect(async () => {
    const styleAttr = await locator.getAttribute("style");
    expect(styleAttr).not.toBeNull();
    const widthMatch = styleAttr!.match(/width:\s*([\d.]+)%/);
    expect(widthMatch).not.toBeNull();
    expect(Number(widthMatch![1])).toBeGreaterThanOrEqual(minValue);
  }).toPass({ timeout: 5000 });
}

// Speed units, longest first so "KB/s" is matched before its "B/s" suffix.
const SPEED_UNITS = ["GB/s", "MB/s", "KB/s", "B/s"];

/** Extracts the numeric speed value from a formatted speed string, e.g. "1.2 MB/s". */
function extractSpeedValue(text: string): number | null {
  const lower = text.toLowerCase();
  for (const unit of SPEED_UNITS) {
    const idx = lower.indexOf(unit.toLowerCase());
    if (idx === -1) continue;
    const value = Number.parseFloat(text.slice(0, idx).trim());
    if (Number.isFinite(value)) return value;
  }
  return null;
}

/**
 * Asserts that the task's speed display shows a non-zero value with a unit suffix
 * (e.g. "1.2 MB/s", "500 KB/s").
 */
export async function expectSpeedDisplay(page: Page, taskId: string): Promise<void> {
  const locator = page.locator(`[data-testid="download-row-${taskId}"] [data-testid="task-speed"]`);
  await expect(locator).toBeVisible();

  const text = await locator.textContent();
  expect(text).toBeTruthy();

  // Should contain a number followed by a unit; the parsed value must be > 0
  const speedValue = extractSpeedValue(text ?? "");
  expect(speedValue).not.toBeNull();
  expect(speedValue!).toBeGreaterThan(0);
}
