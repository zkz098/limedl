import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useNotificationStore } from "../../stores/notification";

describe("useNotificationStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("notify basics: creates a notification with correct properties", () => {
    const store = useNotificationStore();
    store.notify("Test message", "error");

    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0]).toMatchObject({
      id: 0,
      message: "Test message",
      type: "error",
    });
  });

  it("notify basics: pushes to notifications array (multiple entries)", () => {
    const store = useNotificationStore();
    store.notify("First", "info");
    store.notify("Second", "error");

    expect(store.notifications).toHaveLength(2);
    expect(store.notifications[0].message).toBe("First");
    expect(store.notifications[1].message).toBe("Second");
  });

  it("type shortcuts: notifySuccess sets type to success", () => {
    const store = useNotificationStore();
    store.notifySuccess("All good");
    expect(store.notifications[0].type).toBe("success");
  });

  it("type shortcuts: notifyError sets type to error", () => {
    const store = useNotificationStore();
    store.notifyError("Something broke");
    expect(store.notifications[0].type).toBe("error");
  });

  it("type shortcuts: notifyInfo sets type to info", () => {
    const store = useNotificationStore();
    store.notifyInfo("Heads up");
    expect(store.notifications[0].type).toBe("info");
  });

  it("type shortcuts: notifyWarning sets type to warning", () => {
    const store = useNotificationStore();
    store.notifyWarning("Caution");
    expect(store.notifications[0].type).toBe("warning");
  });

  it("type shortcuts: pass custom duration through shortcuts", () => {
    const store = useNotificationStore();
    store.notifySuccess("Quick success", 100);
    store.notifyError("Quick error", 200);

    // Advance 100ms — first should dismiss
    vi.advanceTimersByTime(100);
    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0].type).toBe("error");

    // Advance another 100ms — second should dismiss
    vi.advanceTimersByTime(100);
    expect(store.notifications).toHaveLength(0);
  });

  it("auto IDs: sequential IDs starting from 0", () => {
    const store = useNotificationStore();
    store.notify("A", "info");
    store.notify("B", "info");
    store.notify("C", "info");

    expect(store.notifications[0].id).toBe(0);
    expect(store.notifications[1].id).toBe(1);
    expect(store.notifications[2].id).toBe(2);
  });

  it("dismiss: removes notification by id", () => {
    const store = useNotificationStore();
    store.notify("Alpha", "info");
    store.notify("Beta", "info");
    store.notify("Gamma", "info");

    store.dismiss(1);

    expect(store.notifications).toHaveLength(2);
    expect(store.notifications.map((n) => n.id)).toEqual([0, 2]);
  });

  it("dismiss: clears the associated timer", () => {
    const store = useNotificationStore();
    store.notify("Timed", "info", 3600);

    store.dismiss(0);

    // Advance past the original timeout — should NOT auto-dismiss (timer was cleared)
    vi.advanceTimersByTime(3600);
    expect(store.notifications).toHaveLength(0);
  });

  it("dismiss: does NOT throw when dismissing a non-existent id", () => {
    const store = useNotificationStore();
    store.notify("Only one", "info");

    expect(() => store.dismiss(42)).not.toThrow();
    // Existing notification should be untouched
    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0].id).toBe(0);
  });

  it("dismiss: does NOT throw when dismissing on an empty store", () => {
    const store = useNotificationStore();
    expect(() => store.dismiss(0)).not.toThrow();
    expect(store.notifications).toHaveLength(0);
  });

  it("clearAll: removes all notifications", () => {
    const store = useNotificationStore();
    store.notify("A", "info");
    store.notify("B", "error");
    store.notify("C", "warning");

    store.clearAll();

    expect(store.notifications).toHaveLength(0);
  });

  it("clearAll: clears all timers (no stale auto-dismiss)", () => {
    const store = useNotificationStore();
    store.notify("A", "info", 500);
    store.notify("B", "info", 1000);

    store.clearAll();

    // Advance past both durations — nothing should reappear
    vi.advanceTimersByTime(2000);
    expect(store.notifications).toHaveLength(0);
  });

  it("auto-dismiss timer: notification is dismissed after default duration (3600ms)", () => {
    const store = useNotificationStore();
    store.notify("Auto dismiss me", "info");

    // Not yet dismissed
    vi.advanceTimersByTime(3599);
    expect(store.notifications).toHaveLength(1);

    // One more ms triggers the timeout
    vi.advanceTimersByTime(1);
    expect(store.notifications).toHaveLength(0);
  });

  it("custom duration: notification dismisses after specified duration, not 3600ms", () => {
    const store = useNotificationStore();
    store.notify("Quick", "info", 500);

    // Default duration has not elapsed, but custom duration has not elapsed either
    vi.advanceTimersByTime(499);
    expect(store.notifications).toHaveLength(1);

    // Hits custom timeout
    vi.advanceTimersByTime(1);
    expect(store.notifications).toHaveLength(0);
  });

  it("custom duration: a second notification with default duration is not affected by the custom one", () => {
    const store = useNotificationStore();
    store.notify("Quick", "info", 500);
    store.notify("Slow", "info");

    // Advance to 500ms — only quick one dismisses
    vi.advanceTimersByTime(500);
    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0].message).toBe("Slow");

    // Advance to 3600ms — slow one dismisses too
    vi.advanceTimersByTime(3100);
    expect(store.notifications).toHaveLength(0);
  });

  it("timer cleanup on manual dismiss: manual dismiss prevents auto-dismiss from firing", () => {
    const store = useNotificationStore();
    store.notify("Manual dismiss", "info", 3600);
    store.notify("Keep me", "info", 3600);

    // Manually dismiss the first one early
    store.dismiss(0);

    // Advance all the way past the duration
    vi.advanceTimersByTime(3600);

    // Only the manually dismissed notification should still be gone;
    // the second notification was auto-dismissed by its timer
    expect(store.notifications).toHaveLength(0);
  });

  it("clearAll after timer started: no stale dismisses fire", () => {
    const store = useNotificationStore();
    store.notify("Will be cleared", "info", 3600);

    // Clear all before the timer fires
    store.clearAll();

    // Spy on dismiss to verify it's never called again
    const dismissSpy = vi.spyOn(store, "dismiss");

    vi.advanceTimersByTime(3600);

    // dismiss should not have been called by any leftover timer
    expect(dismissSpy).not.toHaveBeenCalled();
  });

  it("multiple concurrent timers: each dismisses at its correct time", () => {
    const store = useNotificationStore();
    store.notify("Fast", "info", 1000);
    store.notify("Medium", "info", 2000);
    store.notify("Slow", "info", 3000);

    // At t=0, all three are present
    expect(store.notifications).toHaveLength(3);

    // Advance to t=1000ms — only "Fast" should dismiss
    vi.advanceTimersByTime(1000);
    expect(store.notifications).toHaveLength(2);
    expect(store.notifications.map((n) => n.message)).toEqual(["Medium", "Slow"]);

    // Advance to t=2000ms — "Medium" should dismiss
    vi.advanceTimersByTime(1000);
    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0].message).toBe("Slow");

    // Advance to t=3000ms — "Slow" should dismiss
    vi.advanceTimersByTime(1000);
    expect(store.notifications).toHaveLength(0);
  });

  it("multiple concurrent timers: ids stay correct after partial dismissal", () => {
    const store = useNotificationStore();
    store.notify("First", "info", 500);
    store.notify("Second", "info", 1500);
    store.notify("Third", "info", 2500);

    // After 500ms, notification id=0 is gone, remaining are id=1, id=2
    vi.advanceTimersByTime(500);
    expect(store.notifications).toHaveLength(2);
    expect(store.notifications[0].id).toBe(1);
    expect(store.notifications[1].id).toBe(2);

    // After another 1000ms (t=1500), notification id=1 is gone
    vi.advanceTimersByTime(1000);
    expect(store.notifications).toHaveLength(1);
    expect(store.notifications[0].id).toBe(2);

    // After another 1000ms (t=2500), all gone
    vi.advanceTimersByTime(1000);
    expect(store.notifications).toHaveLength(0);
  });
});
