export type ProxyMode = "disabled" | "system" | "manual";

export interface ProxySettings {
  mode: ProxyMode;
  manualUrl: string;
}
