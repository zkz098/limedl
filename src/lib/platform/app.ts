export async function relaunchApp(): Promise<void> {
  if (typeof window !== "undefined") {
    window.location.reload();
  }
}

export async function exitApp(code = 0): Promise<void> {
  void code;
  if (typeof window !== "undefined") {
    window.close();
  }
}

export async function getPlatformOsVersion(): Promise<string> {
  return typeof navigator !== "undefined" ? navigator.userAgent : "web";
}
