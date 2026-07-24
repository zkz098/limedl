/**
 * WebSocket mock for NAS WebUI E2E tests.
 *
 * Wraps Playwright's `page.routeWebSocket()` to intercept WebSocket
 * connections and simulate JSON-RPC 2.0 server responses/events.
 *
 * The limedl frontend (NAS mode) connects via WebSocket at /ws and
 * uses JSON-RPC 2.0 for all communication:
 *   - Requests:   { jsonrpc: "2.0", id: number, method: string, params?: any }
 *   - Responses:  { jsonrpc: "2.0", id: number, result: any }
 *   - Errors:     { jsonrpc: "2.0", id: number, error: { code: number, message: string } }
 *   - Events:     { jsonrpc: "2.0", method: "event", params: { type: string, payload: any } }
 */

import type { Page } from "@playwright/test";
import type { DownloadProgress, DownloadSummary } from "../../src/types/download";

// `routeWebSocket` and `WebSocketRoute` are available at runtime in
// Playwright 1.48+ but may not be exposed in test TypeScript types for
// this version. We use a minimal local interface for type-safety and
// cast at the call site.

interface MockWebSocketRoute {
  onMessage(handler: (message: string | Uint8Array) => void): void;
  send(message: string | Uint8Array): void;
  close(): void;
}

// ---------------------------------------------------------------------------
// Event payload types matching the app's backend events
// ---------------------------------------------------------------------------

/** Lightweight progress payload sent every ~300ms during active downloads. Matches DownloadProgress. */
export type ProgressEventPayload = DownloadProgress;

/** Full download summary sent on state changes. Matches DownloadSummary. */
export type UpdatedEventPayload = DownloadSummary;

export type WsEventPayload = ProgressEventPayload | UpdatedEventPayload | Record<string, unknown>;

export interface WsEvent {
  type: string;
  payload: WsEventPayload;
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

type MessageHandler = (params: unknown) => void;

// ---------------------------------------------------------------------------
// WsMocker
// ---------------------------------------------------------------------------

export class WsMocker {
  private route: MockWebSocketRoute | null = null;
  private readonly callbacks = new Map<string, Set<MessageHandler>>();
  /** Stored params per method (for getMethodCalls / waitForMethod backward compat) */
  private readonly calls = new Map<string, unknown[]>();
  /** Tracks request IDs per method so respondToMethod can match the correct ID */
  private readonly callIds = new Map<string, number[]>();
  /** Auto-responses: method → result — automatically send when request arrives */
  private readonly autoResponses = new Map<string, unknown>();
  private connected = false;

  /**
   * Intercept WebSocket connections matching `urlPattern`.
   * Must be called before the page navigates/connects.
   */
  async install(page: Page, urlPattern: string | RegExp = "**/ws"): Promise<void> {
    // routeWebSocket is available at runtime in Playwright 1.48+.
    // Cast through `any` because test types may not include it.
    await (page as any).routeWebSocket(urlPattern, (route: any) => {
      this.route = route as MockWebSocketRoute;

      route.onMessage((message: string | Uint8Array) => {
        try {
          const parsed = JSON.parse(String(message));

          // Only track JSON-RPC requests (non-empty id, has method)
          if (parsed.method && parsed.id !== undefined && parsed.id !== null) {
            const method = parsed.method as string;
            const id = parsed.id as number;

            // Check for auto-response first
            if (this.autoResponses.has(method)) {
              this.sendResponse(id, this.autoResponses.get(method));
              // Still track the call for tests that need it
            }

            // Store for getMethodCalls
            if (!this.calls.has(method)) {
              this.calls.set(method, []);
            }
            this.calls.get(method)!.push(parsed.params ?? {});

            // Track ID for respondToMethod
            if (!this.callIds.has(method)) {
              this.callIds.set(method, []);
            }
            this.callIds.get(method)!.push(id);

            // Notify waiters
            const handlers = this.callbacks.get(method);
            if (handlers) {
              handlers.forEach((h) => h(parsed.params ?? {}));
            }
          }
        } catch {
          // Ignore malformed messages
        }
      });

      this.connected = true;
    });
  }

