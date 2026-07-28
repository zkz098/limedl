// Mock implementations of @tauri-apps/plugin-notification for NAS/Web mode.
// In a browser environment, OS-level notifications are not available via Tauri APIs,
// so these stubs gracefully degrade to no-ops.

export const isPermissionGranted = async (): Promise<boolean> => false;

export const requestPermission = async (): Promise<"granted" | "denied"> => "denied";

export const sendNotification = async (_options: unknown): Promise<void> => {
  // No-op: browser environment has no Tauri notification API
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const onAction = async (
  _cb: (notification: any) => void,
): Promise<{ unregister: () => void }> => {
  // No-op: browser environment has no Tauri notification API
  return { unregister: () => {} };
};
