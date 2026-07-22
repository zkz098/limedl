import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UiTextField from "../../components/ui/UiTextField.vue";

describe("UiTextField", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders an input element", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "" },
    });
    expect(wrapper.find("input").exists()).toBe(true);
  });

  it("renders readonly input when no unit", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "hello" },
    });
    const input = wrapper.find("input");
    expect(input.attributes("value")).toBe("hello");
  });

  it("forwards placeholder prop", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "", placeholder: "Enter text…" },
    });
    expect(wrapper.find("input").attributes("placeholder")).toBe("Enter text…");
  });

  it("sets disabled attribute when disabled prop is true", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "", disabled: true },
    });
    expect(wrapper.find("input").attributes("disabled")).toBeDefined();
  });

  it("uses type=text by default", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "" },
    });
    expect(wrapper.find("input").attributes("type")).toBe("text");
  });

  it("applies type=number when prop is set", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 0, type: "number" },
    });
    expect(wrapper.find("input").attributes("type")).toBe("number");
    expect(wrapper.find("input").attributes("inputmode")).toBe("numeric");
  });

  it("renders min/max/step attributes for number type", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 5, type: "number", min: 0, max: 100, step: 1 },
    });
    const input = wrapper.find("input");
    expect(input.attributes("min")).toBe("0");
    expect(input.attributes("max")).toBe("100");
    expect(input.attributes("step")).toBe("1");
  });

  // ── Unit rendering ─────────────────────────────────────────
  it("renders wrapper with suffix unit", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 42, type: "number", unit: "MiB" },
    });
    expect(wrapper.find(".ui-textfield-wrapper").exists()).toBe(true);
    expect(wrapper.text()).toContain("MiB");
  });

  it("renders prefix unit when unitPosition is prefix", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "$100", unit: "$", unitPosition: "prefix" },
    });
    expect(wrapper.find(".ui-textfield__prefix").exists()).toBe(true);
    expect(wrapper.find(".ui-textfield__prefix").text()).toBe("$");
  });

  it("does not render wrapper when no unit", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "" },
    });
    expect(wrapper.find(".ui-textfield-wrapper").exists()).toBe(false);
  });

  it("applies disabled class on wrapper when disabled with unit", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 0, type: "number", unit: "MB", disabled: true },
    });
    expect(wrapper.find(".ui-textfield-wrapper--disabled").exists()).toBe(true);
  });

  // ── Events / v-model ───────────────────────────────────────
  it("emits update:modelValue with string on text input", async () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "" },
    });
    const input = wrapper.find("input");
    await input.setValue("new value");
    expect(wrapper.emitted("update:modelValue")).toBeTruthy();
    expect(wrapper.emitted("update:modelValue")![0]).toEqual(["new value"]);
  });

  it("emits update:modelValue with number on number input", async () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 0, type: "number" },
    });
    const input = wrapper.find("input");
    await input.setValue("42");
    expect(wrapper.emitted("update:modelValue")![0]).toEqual([42]);
  });

  it("emits null when number input is cleared", async () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: 5, type: "number" },
    });
    const input = wrapper.find("input");
    await input.setValue("");
    expect(wrapper.emitted("update:modelValue")![0]).toEqual([null]);
  });

  it("renders empty string when number modelValue is null", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: null, type: "number" },
    });
    expect(wrapper.find("input").attributes("value")).toBe("");
  });

  // ── Styling (readonly) ─────────────────────────────────────
  it("does not set readonly attribute by default", () => {
    const wrapper = mount(UiTextField, {
      props: { modelValue: "fixed" },
    });
    // UiTextField does not expose a readonly prop, so the attribute must be absent
    expect(wrapper.find("input").attributes("readonly")).toBeUndefined();
  });
});
