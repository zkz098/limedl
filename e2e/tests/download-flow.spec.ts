/**
 * Download flow E2E tests
 *
 * These tests verify the complete user flow through the Tauri flareget UI:
 * navigation between views, opening the download composer, URL input,
 * and form validation.
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

    // Navigate back to home via the "Home" sidebar button
    await page.getByRole("button", { name: "Home" }).click();

    // Verify the download queue appears on the home view.
    // The queue heading uses t("queue.title") → "Task List".
    // The sidebar's Settings/Labs buttons use ModalOverlay, so after navigating
    // back, the overlay should close and the home view content should render.
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
