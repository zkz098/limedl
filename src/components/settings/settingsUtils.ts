import type { AppSettings, NetworkSceneProfile } from "../../types/settings";

export function serializeSettings(settings: AppSettings) {
  return JSON.stringify(settings);
}

export function copySingleNetworkScene(
  source: { scenes: NetworkSceneProfile[]; currentSceneId: string },
  t: (key: string, options?: Record<string, unknown>) => string,
): NetworkSceneProfile {
  const selectedScene =
    source.scenes.find((scene) => scene.id === source.currentSceneId) ?? source.scenes[0];
  return {
    id: "default",
    name: t("settings.defaultScene"),
    learningEnabled: selectedScene?.learningEnabled ?? true,
    learnedMetrics: selectedScene?.learnedMetrics ? { ...selectedScene.learnedMetrics } : null,
    updatedAtMs: selectedScene?.updatedAtMs ?? 0,
  };
}

export function settingsDraftSnapshot(settings: AppSettings) {
  return serializeSettings(settings);
}
