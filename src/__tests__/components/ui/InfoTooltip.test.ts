import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import InfoTooltip from "../../../components/ui/InfoTooltip.vue";

// ── Global Mocks ───────────────────────────────────────────────────

// Mock matchMedia for non-touch-device behavior (pointer: coarse → false)
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// Mock @floating-ui/dom — returns fake positioning data and calls cb immediately
const { mockCleanup } = vi.hoisted(() => ({
  mockCleanup: vi.fn(),
}));
vi.mock("@floating-ui/dom", () => ({
  computePosition: vi.fn().mockResolvedValue({
    x: 100,
    y: 200,
    middlewareData: { arrow: { x: 50, y: 0 } },
    placement: "top",
  }),
  autoUpdate: vi.fn((_trigger, _popup, cb) => {
    cb();
    return mockCleanup;
  }),
  offset: vi.fn(() => ({ name: "offset" })),
  flip: vi.fn(() => ({ name: "flip" })),
  shift: vi.fn((opts) => ({ name: "shift", options: opts })),
  arrow: vi.fn((opts) => ({ name: "arrow", options: opts })),
}));

// Mock i18n for the t() import used in aria-label.
// Component at src/components/ui/ imports from "../../i18n" = src/i18n.
// From test at src/__tests__/components/ui/, that's "../../../i18n".
vi.mock("../../../i18n", () => ({
  t: (key: string) => key,
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  Teleport: { template: "<div><slot /></div>" },
};

// ── Helpers ────────────────────────────────────────────────────────

function createTooltip(props: Record<string, unknown> = {}) {
  return mount(InfoTooltip, {
    props: { text: "Useful information about this setting", ...props },
    global: { stubs },
    attachTo: document.body,
  });
}

/** Helper to query the tooltip popup from a mounted wrapper. */
function popup(wrapper: ReturnType<typeof mount>) {
  return wrapper.find('[role="tooltip"]');
}

/** Helper to check popup visibility via v-show (inline style display). */
function isPopupVisible(wrapper: ReturnType<typeof mount>): boolean {
  const el = popup(wrapper).element as HTMLElement;
  return el.style.display !== "none";
}

/** Helper to get the trigger button. */
function trigger(wrapper: ReturnType<typeof mount>) {
  return wrapper.find("button.info-tooltip__icon");
}

// ── Tests ──────────────────────────────────────────────────────────

describe("InfoTooltip", () => {
  let wrapper: ReturnType<typeof mount>;

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    wrapper?.unmount();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  // ── 1. Render ────────────────────────────────────────────────────

  it("renders trigger button with info icon and aria-label", () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    expect(btn.exists()).toBe(true);
    expect(btn.find(".i-ri-information-line").exists()).toBe(true);
    expect(btn.attributes("aria-label")).toBe("common.information");
  });

  // ── 2. Hover open delay ──────────────────────────────────────────

  it("shows tooltip content after 300ms hover delay", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    await btn.trigger("mouseenter");

    // Before 300ms — still hidden
    await vi.advanceTimersByTimeAsync(290);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(false);

    // Cross the 300ms threshold
    await vi.advanceTimersByTimeAsync(20);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);
  });

  it("does NOT show tooltip before 300ms hover delay", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    await btn.trigger("mouseenter");

    // Only 100ms in — should still be hidden
    await vi.advanceTimersByTimeAsync(100);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(false);
  });

  // ── 3. Hover close delay ─────────────────────────────────────────

  it("hides tooltip 150ms after mouse leaves (not hovering)", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    // Open via hover
    await btn.trigger("mouseenter");
    await vi.advanceTimersByTimeAsync(310);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);

    // Leave — should start 150ms hide timer
    await btn.trigger("mouseleave");

    // Before 150ms — still visible
    await vi.advanceTimersByTimeAsync(140);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);

    // Past 150ms — now hidden
    await vi.advanceTimersByTimeAsync(20);
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(false);
  });

  // ── 4. Click pin/unpin ───────────────────────────────────────────

  it("clicking trigger shows tooltip immediately (no delay)", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    // Click once — opens immediately with no setTimeout
    await btn.trigger("click");
    // No need to advance fake timers; click open is synchronous
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);
  });

  it("clicking pinned tooltip closes it", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    // First click — opens and pins
    await btn.trigger("click");
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);

    // Second click — closes
    await btn.trigger("click");
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(false);
  });

  // ── 5. Singleton behavior ────────────────────────────────────────

  it("only one tooltip open at a time (singleton): open A, then open B → A closes", async () => {
    // Mount two independent tooltip instances
    const wrapperA = createTooltip({ text: "Tooltip A" });
    const wrapperB = createTooltip({ text: "Tooltip B" });

    const btnA = trigger(wrapperA);
    const btnB = trigger(wrapperB);

    // Open A by click
    await btnA.trigger("click");
    await nextTick();
    expect(isPopupVisible(wrapperA)).toBe(true);
    expect(isPopupVisible(wrapperB)).toBe(false);

    // Open B by click — A should close (singleton contract)
    await btnB.trigger("click");
    await nextTick();

    expect(isPopupVisible(wrapperB)).toBe(true);
    expect(isPopupVisible(wrapperA)).toBe(false);

    wrapperA.unmount();
    wrapperB.unmount();
  });

  // ── 6. Escape key ────────────────────────────────────────────────

  it("pressing Escape closes the tooltip", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);

    // Open by click
    await btn.trigger("click");
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(true);

    // Dispatch Escape keydown on document
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();
    expect(isPopupVisible(wrapper)).toBe(false);
  });

  // ── 7. Accessibility ─────────────────────────────────────────────

  it("has correct accessibility attributes (role='tooltip', aria-describedby when open)", async () => {
    wrapper = createTooltip();
    const btn = trigger(wrapper);
    const tooltipEl = popup(wrapper);

    // Popup always has role="tooltip"
    expect(tooltipEl.attributes("role")).toBe("tooltip");

    // When closed, trigger has no aria-describedby
    expect(btn.attributes("aria-describedby")).toBeUndefined();

    // Open by click
    await btn.trigger("click");
    await nextTick();

    // When open, trigger has aria-describedby pointing to tooltip id
    const describedBy = btn.attributes("aria-describedby");
    expect(describedBy).toBeTruthy();
    expect(describedBy).toBe(tooltipEl.attributes("id"));

    // Close
    await btn.trigger("click");
    await nextTick();

    // aria-describedby removed again
    expect(btn.attributes("aria-describedby")).toBeUndefined();
  });

  // ── 8. Content ───────────────────────────────────────────────────

  it("renders props.text in tooltip content", async () => {
    wrapper = createTooltip({ text: "Helpful explanation text" });
    const btn = trigger(wrapper);

    await btn.trigger("click");
    await nextTick();

    const tooltipEl = popup(wrapper);
    expect(tooltipEl.text()).toContain("Helpful explanation text");
  });
});
