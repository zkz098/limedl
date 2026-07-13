/**
 * Tauri IPC mock utility for Vitest tests.
 *
 * Usage:
 * ```ts
 * import { vi } from "vitest";
 *
 * vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
 *
 * import { invoke } from "@tauri-apps/api/core";
 * import { createMockInvoke, mockTauriCommandValue, resetTauriMocks } from "../mocks/tauri-mock";
 *
 * beforeEach(() => {
 *   resetTauriMocks();
 *   vi.mocked(invoke).mockImplementation(createMockInvoke());
 * });
 * ```
 *
 * @module
 */

type CommandHandler = (args?: Record<string, unknown>) => unknown;

/** Per-worker registry of mock command handlers. */
const handlers = new Map<string, CommandHandler>();

/**
 * Register a handler for a specific Tauri command.
 * The handler receives the args object and returns the mock data.
 */
export function mockTauriCommand(command: string, handler: CommandHandler) {
  handlers.set(command, handler);
}

/**
 * Register a handler that returns a fixed value for a Tauri command.
 * This is a convenience wrapper around `mockTauriCommand`.
 */
export function mockTauriCommandValue(command: string, value: unknown) {
  handlers.set(command, () => value);
}

/**
 * Create a mock implementation for `invoke` that routes calls to registered
 * command handlers. Pass the result to `vi.mocked(invoke).mockImplementation()`.
 *
 * Returns a plain async function (not a vi.fn) to avoid type conflicts with
 * the real `InvokeArgs` type from `@tauri-apps/api/core`.
 */
export function createMockInvoke() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return async (command: string, args?: any) => {
    const handler = handlers.get(command);

    if (!handler) {
      throw new Error(
        `[tauri-mock] No mock handler registered for command: ${command}. ` +
          `Use mockTauriCommand() or mockTauriCommandValue() to register one.`,
      );
    }

    return handler(args);
  };
}

/**
 * Reset all registered command handlers and clear call history.
 * Call in `beforeEach` to ensure clean state between tests.
 */
export function resetTauriMocks() {
  handlers.clear();
}
