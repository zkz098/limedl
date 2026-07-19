import { open } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";

export async function pickDirectory() {
  const result = await open({
    directory: true,
    multiple: false,
    title: t("dialog.chooseFolder"),
  });

  if (typeof result === "string") {
    return result;
  }

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return null;
}

export async function pickTorrentFile() {
  const result = await open({
    directory: false,
    multiple: false,
    title: t("dialog.chooseTorrent"),
    filters: [{ name: "Torrent", extensions: ["torrent"] }],
  });

  if (typeof result === "string") {
    return result;
  }

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return null;
}
