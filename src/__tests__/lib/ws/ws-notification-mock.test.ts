import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  isPermissionGranted,
  onAction,
  requestPermission,
  sendNotification,
  type NotificationPayload,
} from "../../../lib/ws/ws-notification-mock";

// jsdom does not implement the Web Notifications API, so we install a fake
// Notification class on globalThis to exercise the mock's browser behavior.

type FakeClickHandler = () => void;

class FakeNotification {
  static permission: NotificationPermission = "default";
  static requestPermission = vi.fn(
    async (): Promise<NotificationPermission> => FakeNotification.permission,
  );
  static instances: FakeNotification[] = [];
  static emitted: Array<{ title: string; options: Record<string, unknown> }> = [];

  readonly title: string;
  readonly options: Record<string, unknown>;
  /** Attached by the mock when `extra` is passed to sendNotification. */
  extra?: Record<string, unknown>;
  private handlers = new Map<string, FakeClickHandler[]>();
  close = vi.fn();

  constructor(title: string, options: Record<string, unknown> = {}) {
    this.title = title;
    this.options = options;
    FakeNotification.instances.push(this);
    FakeNotification.emitted.push({ title, options });
  }

  addEventListener(event: string, handler: FakeClickHandler) {
    const list = this.handlers.get(event) ?? [];
    list.push(handler);
    this.handlers.set(event, list);
  }

  dispatch(event: string) {
    for (const handler of this.handlers.get(event) ?? []) {
      handler();
    }
  }
}

beforeEach(() => {
  FakeNotification.instances = [];
  FakeNotification.emitted = [];
  FakeNotification.permission = "default";
  FakeNotification.requestPermission.mockClear();
  vi.stubGlobal("Notification", FakeNotification);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ws-notification-mock", () => {
  describe("isPermissionGranted", () => {
    it("returns false when permission is not granted", async () => {
      FakeNotification.permission = "denied";
      expect(await isPermissionGranted()).toBe(false);

      FakeNotification.permission = "default";
      expect(await isPermissionGranted()).toBe(false);
    });

    it("returns true when permission is granted", async () => {
      FakeNotification.permission = "granted";
      expect(await isPermissionGranted()).toBe(true);
    });

    it("returns false when the Notification API is unavailable", async () => {
      vi.stubGlobal("Notification", undefined);
      expect(await isPermissionGranted()).toBe(false);
    });
  });

  describe("requestPermission", () => {
    it("resolves granted when the browser grants permission", async () => {
      FakeNotification.permission = "granted";
      expect(await requestPermission()).toBe("granted");
      expect(FakeNotification.requestPermission).toHaveBeenCalledTimes(1);
    });

    it("resolves denied for denied or default", async () => {
      FakeNotification.permission = "denied";
      expect(await requestPermission()).toBe("denied");

      FakeNotification.permission = "default";
      expect(await requestPermission()).toBe("denied");
    });

    it("resolves denied when the API is unavailable", async () => {
      vi.stubGlobal("Notification", undefined);
      expect(await requestPermission()).toBe("denied");
    });

    it("resolves denied when requestPermission rejects", async () => {
      FakeNotification.requestPermission.mockRejectedValueOnce(new Error("insecure"));
      expect(await requestPermission()).toBe("denied");
    });
  });

  describe("sendNotification", () => {
    it("creates a Notification with title, body and icon when granted", async () => {
      FakeNotification.permission = "granted";

      await sendNotification({ title: "Done", body: "file.zip", icon: "/icon.png" });

      expect(FakeNotification.emitted).toEqual([
        { title: "Done", options: { body: "file.zip", icon: "/icon.png" } },
      ]);
    });

    it("accepts a plain string as the title", async () => {
      FakeNotification.permission = "granted";

      await sendNotification("Tauri is awesome!");

      expect(FakeNotification.emitted).toEqual([{ title: "Tauri is awesome!", options: {} }]);
    });

    it("stashes extra on the instance for click handling", async () => {
      FakeNotification.permission = "granted";

      await sendNotification({ title: "Done", extra: { downloadId: "task-1" } });

      expect(FakeNotification.instances[0].extra).toEqual({ downloadId: "task-1" });
    });

    it("does nothing when permission is not granted", async () => {
      FakeNotification.permission = "denied";

      await sendNotification({ title: "Done" });

      expect(FakeNotification.emitted).toEqual([]);
    });

    it("does nothing and does not throw when the API is unavailable", async () => {
      vi.stubGlobal("Notification", undefined);

      await expect(sendNotification({ title: "Done" })).resolves.toBeUndefined();
    });
  });

  describe("onAction", () => {
    it("fires the callback with the notification payload when clicked", async () => {
      FakeNotification.permission = "granted";
      const callback = vi.fn<(notification: NotificationPayload) => void>();

      const listener = await onAction(callback);
      await sendNotification({ title: "Done", extra: { downloadId: "task-1" } });

      FakeNotification.instances[0].dispatch("click");

      expect(callback).toHaveBeenCalledTimes(1);
      const payload = callback.mock.calls[0][0];
      expect(payload.title).toBe("Done");
      expect(payload.extra?.downloadId).toBe("task-1");

      listener.unregister();
    });

    it("unregister stops further callbacks", async () => {
      FakeNotification.permission = "granted";
      const callback = vi.fn<(notification: NotificationPayload) => void>();

      const listener = await onAction(callback);
      listener.unregister();
      await sendNotification({ title: "Done", extra: { downloadId: "task-1" } });

      FakeNotification.instances[0].dispatch("click");

      expect(callback).not.toHaveBeenCalled();
    });

    it("closes the notification after click", async () => {
      FakeNotification.permission = "granted";
      const listener = await onAction(vi.fn());
      await sendNotification({ title: "Done" });

      FakeNotification.instances[0].dispatch("click");

      expect(FakeNotification.instances[0].close).toHaveBeenCalledTimes(1);
      listener.unregister();
    });

    it("returns a working unregister handle when the API is unavailable", async () => {
      vi.stubGlobal("Notification", undefined);

      const listener = await onAction(vi.fn());
      expect(typeof listener.unregister).toBe("function");
      expect(() => listener.unregister()).not.toThrow();
    });
  });
});
