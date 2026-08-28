import zhCN from "./zh-CN";
import enUS from "./en-US";

export const supportedLanguages = ["zh-CN", "en-US"] as const;
export type SupportedLanguage = (typeof supportedLanguages)[number];

export const resources = {
  "zh-CN": zhCN,
  "en-US": enUS,
} as const;

export type TranslationResources = typeof zhCN;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: {
      translation: typeof zhCN.translation;
    };
    returnNull: false;
  }
}
