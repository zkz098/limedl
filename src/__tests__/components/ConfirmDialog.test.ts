import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import ConfirmDialog from "../../components/ui/ConfirmDialog.vue";

/** Stub for UiDialog — renders content in a simple div, no Teleport, avoids jsdom warts */
const UiDialogStub = {
  name: "UiDialog",
  template:
    '<div v-if="modelValue" class="ui-dialog-stub" :style="{ width }"><slot name="title" /><slot /></div>',
  props: ["modelValue", "width", "closeOnOverlay"],
};

function createWrapper(props: Record<string, unknown> = {}) {
  return mount(ConfirmDialog, {
    props: {
      modelValue: true,
      kicker: "Confirm Action",
      title: "Are you sure?",
      message: "This will delete the item.",
      confirmText: "Delete",
      cancelText: "Cancel",
      ...props,
    },
    global: {
      stubs: {
        UiDialog: UiDialogStub,
      },
    },
  });
}

describe("ConfirmDialog", () => {
  describe("Rendering", () => {
    it("renders kicker text", () => {
      const wrapper = createWrapper();
      expect(wrapper.text()).toContain("Confirm Action");
    });

    it("renders title", () => {
      const wrapper = createWrapper();
      expect(wrapper.text()).toContain("Are you sure?");
    });

    it("renders message text", () => {
      const wrapper = createWrapper();
      expect(wrapper.text()).toContain("This will delete the item.");
    });

    it("renders confirm button with custom text", () => {
      const wrapper = createWrapper();
      const buttons = wrapper.findAll("button");
      expect(buttons.some((b) => b.text() === "Delete")).toBe(true);
    });

    it("renders cancel button with custom text", () => {
      const wrapper = createWrapper();
      const buttons = wrapper.findAll("button");
      expect(buttons.some((b) => b.text() === "Cancel")).toBe(true);
    });

    it("renders optional icon with correct class", () => {
      const wrapper = createWrapper({
        icon: "i-ri-delete-bin-line",
        iconDanger: true,
      });
      const icon = wrapper.find(".dialog-heading__icon");
      expect(icon.exists()).toBe(true);
      expect(icon.classes()).toContain("i-ri-delete-bin-line");
      expect(icon.classes()).toContain("dialog-heading__icon--danger");
    });

    it("does not render icon when icon prop is not provided", () => {
      const wrapper = createWrapper();
      expect(wrapper.find(".dialog-heading__icon").exists()).toBe(false);
    });
  });

  describe("Visibility", () => {
    it("dialog is visible when modelValue is true", () => {
      const wrapper = createWrapper({ modelValue: true });
      expect(wrapper.find(".ui-dialog-stub").exists()).toBe(true);
    });

    it("dialog is hidden when modelValue is false", () => {
      const wrapper = createWrapper({ modelValue: false });
      expect(wrapper.find(".ui-dialog-stub").exists()).toBe(false);
    });
  });

  describe("Events", () => {
    it("clicking confirm button emits confirm event", async () => {
      const wrapper = createWrapper();
      const confirmButton = wrapper.findAll("button").find((b) => b.text() === "Delete")!;
      await confirmButton.trigger("click");
      expect(wrapper.emitted("confirm")).toHaveLength(1);
    });

    it("clicking cancel button emits cancel event", async () => {
      const wrapper = createWrapper();
      const cancelButton = wrapper.findAll("button").find((b) => b.text() === "Cancel")!;
      await cancelButton.trigger("click");
      expect(wrapper.emitted("cancel")).toHaveLength(1);
    });

    it("cancel also emits update:modelValue false", async () => {
      const wrapper = createWrapper();
      const cancelButton = wrapper.findAll("button").find((b) => b.text() === "Cancel")!;
      await cancelButton.trigger("click");
      expect(wrapper.emitted("update:modelValue")).toBeTruthy();
      expect(wrapper.emitted("update:modelValue")![0]).toEqual([false]);
    });
  });

  describe("States", () => {
    it("confirmLoading disables confirm button", () => {
      const wrapper = createWrapper({ confirmLoading: true });
      const confirmButton = wrapper.findAll("button").find((b) => b.text() === "Delete")!;
      // UiButton maps loading → disabled attribute on native <button>
      expect(confirmButton.attributes("disabled")).toBeDefined();
    });

    it("confirmDisabled disables confirm button", () => {
      const wrapper = createWrapper({ confirmDisabled: true });
      const confirmButton = wrapper.findAll("button").find((b) => b.text() === "Delete")!;
      expect(confirmButton.attributes("disabled")).toBeDefined();
    });

    it("cancelDisabled disables cancel button", () => {
      const wrapper = createWrapper({ cancelDisabled: true });
      const cancelButton = wrapper.findAll("button").find((b) => b.text() === "Cancel")!;
      expect(cancelButton.attributes("disabled")).toBeDefined();
    });
  });

  describe("Props", () => {
    it("confirmVariant changes confirm button variant class", () => {
      const wrapper = createWrapper({ confirmVariant: "primary" });
      const confirmButton = wrapper.findAll("button").find((b) => b.text() === "Delete")!;
      expect(confirmButton.classes()).toContain("ui-button--primary");
    });

    it('default confirmVariant is "danger"', () => {
      const wrapper = createWrapper();
      const confirmButton = wrapper.findAll("button").find((b) => b.text() === "Delete")!;
      expect(confirmButton.classes()).toContain("ui-button--danger");
    });

    it("width prop passes to dialog", () => {
      const wrapper = createWrapper({ width: "500px" });
      const dialog = wrapper.find(".ui-dialog-stub");
      expect(dialog.attributes("style")).toContain("500px");
    });
  });
});
