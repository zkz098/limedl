import { ref } from "vue";
import { createGlobalState } from "@vueuse/core";
import { useNotification } from "./useNotification";
import { t } from "../i18n";

/**
 * Reactive browser network status (online/offline).
 *
 * Listens to `window` `online` / `offline` events and shows toast
 * notifications when the connectivity state changes, so the user can
 * distinguish network failures from software bugs.
 *
 * Uses `createGlobalState` so the singleton is shared across the app.
 */
export const useNetworkStatus = createGlobalState(() => {
  const isOnline = ref(navigator.onLine);
  let wasOffline = false;

  function handleOnline() {
    isOnline.value = true;
    if (wasOffline) {
      wasOffline = false;
      useNotification().notifySuccess(t("notifications.networkOnline"), 4000);
    }
  }

  function handleOffline() {
    isOnline.value = false;
    wasOffline = true;
    useNotification().notifyWarning(t("notifications.networkOffline"), 10000);
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
