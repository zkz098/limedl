import { onBeforeUnmount, ref } from "vue";

export function useSettingsNotification() {
  const notificationMessage = ref("");
  let notificationTimer: ReturnType<typeof setTimeout> | null = null;

  function showNotification(message: string) {
    notificationMessage.value = message;
    if (notificationTimer) {
      clearTimeout(notificationTimer);
    }
    notificationTimer = setTimeout(() => {
      notificationMessage.value = "";
      notificationTimer = null;
    }, 2200);
  }

  onBeforeUnmount(() => {
    if (notificationTimer) {
      clearTimeout(notificationTimer);
    }
  });

  return {
    notificationMessage,
    showNotification,
  };
}
