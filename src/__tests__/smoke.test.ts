import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";

const TestComponent = defineComponent({
  template: "<div>Hello</div>",
});

describe("smoke", () => {
  it("mounts a minimal Vue component", () => {
    const wrapper = mount(TestComponent);
    expect(wrapper.text()).toBe("Hello");
  });
});