  /**
   * Send a JSON-RPC event notification to the client.
   * Wire format: { jsonrpc: "2.0", method: "event", params: { type, payload } }
   */
  sendEvent(type: string, payload: WsEventPayload): void {
    this.sendRaw({
      jsonrpc: "2.0",
      method: "event",
      params: { type, payload },
    });
  }

  /**
   * Respond to the most recent JSON-RPC request with the given method name.
   *
   * Looks up the request ID from the last captured call for `method` and
   * sends a success response with that ID. This ensures the frontend can
   * match the response to its pending request.
   *
   * Returns `true` if a matching request was found, `false` otherwise.
   */
  respondToMethod(method: string, result: unknown): boolean {
    const ids = this.callIds.get(method);
    if (!ids || ids.length === 0) {
      return false;
    }
    const id = ids.shift()!;
    this.sendResponse(id, result);
    return true;
  }

  /**
   * Send a JSON-RPC success response.
   */
  private sendResponse(id: number, result: unknown): void {
    this.sendRaw({
      jsonrpc: "2.0",
      id,
      result,
    });
  }

  /**
   * Respond to the most recent JSON-RPC request with the given method name
   * with a JSON-RPC error response.
   *
   * Looks up the request ID from the last captured call for `method` and
   * sends an error response with that ID.
   *
   * Returns `true` if a matching request was found, `false` otherwise.
   */
  respondWithError(method: string, code: number, message: string): boolean {
    const ids = this.callIds.get(method);
    if (!ids || ids.length === 0) {
      return false;
    }
    const id = ids.shift()!;
    this.sendError(id, code, message);
    return true;
  }

  /**
   * Send a JSON-RPC error response.
   */
  public sendError(id: number, code: number, message: string): void {
    this.sendRaw({
      jsonrpc: "2.0",
      id,
      error: { code, message },
    });
  }

  /**
   * Returns a Promise that resolves with the params of the next
   * incoming JSON-RPC request with the given method name.
   * If requests have already been received for this method,
   * resolves immediately with the first one.
   */
  async waitForMethod(method: string): Promise<Record<string, unknown>> {
    return new Promise((resolve) => {
      // Check if we already have calls for this method
      const existing = this.calls.get(method);
      if (existing && existing.length > 0) {
        resolve(existing.shift()! as Record<string, unknown>);
        return;
      }

      // Register handler
      if (!this.callbacks.has(method)) {
        this.callbacks.set(method, new Set());
      }
      const handler: MessageHandler = (params) => {
        this.callbacks.get(method)!.delete(handler);
        resolve(params as Record<string, unknown>);
      };
      this.callbacks.get(method)!.add(handler);

      // Re-check: message may have arrived between the first check and handler registration
      const recheck = this.calls.get(method);
      if (recheck && recheck.length > 0) {
        this.callbacks.get(method)!.delete(handler);
        resolve(recheck.shift()! as Record<string, unknown>);
      }
    });
  }

  /**
   * Returns all params received for a given method.
   */
  getMethodCalls(method: string): unknown[] {
    return this.calls.get(method) ?? [];
  }

  /**
   * Close the current WebSocket connection from the server side.
   *
   * The frontend will detect the close event and attempt to reconnect
   * (with exponential backoff starting at 1s). On reconnection,
   * the `routeWebSocket` handler fires again with a new route,
   * and the mocker continues to work transparently.
   *
   * Safe to call even if no route is installed (no-op).
   */
  disconnect(): void {
    if (this.route) {
      this.route.close();
      this.connected = false;
    }
  }

  /** True after `install()` has intercepted a WebSocket connection. */
  get isConnected(): boolean {
    return this.connected;
  }

  /**
   * Register an auto-response for a given RPC method.
   * When a JSON-RPC request with this method arrives, the mocker will
   * automatically respond with the provided result (using the correct
   * request ID). This is useful for initialization RPCs that every test
   * needs.
   */
  setAutoResponse(method: string, result: unknown): void {
    this.autoResponses.set(method, result);
  }

  // -----------------------------------------------------------------------
  // Private
  // -----------------------------------------------------------------------

  private routeOrThrow(): MockWebSocketRoute {
    if (!this.route) {
      throw new Error(
        "WsMocker not installed. Call `await mocker.install(page)` before sending messages.",
      );
    }
    return this.route;
  }

  private sendRaw(message: Record<string, unknown>): void {
    this.routeOrThrow().send(JSON.stringify(message));
  }
}
