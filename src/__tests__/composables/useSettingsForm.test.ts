import { ref } from "vue";
import { describe, it, expect } from "vitest";

import { useSettingsForm } from "../../components/settings/useSettingsForm";
import { DEFAULT_APP_SETTINGS } from "../../lib/app-settings-defaults";
import type { AppSettings } from "../../types/settings";

function makeSettings(): AppSettings {
  // Deep clone the canonical default so tests never mutate the shared constant.
  return structuredClone(DEFAULT_APP_SETTINGS);
}

describe("useSettingsForm — scheduler toggle sync", () => {
  it("syncs tailSprintEnabled/connectionWarmupEnabled from saved settings into the form", () => {
    const saved = makeSettings();
    saved.scheduler.tailSprintEnabled = true;
    saved.scheduler.connectionWarmupEnabled = true;

    const settings = ref<AppSettings | null>(saved);
    const { form, buildSettingsPayload } = useSettingsForm({ settings });

    // Stored values must appear in the form …
    expect(form.scheduler.tailSprintEnabled).toBe(true);
    expect(form.scheduler.connectionWarmupEnabled).toBe(true);

    // … and be carried back out in the save payload.
    const payload = buildSettingsPayload();
    expect(payload.scheduler.tailSprintEnabled).toBe(true);
    expect(payload.scheduler.connectionWarmupEnabled).toBe(true);
  });

  it("does not reset the toggles after a reload follows a save (the reported bug)", () => {
    const settings = ref<AppSettings | null>(makeSettings());
    const { form, buildSettingsPayload } = useSettingsForm({ settings });

    // User turns both toggles on and saves.
    form.scheduler.tailSprintEnabled = true;
    form.scheduler.connectionWarmupEnabled = true;
    const saved = buildSettingsPayload();

    // The app reloads the just-saved settings (backend → settings prop).
    settings.value = saved;

    // Toggles must still reflect the saved values instead of snapping back
    // to defaults.
    expect(form.scheduler.tailSprintEnabled).toBe(true);
    expect(form.scheduler.connectionWarmupEnabled).toBe(true);
    expect(buildSettingsPayload().scheduler.connectionWarmupEnabled).toBe(true);
  });

  it("copies every scheduler field between saved settings, form, and payload", () => {
    const settings = ref<AppSettings | null>(makeSettings());
    const { form, buildSettingsPayload } = useSettingsForm({ settings });

    // Whole-object structural comparison: if any scheduler field were ever
    // dropped from the sync or the payload, this fails. Because `makeSettings`
    // is built from the regenerated default object, any newly added scheduler
    // setting is covered automatically without needing a manual field list.
    expect(form.scheduler).toEqual(settings.value!.scheduler);
    expect(buildSettingsPayload().scheduler).toEqual(form.scheduler);
  });

  it("defaults connectionWarmupEnabled to true and tailSprintEnabled to false", () => {
    const settings = ref<AppSettings | null>(makeSettings());
    const { form } = useSettingsForm({ settings });

    expect(form.scheduler.connectionWarmupEnabled).toBe(
      DEFAULT_APP_SETTINGS.scheduler.connectionWarmupEnabled,
    );
    expect(form.scheduler.tailSprintEnabled).toBe(
      DEFAULT_APP_SETTINGS.scheduler.tailSprintEnabled,
    );
  });
});
