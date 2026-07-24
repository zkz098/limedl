// WebSocket-based invoke function mirroring @tauri-apps/api/core's invoke<T>()
// Uses JSON-RPC 2.0 protocol over a single shared WebSocket connection.
// Includes automatic reconnection with exponential backoff.
//
// Command names and parameter transforms are auto-generated from the Rust
// source of truth (crates/limedl-core/src/ws_manifest.rs). Do NOT manually
// edit METHOD_MAP or transformParams here — regenerate instead:
//   cargo test --features ts export_typescript_bindings

import { ref } from "vue";
import { METHOD_MAP, WS_COMMANDS } from "./generated/ws-commands";
import type { WsCommandSpec } from "./generated/ws-commands";
import { EVENT_TYPE_MAP } from "./generated/ws-events";

export function isNonNullObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Apply parameter transformation according to the manifest spec.
 *
 * Replaces the previous hardcoded 12-rule switch statement with a generic
 * handler driven by the transform kind declared in `WS_COMMANDS`.
 */
export function applyTransform(
  spec: WsCommandSpec | undefined,
  args?: Record<string, unknown>,
): Record<string, unknown> {
  if (!args) return {};
  if (!spec) return args;

  switch (spec.paramTransform.kind) {
    case "identity":
      return args;

    case "rename": {
      // Rename preserves all fields from `args` (not just `to`), unlike the
      // old per-command whitelist that only kept known keys. Safe because
      // typed wrappers don't pass extra fields and serde ignores unknowns.
      const { from, to } = spec.paramTransform;
      if (from in args) {
        const result = { ...args };
        result[to] = result[from];
        delete result[from];
        return result;
      }
      return args;
    }

    case "unwrapField": {
      const { field } = spec.paramTransform;
      const value = args[field];
      if (isNonNullObject(value)) {
        return value;
      }
      return args;
    }

    default:
      return args;
  }
}

/**
 * Connection status for the WebSocket link.
 * - 'disconnected':  initial state, or after manual disconnect
 * - 'connecting':    connection attempt in progress
 * - 'connected':     open and ready
 * - 'reconnecting':  waiting for backoff timer before next attempt
 */
export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "reconnecting";
export const connectionStatus = ref<ConnectionStatus>("disconnected");

type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  /** Full JSON-RPC message stored so it can be re-sent after reconnect */
  message: Record<string, unknown>;
};

let ws: WebSocket | null = null;
let requestId = 0;
const pending = new Map<number, PendingRequest>();
let connectPromise: Promise<WebSocket> | null = null;

const WS_URL =
  (import.meta !== undefined && import.meta.env?.VITE_WS_URL) || "ws://localhost:9090/ws";

// ── Reconnect state ──
const INITIAL_RECONNECT_DELAY = 1000;
const MAX_RECONNECT_DELAY = 30000;
let reconnectAttempt = 0;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let manualDisconnect = false;

// Re-exported by ws-event.ts
let eventDispatcher: ((eventName: string, payload: unknown) => void) | null = null;
export function setEventDispatcher(fn: (eventName: string, payload: unknown) => void) {
  eventDispatcher = fn;
}

/** Reset all reconnect state (call after a successful connection). */
function resetReconnectState() {
  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  reconnectAttempt = 0;
}

function clearReconnectTimer() {
  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
}

function scheduleReconnect() {
  if (manualDisconnect) return;
  clearReconnectTimer();

  const delay = Math.min(
    INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttempt),
    MAX_RECONNECT_DELAY,
  );
  reconnectAttempt++;
  connectionStatus.value = "reconnecting";

  // During backoff, prevent callers from creating conflicting connections.
  // This never-resolving promise acts as a gate — invoke() throws in
  // 'reconnecting' state before ever awaiting it, so it is never leaked.
  // When the timer fires, connectPromise is released and getWs() creates a
  // fresh connection (and a new connectPromise) internally.
  connectPromise = new Promise<WebSocket>(() => {});

  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectionStatus.value = "connecting";

    // Release the gate so getWs() creates a fresh connection
    connectPromise = null;

    // Establish a new connectPromise (managed internally by getWs()).
    // Suppress unhandled rejection — on connection failure the 'close' event
    // handler will call scheduleReconnect() for the next retry.
    getWs().catch(() => {});
  }, delay);
}

