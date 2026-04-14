import { open } from "@tauri-apps/plugin-dialog";

export async function pickDirectory() {
  const result = await open({
    directory: true,
    multiple: false,
    title: "Choose destination folder",
  });

  if (typeof result === "string") {
    return result;
  }

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return null;
}
