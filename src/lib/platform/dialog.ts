export interface OpenDialogOptions {
  directory?: boolean;
  multiple?: boolean;
  defaultPath?: string;
  title?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}

/**
 * File / directory picker.
 *
 * A browser cannot open a native filesystem picker without user-selected file
 * inputs, so the WebUI always returns `null`; callers fall back to a text field
 * where one exists. The desktop client picks natively in Rust via `rfd`.
 */
export async function openDialog(options?: OpenDialogOptions): Promise<string | string[] | null> {
  void options;
  return null;
}
