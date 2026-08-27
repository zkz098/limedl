// Web Notifications API implementation of @tauri-apps/plugin-notification for
// NAS/Web mode. In a browser there is no Tauri notification bridge, so we use
// the standard Web Notifications API instead. When the API is unavailable
// (jsdom test environments, old browsers) we gracefully degrade to no-ops —
// never throw, notifications are non-critical.

/** Extra payload stashed on the Notification instance for onAction callbacks. */
export interface NotificationOptions {
  title: string;
  body?: string;
  icon?: string;
  extra?: Record<string, unknown>;
}

/** Payload handed to onAction callbacks (mirrors the plugin's Options shape). */
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

export const isPermissionGranted = async (): Promise<boolean> => {
  return isSupported() && Notification.permission === "granted";
};

export const requestPermission = async (): Promise<"granted" | "denied"> => {
  if (!isSupported()) return "denied";
  try {
    const result = await Notification.requestPermission();
    return result === "granted" ? "granted" : "denied";
  } catch {
    // Insecure context or other restrictions — degrade silently.
    return "denied";
  }
};

export const sendNotification = async (options: NotificationOptions | string): Promise<void> => {
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
};

export const onAction = async (
  cb: (notification: NotificationPayload) => void,
): Promise<{ unregister: () => void }> => {
  actionCallbacks.add(cb);
  return {
    unregister: () => {
      actionCallbacks.delete(cb);
    },
  };
};
