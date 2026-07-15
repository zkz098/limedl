import { ref } from "vue";
import { createGlobalState } from "@vueuse/core";
import type { Notification } from "../types/notification";

export const useNotification = createGlobalState(() => {
  let nextId = 0;
  const notifications = ref<Notification[]>([]);
  const timers = new Map<number, ReturnType<typeof setTimeout>>();
  function notify(message: string, type: Notification["type"] = "info", durationMs = 3600) {
    const id = nextId++;
    const notification: Notification = { id, message, type };
    notifications.value = [...notifications.value, notification];

    const timer = setTimeout(() => {
      dismiss(id);
    }, durationMs);
    timers.set(id, timer);
  }

  function dismiss(id: number) {
    const timer = timers.get(id);
    if (timer) {
      clearTimeout(timer);
      timers.delete(id);
    }
    notifications.value = notifications.value.filter((n) => n.id !== id);
  }

  function notifySuccess(message: string, durationMs?: number) {
    notify(message, "success", durationMs);
  }

  function notifyError(message: string, durationMs?: number) {
    notify(message, "error", durationMs);
  }

  function notifyInfo(message: string, durationMs?: number) {
    notify(message, "info", durationMs);
  }

  function notifyWarning(message: string, durationMs?: number) {
    notify(message, "warning", durationMs);
  }

  function clearAll() {
    for (const [, timer] of timers) {
      clearTimeout(timer);
    }
    timers.clear();
    notifications.value = [];
  }

  return {
    notifications,
    notify,
    notifySuccess,
    notifyError,
    notifyInfo,
    notifyWarning,
    dismiss,
    clearAll,
  };
});
