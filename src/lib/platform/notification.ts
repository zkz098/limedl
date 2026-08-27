import {
  isPermissionGranted as tauriIsPermissionGranted,
  onAction as tauriOnAction,
  requestPermission as tauriRequestPermission,
  sendNotification as tauriSendNotification,
} from "@tauri-apps/plugin-notification";
import { isTauri } from "./env";

export interface NotificationOptions {
  title: string;
  body?: string;
  icon?: string;
  extra?: Record<string, unknown>;
}

export interface NotificationPayload {
  title: string;
  body?: string;
  icon?: string;
  extra?: Record<string, unknown>;
}

export async function isNotificationPermissionGranted(): Promise<boolean> {
  if (isTauri() && typeof tauriIsPermissionGranted === "function") {
    return tauriIsPermissionGranted();
  }
  return typeof Notification !== "undefined" && Notification.permission === "granted";
}

export async function requestNotificationPermission(): Promise<"granted" | "denied"> {
  if (isTauri() && typeof tauriRequestPermission === "function") {
    const result = await tauriRequestPermission();
    return result === "granted" ? "granted" : "denied";
  }
  if (typeof Notification === "undefined") return "denied";
  try {
    const result = await Notification.requestPermission();
    return result === "granted" ? "granted" : "denied";
  } catch {
    return "denied";
  }
}

export async function sendNotification(options: NotificationOptions | string): Promise<void> {
  if (isTauri() && typeof tauriSendNotification === "function") {
    return tauriSendNotification(options);
  }
  if (typeof Notification === "undefined" || Notification.permission !== "granted") return;
  try {
    const opts = typeof options === "string" ? { title: options } : options;
    const notification = new Notification(opts.title, {
      body: opts.body,
      icon: opts.icon,
    });
    notification.addEventListener("close", () => {}, { once: true });
  } catch {
    // Ignored
  }
}

export async function onAction(
  cb: (notification: NotificationPayload) => void,
): Promise<{ unregister: () => void }> {
  if (isTauri() && typeof tauriOnAction === "function") {
    return tauriOnAction(cb);
  }
  return { unregister: () => {} };
}
