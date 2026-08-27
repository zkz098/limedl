import { isTauri } from "./env";

export interface OpenDialogOptions {
  directory?: boolean;
  multiple?: boolean;
  defaultPath?: string;
  title?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}

export async function openDialog(options?: OpenDialogOptions): Promise<string | string[] | null> {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    return open(options);
  }
  return null;
}
