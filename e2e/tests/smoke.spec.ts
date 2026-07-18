import { test, expect } from "../fixtures";

test.describe("smoke", () => {
  test("page loads and renders the app root", async ({ page }) => {
    // Navigate to the Tauri webview URL (Vite dev server)
    await page.goto("/");

    // Verify the Vue mount point exists
    await expect(page.locator("#app")).toBeAttached();

    // Verify the app component rendered (the root element in App.vue)
    await expect(page.locator(".app-root")).toBeVisible();

    // Verify the page title (set in index.html)
    await expect(page).toHaveTitle(/Flareget/i);
  });

  test("main UI elements are present on the home view", async ({ page }) => {
    await page.goto("/");

    // The TopToolbar is rendered when currentView === 'home' (default)
    // Category sidebar should be visible
    await expect(page.locator("aside.category-sidebar")).toBeAttached();

    // The download queue table area should be present
    // Check for the content area which wraps the table
    await expect(page.locator("main.content-area")).toBeVisible();
  });
});
