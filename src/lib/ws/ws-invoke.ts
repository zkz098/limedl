// WebSocket-based invoke function mirroring @tauri-apps/api/core's invoke<T>()
// Uses JSON-RPC 2.0 protocol over a single shared WebSocket connection.

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

function transformParams(cmd: string, args?: Record<string, unknown>): Record<string, unknown> {
  if (!args) return {};

  switch (cmd) {
    // download_start: unwrap { request: StartDownloadRequest } → StartDownloadRequest
    case 'download_start':
      return (args.request as Record<string, unknown>) || args;

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
      return (args.settings as Record<string, unknown>) || args;

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

type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

let ws: WebSocket | null = null;
let requestId = 0;
const pending = new Map<number, PendingRequest>();
let connectPromise: Promise<WebSocket> | null = null;

const WS_URL =
  (typeof import.meta !== 'undefined' && import.meta.env?.VITE_WS_URL) ||
  'ws://localhost:9090/ws';

// Re-exported by ws-event.ts
let eventDispatcher: ((eventName: string, payload: unknown) => void) | null = null;
export function setEventDispatcher(fn: (eventName: string, payload: unknown) => void) {
  eventDispatcher = fn;
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
      const socket = new WebSocket(WS_URL);
      socket.onopen = () => {
        ws = socket;
        connectPromise = null;
        resolve(socket);
      };
      socket.onerror = () => {
        connectPromise = null;
        reject(new Error(`WebSocket connection to ${WS_URL} failed`));
      };
      socket.onclose = () => {
        ws = null;
      };
      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data as string);
          // Check if it's a JSON-RPC response (has id)
          if (data.jsonrpc === '2.0' && data.id !== undefined && data.id !== null) {
            const id = data.id as number;
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
      };
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
    default:
      return null;
  }
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const socket = await getWs();
  const id = ++requestId;

  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Request "${cmd}" timed out after 30s`));
    }, 30000);

    pending.set(id, {
      resolve: resolve as (v: unknown) => void,
      reject,
      timer,
    });

    const method = METHOD_MAP[cmd] || cmd;
    const params = transformParams(cmd, args);

    const message: Record<string, unknown> = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    socket.send(JSON.stringify(message));
  });
}
