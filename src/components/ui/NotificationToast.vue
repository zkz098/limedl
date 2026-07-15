<script setup lang="ts">
import { computed } from "vue";
import type { Notification } from "../../types/notification";

const props = defineProps<{
  notifications: Notification[];
}>();

const emit = defineEmits<{
  dismiss: [id: number];
}>();

function iconFor(type: Notification["type"]) {
  return {
    info: "i-ri-information-line",
    success: "i-ri-checkbox-circle-line",
    error: "i-ri-error-warning-line",
    warning: "i-ri-alert-line",
  }[type];
}

const reversed = computed(() => props.notifications.slice().reverse());
</script>

<template>
  <Teleport to="body">
    <div class="notification-toast-stack" role="region" aria-label="Notifications">
      <TransitionGroup name="toast">
        <div
          v-for="notification in reversed"
          :key="notification.id"
          class="notification-toast"
          :class="`notification-toast--${notification.type}`"
          role="alert"
        >
          <span
            class="notification-toast__icon"
            :class="iconFor(notification.type)"
            aria-hidden="true"
          />
          <span class="notification-toast__message">{{ notification.message }}</span>
          <button
            type="button"
            class="notification-toast__close"
            aria-label="Dismiss"
            @click="emit('dismiss', notification.id)"
          >
            <span class="i-ri-close-line" aria-hidden="true" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.notification-toast-stack {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 200;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-width: min(26rem, calc(100vw - 2rem));
  pointer-events: none;
}

.notification-toast {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  padding: 0.75rem 0.85rem;
  border-radius: var(--radius-lg);
  border: 1px solid var(--toast-border);
  background: var(--toast-bg);
  box-shadow: var(--shadow-card-hover);
  font-size: 0.85rem;
  line-height: 1.45;
  pointer-events: auto;
}

.notification-toast--info {
  --toast-bg: var(--color-info-bg);
  --toast-border: var(--color-info-border);
  color: var(--color-info-text);
}

.notification-toast--success {
  --toast-bg: var(--color-success-bg);
  --toast-border: var(--color-success-border);
  color: var(--color-success-text);
}

.notification-toast--error {
  --toast-bg: var(--color-danger-bg);
  --toast-border: var(--color-danger-border);
  color: var(--color-danger-text);
}

.notification-toast--warning {
  --toast-bg: var(--color-warning-bg);
  --toast-border: var(--color-warning-border);
  color: var(--color-warning-text);
}

.notification-toast__icon {
  flex: 0 0 auto;
  margin-top: 0.1rem;
  font-size: 1.05rem;
}

.notification-toast__message {
  flex: 1 1 auto;
  word-break: break-word;
}

.notification-toast__close {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.4rem;
  height: 1.4rem;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  opacity: 0.55;
  cursor: pointer;
  font-size: 0.95rem;
  margin: -0.15rem -0.25rem 0 0;
  transition: opacity 0.15s;
}

.notification-toast__close:hover {
  opacity: 1;
}

.toast-enter-active {
  transition: all 0.3s ease;
}

.toast-leave-active {
  transition: all 0.2s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(2rem) scale(0.95);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(2rem) scale(0.95);
}
</style>
