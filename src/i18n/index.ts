import i18next from "i18next";
import { computed, readonly, ref } from "vue";

import { resources, supportedLanguages, type SupportedLanguage } from "./resources";

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

export function t(key: string, options?: Record<string, unknown>) {
  return i18next.t(key, { lng: currentLanguage.value, ...options });
}

export async function setLanguage(language: SupportedLanguage) {
  if (language === currentLanguage.value) {
    return;
  }

  await i18next.changeLanguage(language);
  currentLanguage.value = language;
  localStorage.setItem(storageKey, language);
  document.documentElement.lang = language;
}

document.documentElement.lang = currentLanguage.value;

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
