import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive, nextTick } from "vue";
import SettingsAppearancePanel from "../../../components/settings/SettingsAppearancePanel.vue";

// ── Mocks ──────────────────────────────────────────────────────────

const { mockEnable, mockDisable } = vi.hoisted(() => ({
  mockEnable: vi.fn().mockResolvedValue(undefined),
  mockDisable: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: mockEnable,
  disable: mockDisable,
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  SettingsField: {
    template:
      '<div class="field-stub"><slot /><span v-if="label" class="label">{{ label }}</span></div>',
    props: ["label", "hint", "infoTooltip", "wide"],
  },
  SettingsSection: {
    template: '<div class="section-stub"><slot /></div>',
    props: ["title", "icon", "summary"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option></select>',
    props: ["modelValue", "options"],
  },
  UiSwitch: {
    template:
      '<input type="checkbox" class="ui-switch-stub" :checked="modelValue" @change="$emit(\'update:modelValue\', $event.target.checked)" />',
    props: ["modelValue"],
  },
  UiButton: {
    template:
      '<button class="ui-button-stub" @click="$emit(\'click\')"><slot /></button>',
    props: ["loading"],
  },
  InfoTooltip: {
    template: '<span class="info-tooltip-stub" />',
    props: ["text"],
  },
  Teleport: false,
};

// ── Options ────────────────────────────────────────────────────────

const languageOptions = [
  { label: "English", value: "en-US" },
  { label: "中文", value: "zh-CN" },
];

const colorModeOptions = [
  { label: "System", value: "system" },
  { label: "Light", value: "light" },
  { label: "Dark", value: "dark" },
];

const backgroundOpacityOptions = [
  { label: "100%", value: "100" },
  { label: "Acrylic", value: "acrylic" },
  { label: "Frosted", value: "frosted" },
];

// ── Fixture ────────────────────────────────────────────────────────

function createDraft() {
  return reactive({
    appearance: {
      colorMode: "system",
      themeColor: "amber",
      backgroundOpacity: "100",
      showDetailInfo: true,
      closeBehavior: "minimizeToTray",
    },
    notifications: { enabled: true },
    autostart: false,
  });
}

// ── Helpers ────────────────────────────────────────────────────────

function mountPanel(props: Record<string, unknown> = {}) {
  const draft = createDraft();
  return {
    draft,
    wrapper: mount(SettingsAppearancePanel, {
      props: {
        draft,
        t: (key: string) => key,
        language: "en-US",
        languageOptions,
        colorModeOptions,
        backgroundOpacityOptions,
        ...props,
      },
      global: { stubs },
    }),
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsAppearancePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── 1. Language ──────────────────────────────────────────────────

  it("renders language select with model-value bound to language prop (not draft)", () => {
    const draft = createDraft();
    // Set language to zh-CN via prop — draft doesn't have a language field
    const { wrapper } = mountPanel({ language: "zh-CN", draft });
    const selects = wrapper.findAll("select.ui-select-stub");
    expect(selects.length).toBeGreaterThanOrEqual(1);

    // The first select is the language selector
    const langSelect = selects[0];
    expect((langSelect.element as HTMLSelectElement).value).toBe("zh-CN");

    // Changing the prop re-renders — draft.language doesn't exist
    expect((draft as Record<string, unknown>).language).toBeUndefined();
  });

  it("emits changeLanguage when language select changes", async () => {
    const { wrapper } = mountPanel();
    const selects = wrapper.findAll("select.ui-select-stub");
    const langSelect = selects[0];

    await langSelect.setValue("zh-CN");
    await nextTick();

    expect(wrapper.emitted("changeLanguage")).toBeTruthy();
    expect(wrapper.emitted("changeLanguage")![0][0]).toBe("zh-CN");
  });

  // ── 2. Appearance: color mode ────────────────────────────────────

  it("renders color mode select with v-model bound to draft.appearance.colorMode", async () => {
    const { draft, wrapper } = mountPanel();
    const selects = wrapper.findAll("select.ui-select-stub");
    const colorSelect = selects[1];

    // Initial value is bound from draft
    expect((colorSelect.element as HTMLSelectElement).value).toBe("system");

    // Changing select updates draft directly (v-model)
    await colorSelect.setValue("dark");
    await nextTick();
    expect(draft.appearance.colorMode).toBe("dark");
  });

  // ── 3. Theme color buttons ───────────────────────────────────────

  it("renders three theme color buttons (amber, sky, lime)", () => {
    const { wrapper } = mountPanel();
    const buttons = wrapper.findAll("button.theme-color-button");
    expect(buttons.length).toBe(3);

    const labels = buttons.map((b) => b.attributes("aria-label"));
    expect(labels).toContain("settings.themeColorNames.amber");
    expect(labels).toContain("settings.themeColorNames.sky");
    expect(labels).toContain("settings.themeColorNames.lime");
  });

  it("clicking a theme color button sets draft.appearance.themeColor", async () => {
    const { draft, wrapper } = mountPanel();
    expect(draft.appearance.themeColor).toBe("amber");

    const skyBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.sky"]',
    );
    await skyBtn.trigger("click");
    await nextTick();

    expect(draft.appearance.themeColor).toBe("sky");
  });

  it("active theme button has is-active class, inactive ones don't", async () => {
    const { draft, wrapper } = mountPanel();
    expect(draft.appearance.themeColor).toBe("amber");

    const amberBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.amber"]',
    );
    const skyBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.sky"]',
    );
    const limeBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.lime"]',
    );

    // Amber is active initially
    expect(amberBtn.classes()).toContain("is-active");
    expect(skyBtn.classes()).not.toContain("is-active");
    expect(limeBtn.classes()).not.toContain("is-active");

    // Click sky
    await skyBtn.trigger("click");
    await nextTick();

    expect(amberBtn.classes()).not.toContain("is-active");
    expect(skyBtn.classes()).toContain("is-active");
    expect(limeBtn.classes()).not.toContain("is-active");

    // Click lime
    await limeBtn.trigger("click");
    await nextTick();

    expect(amberBtn.classes()).not.toContain("is-active");
    expect(skyBtn.classes()).not.toContain("is-active");
    expect(limeBtn.classes()).toContain("is-active");
  });

  it("only active theme button shows check icon (i-ri-check-line)", async () => {
    const { draft, wrapper } = mountPanel();
    expect(draft.appearance.themeColor).toBe("amber");

    const amberBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.amber"]',
    );
    const skyBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.sky"]',
    );
    const limeBtn = wrapper.find(
      'button[aria-label="settings.themeColorNames.lime"]',
    );

    // Only amber has check
    expect(amberBtn.find(".i-ri-check-line").exists()).toBe(true);
    expect(skyBtn.find(".i-ri-check-line").exists()).toBe(false);
    expect(limeBtn.find(".i-ri-check-line").exists()).toBe(false);

    // Switch to sky
    await skyBtn.trigger("click");
    await nextTick();

    expect(amberBtn.find(".i-ri-check-line").exists()).toBe(false);
    expect(skyBtn.find(".i-ri-check-line").exists()).toBe(true);
    expect(limeBtn.find(".i-ri-check-line").exists()).toBe(false);
  });

  // ── 4. Background opacity ────────────────────────────────────────

  it("renders background opacity select with v-model", async () => {
    const { draft, wrapper } = mountPanel();
    const selects = wrapper.findAll("select.ui-select-stub");
    const bgSelect = selects[2];

    // Initial value
    expect((bgSelect.element as HTMLSelectElement).value).toBe("100");

    // Change via v-model
    await bgSelect.setValue("acrylic");
    await nextTick();
    expect(draft.appearance.backgroundOpacity).toBe("acrylic");
  });

  // ── 5. Info panel ────────────────────────────────────────────────

  it("renders detail info switch with v-model on draft.appearance.showDetailInfo", async () => {
    const { draft, wrapper } = mountPanel();
    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");

    // Detail info is the first switch
    const detailSwitch = switches[0];
    expect((detailSwitch.element as HTMLInputElement).checked).toBe(
      draft.appearance.showDetailInfo,
    );

    // Toggle off
    await detailSwitch.setValue(false);
    await nextTick();
    expect(draft.appearance.showDetailInfo).toBe(false);

    // Toggle back on
    await detailSwitch.setValue(true);
    await nextTick();
    expect(draft.appearance.showDetailInfo).toBe(true);
  });

  // ── 6. Notifications ─────────────────────────────────────────────

  it("renders notifications switch with v-model on draft.notifications.enabled", async () => {
    const { draft, wrapper } = mountPanel();
    // Ensure notifications section is rendered
    expect(wrapper.text()).toContain("settings.notificationSettings.toggleLabel");

    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");
    const notifSwitch = switches[1];
    expect((notifSwitch.element as HTMLInputElement).checked).toBe(
      draft.notifications.enabled,
    );

    // Toggle off
    await notifSwitch.setValue(false);
    await nextTick();
    expect(draft.notifications.enabled).toBe(false);

    // Toggle back on
    await notifSwitch.setValue(true);
    await nextTick();
    expect(draft.notifications.enabled).toBe(true);
  });

  // ── 7. Autostart ─────────────────────────────────────────────────

  it("renders autostart switch (not v-model, uses onAutostartChange)", async () => {
    const { draft, wrapper } = mountPanel();
    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");
    const autoSwitch = switches[2];

    // The switch uses :model-value — it shows current draft value
    expect((autoSwitch.element as HTMLInputElement).checked).toBe(
      draft.autostart,
    );

    // Since it's NOT v-model, toggling the switch via the emitted event
    // does NOT update draft.autostart directly. The handler (onAutostartChange)
    // only calls the plugin API, it does not modify draft.
    await autoSwitch.setValue(true);
    await nextTick();
    // draft.autostart remains false because the handler doesn't touch it
    expect(draft.autostart).toBe(false);
  });

  it("toggling autostart ON calls enable() from plugin-autostart", async () => {
    const { wrapper } = mountPanel();
    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");
    const autoSwitch = switches[2];

    await autoSwitch.setValue(true);
    await nextTick();

    expect(mockEnable).toHaveBeenCalledOnce();
    expect(mockDisable).not.toHaveBeenCalled();
  });

  it("toggling autostart OFF calls disable() from plugin-autostart", async () => {
    const { wrapper } = mountPanel();
    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");
    const autoSwitch = switches[2];

    // First turn on, then turn off
    await autoSwitch.setValue(true);
    await nextTick();
    vi.clearAllMocks();

    await autoSwitch.setValue(false);
    await nextTick();

    expect(mockDisable).toHaveBeenCalledOnce();
    expect(mockEnable).not.toHaveBeenCalled();
  });

  it("autostart error is silently caught (plugin throws, no crash)", async () => {
    mockEnable.mockRejectedValueOnce(new Error("Permission denied"));
    const { wrapper } = mountPanel();
    const switches = wrapper.findAll("input.ui-switch-stub[type='checkbox']");
    const autoSwitch = switches[2];

    // Should not throw despite the rejected promise
    await expect(autoSwitch.setValue(true)).resolves.toBeUndefined();
    await nextTick();

    expect(mockEnable).toHaveBeenCalledOnce();
  });

  // ── 8. Close behavior ────────────────────────────────────────────

  it("renders close behavior select with computed options", () => {
    const { wrapper } = mountPanel();
    const selects = wrapper.findAll("select.ui-select-stub");
    const closeSelect = selects[3];

    // Options should be rendered
    const options = closeSelect.findAll("option");
    expect(options.length).toBe(2);

    // Initial value from v-model
    expect((closeSelect.element as HTMLSelectElement).value).toBe(
      "minimizeToTray",
    );
  });

  it("closeBehaviorOptions includes minimizeToTray and exit with translated labels", () => {
    const { wrapper } = mountPanel();

    const minimizeOption = wrapper.find('option[value="minimizeToTray"]');
    expect(minimizeOption.exists()).toBe(true);
    expect(minimizeOption.text()).toBe("settings.closeBehaviorMinimizeToTray");

    const exitOption = wrapper.find('option[value="exit"]');
    expect(exitOption.exists()).toBe(true);
    expect(exitOption.text()).toBe("settings.closeBehaviorExit");
  });
});