/** Re-send all pending RPC requests through the new socket. */
function retryPendingRequests(socket: WebSocket) {
  for (const [id, req] of pending) {
    try {
      socket.send(JSON.stringify(req.message));
    } catch {
      // Socket is not actually usable — reject this request immediately
      clearTimeout(req.timer);
      pending.delete(id);
      req.reject(new Error("Failed to re-send request after reconnect"));
    }
  }
}

/**
 * Tear down the WebSocket and prevent any further reconnection attempts.
 * Call when the application is shutting down or switching away from NAS mode.
 */
export function disconnect() {
  manualDisconnect = true;
  clearReconnectTimer();
  // Note: If a connectPromise is in-flight, its callers may hang because we
  // can't reject a foreign Promise. In practice, disconnect() is called on app
  // shutdown, so this is low-risk. A future refactor should track
  // {promise, reject} instead of just the promise.
  connectPromise = null;
  if (ws) {
    ws.close();
    ws = null;
  }
  // Reject all pending requests
  for (const [id, req] of pending) {
    clearTimeout(req.timer);
    pending.delete(id);
    req.reject(new Error("Disconnected"));
  }
  connectionStatus.value = "disconnected";
}

function getWs(): Promise<WebSocket> {
  if (ws?.readyState === WebSocket.OPEN) {
    return Promise.resolve(ws);
  }
  if (connectPromise) {
    return connectPromise;
  }
  connectPromise = new Promise<WebSocket>((resolve, reject) => {
    try {
      connectionStatus.value = "connecting";
      const socket = new WebSocket(WS_URL);
      socket.addEventListener("open", () => {
        ws = socket;
        connectPromise = null;
        // Reset reconnect state on success
        resetReconnectState();
        connectionStatus.value = "connected";
        // Retry any requests that were pending while disconnected
        retryPendingRequests(socket);
        resolve(socket);
      });
      socket.addEventListener("error", () => {
        connectPromise = null;
        reject(new Error(`WebSocket connection to ${WS_URL} failed`));
      });
      socket.addEventListener("close", () => {
        ws = null;
        connectPromise = null;
        // Automatically reconnect unless manually disconnected
        if (!manualDisconnect) {
          scheduleReconnect();
        }
      });
      socket.addEventListener("message", (event) => {
        try {
          const rawData = event.data;
          if (typeof rawData !== "string") return;
          const data = JSON.parse(rawData);
          // Check if it's a JSON-RPC response (has id)
          if (data.jsonrpc === "2.0" && typeof data.id === "number") {
            const id = data.id;
            const req = pending.get(id);
            if (req) {
              clearTimeout(req.timer);
              pending.delete(id);
              if (data.error) {
                req.reject(new Error(data.error.message || "RPC error"));
              } else {
                req.resolve(data.result);
              }
            }
          }
          // Check if it's a server-pushed event (has method but no id)
          else if (data.jsonrpc === "2.0" && data.method && data.id === undefined) {
            if (data.method === "event" && data.params && eventDispatcher) {
              const params = data.params;
              const eventType = params.type;
              const payload = params.payload;
              // Map DownloadEvent types to Tauri event names
              const eventName = mapEventType(eventType, payload);
              if (eventName) {
                eventDispatcher(eventName, payload);
              }
            }
          }
        } catch {
          // Ignore parse errors on individual messages
        }
      });
    } catch (e) {
      connectPromise = null;
      reject(e instanceof Error ? e : new Error(String(e)));
    }
  });
  return connectPromise;
}

export function mapEventType(type: string, _payload: unknown): string | null {
  return EVENT_TYPE_MAP[type] ?? null;
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Fail fast when we know we're in a reconnection backoff cycle
  if (connectionStatus.value === "reconnecting") {
    throw new Error(`Server connection lost. Cannot send "${cmd}". Retrying automatically...`);
  }

  const socket = await getWs();
  const id = ++requestId;

  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Request "${cmd}" timed out after 30s`));
    }, 30000);

    const method = METHOD_MAP[cmd] || cmd;
    const spec = WS_COMMANDS.find((c) => c.tauriName === cmd);
    const params = applyTransform(spec, args);

    const message: Record<string, unknown> = {
      jsonrpc: "2.0",
      id,
      method,
      params,
    };

    // Store the full message so it can be retried after reconnect
    pending.set(id, {
      // oxlint-disable-next-line no-unsafe-type-assertion
      resolve: resolve as (v: unknown) => void,
      reject,
      timer,
      message,
    });

    try {
      socket.send(JSON.stringify(message));
    } catch (e) {
      clearTimeout(timer);
      pending.delete(id);
      reject(e instanceof Error ? e : new Error(String(e)));
    }
  });
}
