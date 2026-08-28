import i18next, { type ParseKeys } from "i18next";
import { computed, readonly, ref } from "vue";

import { updateTrayLanguage } from "../lib/tauri/app-api";

import {
  resources,
  supportedLanguages,
  type SupportedLanguage,
  type TranslationResources,
} from "./resources";

export type TranslationKey = ParseKeys;
export type { SupportedLanguage, TranslationResources };

const storageKey = "limedl.language";

function isSupportedLanguage(value: string): value is SupportedLanguage {
  return (supportedLanguages as readonly string[]).includes(value);
}

export function normalizeLanguage(value?: string | null): SupportedLanguage {
  if (value && isSupportedLanguage(value)) {
    return value;
  }

  const base = value?.split("-")[0];
  if (base === "zh") {
    return "zh-CN";
  }
  if (base === "en") {
    return "en-US";
  }

  return "zh-CN";
}

export function resolveInitialLanguage(): SupportedLanguage {
  const stored = localStorage.getItem(storageKey);
  if (stored) {
    return normalizeLanguage(stored);
  }

  return normalizeLanguage(navigator.language);
}

const currentLanguage = ref<SupportedLanguage>(resolveInitialLanguage());

void i18next.init({
  compatibilityJSON: "v4",
  fallbackLng: "zh-CN",
  interpolation: {
    escapeValue: false,
  },
  lng: currentLanguage.value,
  resources,
});

export const languageOptions = computed(() => [
  { label: i18next.t("language.zhCN", { lng: currentLanguage.value }), value: "zh-CN" },
  { label: i18next.t("language.enUS", { lng: currentLanguage.value }), value: "en-US" },
]);

export function t(key: TranslationKey | (string & {}), options?: Record<string, unknown>): string {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return (i18next.t as (k: string, opts?: Record<string, unknown>) => string)(key, {
    lng: currentLanguage.value,
    ...options,
  });
}

export async function setLanguage(language: SupportedLanguage) {
  if (language === currentLanguage.value) {
    return;
  }

  await i18next.changeLanguage(language);
  currentLanguage.value = language;
  localStorage.setItem(storageKey, language);
  document.documentElement.lang = language;

  // Update system tray menu language (desktop only — no-op in NAS/web mode)
  try {
    await updateTrayLanguage(language);
  } catch {
    // Tray update is non-critical — silently ignore
  }
}

document.documentElement.lang = currentLanguage.value;

// Update system tray menu on initial load (setLanguage only fires on change)
void (async () => {
  // NOSONAR: deliberate fire-and-forget — top-level await would block module init on WS connect
  try {
    await updateTrayLanguage(currentLanguage.value);
  } catch {
    // Tray update is non-critical — silently ignore
  }
})();

export function useI18n() {
  return {
    language: currentLanguage,
    languageOptions,
    setLanguage,
    supportedLanguages,
    t,
  };
}

export function useReadonlyLanguage() {
  return readonly(currentLanguage);
}
