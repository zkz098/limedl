/**
 * Global setup for E2E tests.
 *
 * - Creates a temp directory for test files
 * - Generates deterministic test files (1 MB, 10 MB, small text)
 * - Starts TestFileServer on port 9876
 * - Sets env vars for test access
 *
 * Stores the server instance on `globalThis.__TEST_FILE_SERVER__`
 * for access in global-teardown.ts.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { fileURLToPath } from "node:url";
import { TestFileServer } from "./server/test-file-server";

/** ESM-safe directory of this file (package.json is `type: module`). */
const scriptDir = path.dirname(fileURLToPath(import.meta.url));

/**
 * Playwright storage state marking the setup wizard as completed.
 * Generated here (not committed) so fresh CI clones can run NAS WebUI tests.
 * The `origin` must match the NAS WebUI baseURL used in playwright.config.ts.
 */
const STORAGE_STATE_PATH = path.join(scriptDir, "storage-state.json");

export default async function globalSetup(): Promise<void> {
  const testDir = fs.mkdtempSync(path.join(os.tmpdir(), "limedl-e2e-"));

  // Set env vars so tests and helpers can discover the server
  process.env.TEST_FILE_SERVER_PORT = "9876";
  process.env.TEST_FILE_SERVER_URL = `http://localhost:9876`;
  process.env.TEST_FILE_SERVER_DIR = testDir;

  // Create and start the test file server
  const server = new TestFileServer(testDir);
  await server.start(9876);

  // Seed storage state so NAS WebUI projects skip the setup wizard
  fs.writeFileSync(
    STORAGE_STATE_PATH,
    JSON.stringify(
      {
        cookies: [],
        origins: [
          {
            origin: "http://localhost:9090",
            localStorage: [{ name: "limedl.setupCompleted", value: "true" }],
          },
        ],
      },
      null,
      2,
    ),
  );

  console.log(`[global-setup] TestFileServer listening on ${server.url}`);
  console.log(`[global-setup] Test files: ${Object.keys(server.files).join(", ")}`);
  console.log(`[global-setup] Temp dir: ${testDir}`);
}
