// WebSocket-based invoke function mirroring @tauri-apps/api/core's invoke<T>()
// Uses JSON-RPC 2.0 protocol over a single shared WebSocket connection.
// Includes automatic reconnection with exponential backoff.

import { ref } from "vue";

// Tauri command name → JSON-RPC method name
const METHOD_MAP: Record<string, string> = {
  download_start: 'download.start',
  download_pause: 'download.pause',
  download_resume: 'download.resume',
  download_cancel: 'download.cancel',
  download_remove: 'download.remove',
  download_purge: 'download.purge',
  download_status: 'download.status',
  download_list: 'download.list',
  download_open_in_explorer: 'download.openInExplorer',
  settings_get: 'settings.get',
  settings_save: 'settings.save',
  bt_runtime_status: 'bt.runtimeStatus',
  bt_set_speed_limit: 'bt.setSpeedLimit',
  bt_preview_torrent: 'bt.previewTorrent',
  bt_get_peers: 'bt.getPeers',
  bt_get_trackers: 'bt.getTrackers',
  bt_get_pieces: 'bt.getPieces',
  get_bt_files: 'bt.getFiles',
  update_bt_files: 'bt.updateFiles',
  cdn_fetch_ranges: 'cdn.fetchRanges',
  cdn_test: 'cdn.test',
  cdn_apply: 'cdn.apply',
  cdn_clear: 'cdn.clear',
  cdn_status: 'cdn.status',
  cdn_cancel: 'cdn.cancel',
  cdn_detail: 'cdn.detail',
  cdn_candidates: 'cdn.candidates',
  toggle_game_mode: 'settings.toggleGameMode',
  get_io_status: 'settings.getIoStatus',
  toggle_overclock_mode: 'settings.toggleOverclockMode',
  get_overclock_mode: 'settings.getOverclockMode',
  settings_fetch_tracker_list: 'settings.fetchTrackerList',
};

function isNonNullObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function transformParams(cmd: string, args?: Record<string, unknown>): Record<string, unknown> {
  if (!args) return {};

  switch (cmd) {
    // download_start: unwrap { request: StartDownloadRequest } → StartDownloadRequest
    case 'download_start':
      return isNonNullObject(args.request) ? args.request : args;

    // Commands that rename downloadId → taskId
    case 'download_pause':
    case 'download_resume':
    case 'download_cancel':
    case 'download_remove':
    case 'download_purge':
    case 'download_status':
    case 'download_open_in_explorer':
      return { taskId: args.downloadId };

    // settings_save: { settings: AppSettings } → AppSettings (flat)
    case 'settings_save':
      return isNonNullObject(args.settings) ? args.settings : args;

    // bt_set_speed_limit: rename params
    case 'bt_set_speed_limit':
      return {
        taskId: args.downloadId,
        downloadLimitBps: args.downloadLimitBps,
        uploadLimitBps: args.uploadLimitBps,
      };

    // bt_preview_torrent: { source } → { source }
    case 'bt_preview_torrent':
      return { source: args.source };

    // bt_get_peers/trackers/pieces/files: { downloadId } → { taskId }
    case 'bt_get_peers':
    case 'bt_get_trackers':
    case 'bt_get_pieces':
    case 'get_bt_files':
      return { taskId: args.downloadId };

    // update_bt_files: { downloadId, includedIndices } → { taskId, includedIndices }
    case 'update_bt_files':
      return { taskId: args.downloadId, includedIndices: args.includedIndices };

    // cdn_apply: { ip, speedMbps } → { ip, speedMbps }
    case 'cdn_apply':
      return { ip: args.ip, speedMbps: args.speedMbps };

    // toggle_game_mode / toggle_overclock_mode: { enabled } → { enabled }
    case 'toggle_game_mode':
    case 'toggle_overclock_mode':
      return { enabled: args.enabled };

    // settings_fetch_tracker_list: { trackerListUrl } → { trackerListUrl }
    case 'settings_fetch_tracker_list':
      return { trackerListUrl: args.trackerListUrl };

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
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';
export const connectionStatus = ref<ConnectionStatus>('disconnected');

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
  (typeof import.meta !== 'undefined' && import.meta.env?.VITE_WS_URL) ||
  'ws://localhost:9090/ws';

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
  connectionStatus.value = 'reconnecting';

  // During backoff, prevent callers from creating conflicting connections.
  // This never-resolving promise acts as a gate — invoke() throws in
  // 'reconnecting' state before ever awaiting it, so it is never leaked.
  // When the timer fires, connectPromise is released and getWs() creates a
  // fresh connection (and a new connectPromise) internally.
  connectPromise = new Promise<WebSocket>(() => {});

  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectionStatus.value = 'connecting';

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
      req.reject(new Error('Failed to re-send request after reconnect'));
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
    req.reject(new Error('Disconnected'));
  }
  connectionStatus.value = 'disconnected';
}

function getWs(): Promise<WebSocket> {
  if (ws && ws.readyState === WebSocket.OPEN) {
    return Promise.resolve(ws);
  }
  if (connectPromise) {
    return connectPromise;
  }
  connectPromise = new Promise<WebSocket>((resolve, reject) => {
    try {
      connectionStatus.value = 'connecting';
      const socket = new WebSocket(WS_URL);
      socket.addEventListener('open', () => {
        ws = socket;
        connectPromise = null;
        // Reset reconnect state on success
        resetReconnectState();
        connectionStatus.value = 'connected';
        // Retry any requests that were pending while disconnected
        retryPendingRequests(socket);
        resolve(socket);
      });
      socket.addEventListener('error', () => {
        connectPromise = null;
        reject(new Error(`WebSocket connection to ${WS_URL} failed`));
      });
      socket.addEventListener('close', () => {
        ws = null;
        connectPromise = null;
        // Automatically reconnect unless manually disconnected
        if (!manualDisconnect) {
          scheduleReconnect();
        }
      });
      socket.addEventListener('message', (event) => {
        try {
          const rawData = event.data;
          if (typeof rawData !== 'string') return;
          const data = JSON.parse(rawData);
          // Check if it's a JSON-RPC response (has id)
          if (data.jsonrpc === '2.0' && typeof data.id === 'number') {
            const id = data.id;
            const req = pending.get(id);
            if (req) {
              clearTimeout(req.timer);
              pending.delete(id);
              if (data.error) {
                req.reject(new Error(data.error.message || 'RPC error'));
              } else {
                req.resolve(data.result);
              }
            }
          }
          // Check if it's a server-pushed event (has method but no id)
          else if (data.jsonrpc === '2.0' && data.method && data.id === undefined) {
            if (data.method === 'event' && data.params && eventDispatcher) {
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

function mapEventType(type: string, _payload: unknown): string | null {
  switch (type) {
    case 'updated':
      return 'download-updated';
    case 'progress':
      return 'download-progress';
    case 'aria2Notification':
      return 'aria2-notification';
    case 'cdnProgress':
      return 'cdn-test-progress';
    case 'cdnComplete':
      return 'cdn-test-complete';
    case 'warning':
      return 'download-warning';
    default:
      return null;
  }
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Fail fast when we know we're in a reconnection backoff cycle
  if (connectionStatus.value === 'reconnecting') {
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
    const params = transformParams(cmd, args);

    const message: Record<string, unknown> = {
      jsonrpc: '2.0',
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
