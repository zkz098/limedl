import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UiProgress from "../../components/ui/UiProgress.vue";

describe("UiProgress", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders a progress track", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 50 },
    });
    expect(wrapper.find(".ui-progress__track").exists()).toBe(true);
    expect(wrapper.find(".ui-progress__value").exists()).toBe(true);
  });

  it("sets width style based on value prop", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 75 },
    });
    const bar = wrapper.find(".ui-progress__value");
    expect(bar.attributes("style")).toContain("width: 75%");
  });

  it("clamps value between 0 and 100", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 150 },
    });
    const bar = wrapper.find(".ui-progress__value");
    expect(bar.attributes("style")).toContain("width: 100%");
  });

  it("clamps negative value to 0", () => {
    const wrapper = mount(UiProgress, {
      props: { value: -10 },
    });
    const bar = wrapper.find(".ui-progress__value");
    expect(bar.attributes("style")).toContain("width: 0%");
  });

  // ── Indeterminate ──────────────────────────────────────────
  it("renders indeterminate class when indeterminate prop is true", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 0, indeterminate: true },
    });
    const bar = wrapper.find(".ui-progress__value");
    expect(bar.classes()).toContain("ui-progress__value--indeterminate");
  });

  it("does not set width style when indeterminate", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 50, indeterminate: true },
    });
    const bar = wrapper.find(".ui-progress__value");
    expect(bar.attributes("style")).toBeUndefined();
  });

  // ── Label ──────────────────────────────────────────────────
  it("does not render label when showLabel is false", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 50 },
    });
    expect(wrapper.find(".ui-progress__label").exists()).toBe(false);
  });

  it("renders auto label when showLabel is true and no custom label", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 50, showLabel: true },
    });
    const label = wrapper.find(".ui-progress__label");
    expect(label.exists()).toBe(true);
    expect(label.text()).toContain("50.0%");
  });

  it("renders custom label when showLabel and label prop are set", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 50, showLabel: true, label: "Downloading…" },
    });
    expect(wrapper.find(".ui-progress__label").text()).toBe("Downloading…");
  });

  // ── Test ID ────────────────────────────────────────────────
  it("has data-testid on the value bar", () => {
    const wrapper = mount(UiProgress, {
      props: { value: 30 },
    });
    expect(wrapper.find('[data-testid="task-progress-bar"]').exists()).toBe(true);
  });
});
