import { ref } from "vue";
import { defineStore } from "pinia";
import { useNotificationStore } from "./notification";
import { t } from "../i18n";

/**
 * Reactive browser network status (online/offline).
 *
 * Listens to `window` `online` / `offline` events and shows toast
 * notifications when the connectivity state changes, so the user can
 * distinguish network failures from software bugs.
 */
export const useNetworkStatusStore = defineStore("networkStatus", () => {
  const isOnline = ref(navigator.onLine);
  let wasOffline = false;

  function handleOnline() {
    isOnline.value = true;
    if (wasOffline) {
      wasOffline = false;
      useNotificationStore().notifySuccess(t("notifications.networkOnline"), 4000);
    }
  }

  function handleOffline() {
    isOnline.value = false;
    wasOffline = true;
    useNotificationStore().notifyWarning(t("notifications.networkOffline"), 10000);
  }

  function start() {
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);
    // Set initial state
    isOnline.value = navigator.onLine;
  }

  function stop() {
    window.removeEventListener("online", handleOnline);
    window.removeEventListener("offline", handleOffline);
  }

  return {
    isOnline,
    start,
    stop,
  };
});
