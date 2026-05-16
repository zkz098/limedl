import zhCN from "./zh-CN";
import enUS from "./en-US";

export const resources = {
  "zh-CN": zhCN,
  "en-US": enUS,
} as const;

export type SupportedLanguage = keyof typeof resources;

export const supportedLanguages = Object.keys(resources) as SupportedLanguage[];
