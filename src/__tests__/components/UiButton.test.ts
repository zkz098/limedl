import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UiButton from "../../components/ui/UiButton.vue";

describe("UiButton", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders a button element", () => {
    const wrapper = mount(UiButton, {
      slots: { default: "Click me" },
    });
    const button = wrapper.find("button");
    expect(button.exists()).toBe(true);
  });

  it("has type='button' by default", () => {
    const wrapper = mount(UiButton);
    expect(wrapper.find("button").attributes("type")).toBe("button");
  });

  it("renders slot content as button text", () => {
    const wrapper = mount(UiButton, {
      slots: { default: "Click me" },
    });
    expect(wrapper.find("button").text()).toBe("Click me");
  });

  it("applies ui-button--primary class by default", () => {
    const wrapper = mount(UiButton);
    expect(wrapper.find("button").classes()).toContain("ui-button--primary");
  });

  it("applies ui-button--secondary class when variant='secondary'", () => {
    const wrapper = mount(UiButton, {
      props: { variant: "secondary" },
    });
    expect(wrapper.find("button").classes()).toContain("ui-button--secondary");
  });

  it("applies ui-button--danger class when variant='danger'", () => {
    const wrapper = mount(UiButton, {
      props: { variant: "danger" },
    });
    expect(wrapper.find("button").classes()).toContain("ui-button--danger");
  });

  it("applies ui-button--ghost class when variant='ghost'", () => {
    const wrapper = mount(UiButton, {
      props: { variant: "ghost" },
    });
    expect(wrapper.find("button").classes()).toContain("ui-button--ghost");
  });

  it("applies ui-button--sm class when size='sm'", () => {
    const wrapper = mount(UiButton, {
      props: { size: "sm" },
    });
    expect(wrapper.find("button").classes()).toContain("ui-button--sm");
  });

  // ── Props ─────────────────────────────────────────────────
  it("disabled prop sets disabled attribute", () => {
    const wrapper = mount(UiButton, {
      props: { disabled: true },
    });
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
  });

  it("disabled+loading both disable the button", () => {
    const wrapper = mount(UiButton, {
      props: { disabled: true, loading: true },
    });
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
  });

  it("type='submit' sets button type attribute to submit", () => {
    const wrapper = mount(UiButton, {
      props: { type: "submit" },
    });
    expect(wrapper.find("button").attributes("type")).toBe("submit");
  });

  it("applies ui-button--block class when block is true", () => {
    const wrapper = mount(UiButton, {
      props: { block: true },
    });
    expect(wrapper.find("button").classes()).toContain("ui-button--block");
  });

  // ── Icons ─────────────────────────────────────────────────
  it("renders icon element with correct class when icon prop is set", () => {
    const wrapper = mount(UiButton, {
      props: { icon: "i-ri-check-line" },
    });
    const icon = wrapper.find(".ui-button__icon");
    expect(icon.exists()).toBe(true);
    expect(icon.classes()).toContain("i-ri-check-line");
  });

  it("renders right-side icon when iconRight prop is set", () => {
    const wrapper = mount(UiButton, {
      props: { iconRight: "i-ri-arrow-right-line" },
    });
    const icons = wrapper.findAll(".ui-button__icon");
    expect(icons).toHaveLength(1);
    expect(icons[0].classes()).toContain("i-ri-arrow-right-line");
  });

  it("does not render icon when prop is empty string", () => {
    const wrapper = mount(UiButton, {
      props: { icon: "", iconRight: "" },
    });
    expect(wrapper.find(".ui-button__icon").exists()).toBe(false);
  });

  // ── Loading ───────────────────────────────────────────────
  it("renders a spinner when loading is true", () => {
    const wrapper = mount(UiButton, {
      props: { loading: true },
    });
    expect(wrapper.find(".ui-button__spinner").exists()).toBe(true);
  });

  it("adds is-loading class when loading is true", () => {
    const wrapper = mount(UiButton, {
      props: { loading: true },
    });
    expect(wrapper.find("button").classes()).toContain("is-loading");
  });

  it("sets aria-hidden on spinner during loading", () => {
    const wrapper = mount(UiButton, {
      props: { loading: true },
    });
    expect(wrapper.find(".ui-button__spinner").attributes("aria-hidden")).toBe("true");
  });

  it("disables button when loading is true", () => {
    const wrapper = mount(UiButton, {
      props: { loading: true },
    });
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
  });

  it("hides icon and iconRight during loading", () => {
    const wrapper = mount(UiButton, {
      props: { loading: true, icon: "i-ri-check-line", iconRight: "i-ri-arrow-right-line" },
    });
    // Spinner should be visible, not icons
    expect(wrapper.find(".ui-button__spinner").exists()).toBe(true);
    expect(wrapper.find(".ui-button__icon").exists()).toBe(false);
  });

  // ── Events ────────────────────────────────────────────────
  it("emits click event when clicked", async () => {
    const wrapper = mount(UiButton, {
      slots: { default: "Click me" },
    });
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeTruthy();
    expect(wrapper.emitted("click")).toHaveLength(1);
  });

  it("does not emit click when button is disabled", async () => {
    const wrapper = mount(UiButton, {
      props: { disabled: true },
      slots: { default: "Click me" },
    });
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeUndefined();
  });

  it("does not emit click when button is loading", async () => {
    const wrapper = mount(UiButton, {
      props: { loading: true },
      slots: { default: "Click me" },
    });
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("click")).toBeUndefined();
  });
});
