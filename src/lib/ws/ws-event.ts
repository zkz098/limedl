// WebSocket-based event listener, mirroring @tauri-apps/api/event's listen<T>()
// Uses the shared WebSocket connection from ws-invoke.ts

import { setEventDispatcher } from "./ws-invoke";

export type UnlistenFn = () => void;

type EventCallback = (event: { payload: unknown }) => void;

const listeners = new Map<string, Set<EventCallback>>();

// Set up the event dispatcher that the ws-invoke module calls
setEventDispatcher((eventName: string, payload: unknown) => {
  const handlers = listeners.get(eventName);
  if (handlers) {
    handlers.forEach((handler) => {
      try {
        handler({ payload });
      } catch {
        // Ignore handler errors
      }
    });
  }
});

// oxlint-disable-next-line no-unnecessary-type-parameters
export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  // oxlint-disable-next-line no-unsafe-type-assertion
  const wrapped: EventCallback = (e) => handler(e as { payload: T });

  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)!.add(wrapped);

  return () => {
    const set = listeners.get(event);
    if (set) {
      set.delete(wrapped);
      if (set.size === 0) {
        listeners.delete(event);
      }
    }
  };
}
