import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { defineComponent } from "vue";
import { mount } from "@vue/test-utils";
import { usePolling } from "../../composables/usePolling";

describe("usePolling", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts with isPolling = false", () => {
    const { isPolling } = usePolling(vi.fn());
    expect(isPolling.value).toBe(false);
  });

  it("sets isPolling to true after start()", () => {
    const { isPolling, start } = usePolling(vi.fn());
    start();
    expect(isPolling.value).toBe(true);
  });

  it("sets isPolling to false after stop()", () => {
    const { isPolling, start, stop } = usePolling(vi.fn());
    start();
    expect(isPolling.value).toBe(true);
    stop();
    expect(isPolling.value).toBe(false);
  });

  it("calls the callback immediately on start()", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback, 1000);

    start();
    // Let the initial poll microtask complete
    await vi.advanceTimersByTimeAsync(0);

    expect(callback).toHaveBeenCalledTimes(1);
  });

  it("calls the callback at the specified interval", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback, 500);

    start();
    await vi.advanceTimersByTimeAsync(0); // flush initial call
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance by one interval
    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(2);

    // Advance by another interval
    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(3);
  });

  it("uses the default interval of 2000ms when not specified", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback);

    start();
    await vi.advanceTimersByTimeAsync(0); // initial call
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance less than default interval - no new call
    await vi.advanceTimersByTimeAsync(1999);
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance past default interval
    await vi.advanceTimersByTimeAsync(1);
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it("does not call the callback after stop()", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start, stop } = usePolling(callback, 500);

    start();
    await vi.advanceTimersByTimeAsync(0); // initial call
    expect(callback).toHaveBeenCalledTimes(1);

    stop();

    // Advance time significantly
    await vi.advanceTimersByTimeAsync(5000);
    // Should still be 1 (no interval calls after stop)
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it("does nothing if start() is called twice", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback, 500);

    start();
    await vi.advanceTimersByTimeAsync(0); // initial call
    expect(callback).toHaveBeenCalledTimes(1);

    // Call start again (should be a no-op)
    start();

    await vi.advanceTimersByTimeAsync(500);
    // Should only have 2 calls (initial + 1 interval), not 3
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it("continues polling after callback throws", async () => {
    const callback = vi.fn().mockRejectedValue(new Error("Network error"));
    const { start } = usePolling(callback, 500);

    // Spy on console.error to suppress expected error logs
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    start();

    await vi.advanceTimersByTimeAsync(0); // initial call (fails)
    expect(callback).toHaveBeenCalledTimes(1);

    // Interval should still fire despite previous error
    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(3);

    consoleSpy.mockRestore();
  });

  it("calls console.error when callback throws", async () => {
    const callback = vi.fn().mockRejectedValue(new Error("Network error"));
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const { start } = usePolling(callback, 500);

    start();
    await vi.advanceTimersByTimeAsync(0);

    expect(consoleSpy).toHaveBeenCalledWith(
      "[usePolling] Error during polling:",
      expect.any(Error),
    );

    consoleSpy.mockRestore();
  });

  it("does not start polling twice on the same instance", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback, 500);

    // First start
    start();
    await vi.advanceTimersByTimeAsync(0);
    expect(callback).toHaveBeenCalledTimes(1);

    // Second start should be a no-op
    start();
    await vi.advanceTimersByTimeAsync(500);

    // Should only have 2 calls (initial + 1 interval), not 3
    expect(callback).toHaveBeenCalledTimes(2);

    // Advance another interval to confirm only one timer is running
    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(3);
  });

  it("stops and starts again correctly (restart cycle)", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);
    const { isPolling, start, stop } = usePolling(callback, 500);

    // First cycle
    start();
    await vi.advanceTimersByTimeAsync(0);
    expect(callback).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(2);

    stop();
    expect(isPolling.value).toBe(false);

    // Second cycle
    start();
    await vi.advanceTimersByTimeAsync(0);
    expect(callback).toHaveBeenCalledTimes(3);

    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(4);

    stop();
    expect(isPolling.value).toBe(false);
  });

  it("calling start() twice does not create duplicate intervals", async () => {
    vi.spyOn(globalThis, "setInterval");
    const callback = vi.fn().mockResolvedValue(undefined);
    const { start } = usePolling(callback, 500);

    // First start should create one interval
    start();
    await vi.advanceTimersByTimeAsync(0);
    expect(callback).toHaveBeenCalledTimes(1);
    expect(setInterval).toHaveBeenCalledTimes(1);

    // Call start again — guard should prevent a second interval
    start();
    expect(setInterval).toHaveBeenCalledTimes(1);

    // Advance time — should still only have one timer running
    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(500);
    expect(callback).toHaveBeenCalledTimes(3);
  });

  it("does not call the callback after component unmount", async () => {
    const callback = vi.fn().mockResolvedValue(undefined);

    const TestComponent = defineComponent({
      setup() {
        const { start } = usePolling(callback, 500);
        start();
        // No template needed — the polling is all we care about.
        return () => null;
      },
    });

    const wrapper = mount(TestComponent);

    // Flush the initial poll microtask
    await vi.advanceTimersByTimeAsync(0);
    expect(callback).toHaveBeenCalledTimes(1);

    // Unmount the component — this triggers onUnmounted → stop()
    wrapper.unmount();

    // Advance time significantly past the interval
    await vi.advanceTimersByTimeAsync(5000);

    // Callback should not have been called again after unmount
    expect(callback).toHaveBeenCalledTimes(1);
  });
});
