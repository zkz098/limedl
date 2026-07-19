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
  const locator = page.locator(
    `[data-testid="download-row-${taskId}"] [data-testid="task-progress-bar"]`,
  );
  await expect(locator).toBeVisible();

  // Read the style attribute and extract the width percentage
  const styleAttr = await locator.getAttribute("style");
  expect(styleAttr).not.toBeNull();

  const widthMatch = styleAttr!.match(/width:\s*([\d.]+)%/);
  expect(widthMatch).not.toBeNull();
  expect(Number(widthMatch![1])).toBeGreaterThanOrEqual(minValue);
}

/**
 * Asserts that the task's speed display shows a non-zero value with a unit suffix
 * (e.g. "1.2 MB/s", "500 KB/s").
 */
export async function expectSpeedDisplay(page: Page, taskId: string): Promise<void> {
  const locator = page.locator(
    `[data-testid="download-row-${taskId}"] [data-testid="task-speed"]`,
  );
  await expect(locator).toBeVisible();

  const text = await locator.textContent();
  expect(text).toBeTruthy();

  // Should contain a number followed by a unit
  const speedPattern = /(\d+(?:\.\d+)?)\s*(B\/s|KB\/s|MB\/s|GB\/s)/i;
  expect(text).toMatch(speedPattern);

  // Extract the numeric value and ensure it's > 0
  const match = text!.match(speedPattern);
  if (match) {
    const value = parseFloat(match[1]);
    expect(value).toBeGreaterThan(0);
  }
}
