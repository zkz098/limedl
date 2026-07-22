import { describe, it, expect, vi, beforeAll } from "vitest";
import { mount } from "@vue/test-utils";
import UiSelect from "../../components/ui/UiSelect.vue";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("@vueuse/core", () => ({
  onClickOutside: vi.fn(() => () => {}),
}));

// jsdom does not implement scrollIntoView
beforeAll(() => {
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
});

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  Teleport: { template: "<div><slot /></div>" },
};

// ── Fixtures ───────────────────────────────────────────────────────

const OPTIONS = [
  { label: "Option A", value: "a" },
  { label: "Option B", value: "b" },
  { label: "Option C", value: "c" },
];

// ── Tests ──────────────────────────────────────────────────────────

describe("UiSelect", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders a trigger button", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__trigger").exists()).toBe(true);
  });

  it("displays placeholder when no option selected", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__label").text()).toBe("Select…");
  });

  it("displays selected option label", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: "b", options: OPTIONS },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__label").text()).toBe("Option B");
  });

  it("has aria-haspopup='listbox' and aria-expanded attributes", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });
    const trigger = wrapper.find(".ui-select__trigger");
    expect(trigger.attributes("aria-haspopup")).toBe("listbox");
    expect(trigger.attributes("aria-expanded")).toBe("false");
  });

  it("renders custom placeholder text", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS, placeholder: "Choose…" },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__label").text()).toBe("Choose…");
  });

  // ── Dropdown open/close ────────────────────────────────────
  it("opens panel when trigger is clicked", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);

    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(true);
    expect(wrapper.find(".ui-select__trigger").attributes("aria-expanded")).toBe("true");
  });

  it("closes panel when trigger is clicked again", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(true);

    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  it("does not open when disabled", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS, disabled: true },
      global: { stubs },
    });
    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  it("does not open when options array is empty", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: [] },
      global: { stubs },
    });
    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  // ── Selection ──────────────────────────────────────────────
  it("emits update:modelValue when option is clicked", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    const options = wrapper.findAll(".ui-select__option");
    expect(options).toHaveLength(3);

    await options[1].trigger("click");
    expect(wrapper.emitted("update:modelValue")).toBeTruthy();
    expect(wrapper.emitted("update:modelValue")![0]).toEqual(["b"]);
  });

  it("closes panel after selecting an option", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    await wrapper.findAll(".ui-select__option")[0].trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  it("marks the selected option with is-selected class", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: "b", options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    const optionEls = wrapper.findAll(".ui-select__option");
    expect(optionEls[1].classes()).toContain("is-selected");
    expect(optionEls[0].classes()).not.toContain("is-selected");
    expect(optionEls[2].classes()).not.toContain("is-selected");
  });

  it("sets aria-selected on the selected option", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: "c", options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    const optionEls = wrapper.findAll(".ui-select__option");
    expect(optionEls[2].attributes("aria-selected")).toBe("true");
    expect(optionEls[0].attributes("aria-selected")).toBe("false");
  });

  // ── Keyboard navigation ────────────────────────────────────
  it("opens panel on ArrowDown keydown and navigates", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    const trigger = wrapper.find(".ui-select__trigger");
    await trigger.trigger("keydown", { key: "ArrowDown" });

    // Panel should open
    expect(wrapper.find(".ui-select__panel").exists()).toBe(true);
  });

  it("selects active option on Enter keydown in panel", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    // Navigate down twice to index 2
    const panel = wrapper.find(".ui-select__panel");
    await panel.trigger("keydown", { key: "ArrowDown" });
    await panel.trigger("keydown", { key: "ArrowDown" });

    // Select with Enter
    await panel.trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("update:modelValue")).toBeTruthy();
    expect(wrapper.emitted("update:modelValue")![0]).toEqual(["c"]);
  });

  it("closes panel on Escape keydown", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    expect(wrapper.find(".ui-select__panel").exists()).toBe(true);

    await wrapper.find(".ui-select__panel").trigger("keydown", { key: "Escape" });
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  it("closes panel on Tab keydown", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");
    await wrapper.find(".ui-select__panel").trigger("keydown", { key: "Tab" });
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });

  it("focuses first option on Home keydown", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");

    // Navigate down twice then Home
    const panel = wrapper.find(".ui-select__panel");
    await panel.trigger("keydown", { key: "ArrowDown" });
    await panel.trigger("keydown", { key: "ArrowDown" });
    await panel.trigger("keydown", { key: "Home" });

    // Select with Enter to verify we're at index 0
    await panel.trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("update:modelValue")![0]).toEqual(["a"]);
  });

  it("focuses last option on End keydown", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("click");

    const panel = wrapper.find(".ui-select__panel");
    await panel.trigger("keydown", { key: "End" });
    await panel.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("update:modelValue")![0]).toEqual(["c"]);
  });

  // ── Disabled ───────────────────────────────────────────────
  it("sets disabled attribute on trigger button", () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS, disabled: true },
      global: { stubs },
    });
    expect(wrapper.find(".ui-select__trigger").attributes("disabled")).toBeDefined();
  });

  it("does not respond to keyboard when disabled", async () => {
    const wrapper = mount(UiSelect, {
      props: { modelValue: null, options: OPTIONS, disabled: true },
      global: { stubs },
    });

    await wrapper.find(".ui-select__trigger").trigger("keydown", { key: "ArrowDown" });
    expect(wrapper.find(".ui-select__panel").exists()).toBe(false);
  });
});
