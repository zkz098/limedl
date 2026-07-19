/**
 * Global teardown for E2E tests.
 *
 * - Retrieves the TestFileServer instance from globalThis
 * - Stops the server
 * - Cleans up the temp directory
 */

export default async function globalTeardown(): Promise<void> {
  const server = (globalThis as any).__TEST_FILE_SERVER__;

  if (server) {
    console.log("[global-teardown] Stopping TestFileServer...");
    await server.stop();

    console.log("[global-teardown] Cleaning up test files...");
    server.cleanup();

    delete (globalThis as any).__TEST_FILE_SERVER__;
    console.log("[global-teardown] Done.");
  } else {
    console.warn("[global-teardown] No TestFileServer instance found on globalThis.");
  }
}
