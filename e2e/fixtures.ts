import { test as base } from "@playwright/test";
import { WsMocker } from "./helpers/ws-mocker";

type MyFixtures = {
  wsMocker: WsMocker;
};

/**
 * Extended test fixture with WebSocket mocking support for NAS WebUI tests.
 *
 * Usage:
 * ```ts
 * import { test, expect } from "../fixtures";
 *
 * test("mocked download start", async ({ page, wsMocker }) => {
 *   await wsMocker.install(page);
 *   await page.goto("/");
 *   // ...
 * });
 * ```
 */
export const test = base.extend<MyFixtures>({
  wsMocker: async ({ page }, use) => {
    const mocker = new WsMocker();
    await mocker.install(page);
    await use(mocker);
  },
});

export { expect } from "@playwright/test";
