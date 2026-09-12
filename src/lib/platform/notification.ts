/**
 * Web Notifications API implementation.
 *
 * In a browser there is no native notification bridge, so notifications go
 * through the standard Web Notifications API. When the API is unavailable
 * (jsdom test environments, old browsers) everything degrades to a no-op —
 * notifications are non-critical and must never throw.
 */

/** Extra payload stashed on the Notification instance for onAction callbacks. */
export interface NotificationOptions {
  title: string;
  body?: string;
  icon?: string;
  extra?: Record<string, unknown>;
}

/** Payload handed to onAction callbacks. */
export interface NotificationPayload {
  title: string;
  body?: string;
  icon?: string;
  extra?: Record<string, unknown>;
}

const actionCallbacks = new Set<(notification: NotificationPayload) => void>();

/** True when the Web Notifications API exists in this environment. */
function isSupported(): boolean {
  return typeof Notification !== "undefined";
}

export async function isNotificationPermissionGranted(): Promise<boolean> {
  return isSupported() && Notification.permission === "granted";
}

export async function requestNotificationPermission(): Promise<"granted" | "denied"> {
  if (!isSupported()) return "denied";
  try {
    const result = await Notification.requestPermission();
    return result === "granted" ? "granted" : "denied";
  } catch {
    // Insecure context or other restrictions — degrade silently.
    return "denied";
  }
}

export async function sendNotification(options: NotificationOptions | string): Promise<void> {
  if (!isSupported() || Notification.permission !== "granted") return;
  try {
    const opts = typeof options === "string" ? { title: options } : options;
    const { title, body, icon, extra } = opts;
    const notification = new Notification(title, {
      ...(body ? { body } : {}),
      ...(icon ? { icon } : {}),
    });

    // `extra` is not part of the Web Notifications spec — stash it on the
    // instance so onAction callbacks can read it when the user clicks.
    const extended = notification as Notification & NotificationPayload;
    extended.extra = extra;

    notification.addEventListener("click", () => {
      for (const cb of actionCallbacks) {
        try {
          cb(extended);
        } catch {
          // A misbehaving listener must not break the rest.
        }
      }
      notification.close();
    });
  } catch {
    // Browser refused to create the notification (e.g. transient quota or
    // service-worker restrictions) — drop it silently.
  }
}

export async function onAction(
  cb: (notification: NotificationPayload) => void,
): Promise<{ unregister: () => void }> {
  actionCallbacks.add(cb);
  return {
    unregister: () => {
      actionCallbacks.delete(cb);
    },
  };
}
