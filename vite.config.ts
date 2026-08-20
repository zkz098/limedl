import { defineConfig, type UserConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import UnoCSS from "unocss/vite";
import { fileURLToPath } from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async ({ mode }): Promise<UserConfig> => {
  const isNas = mode === "nas";

  const nasAlias = {
    "#invoke": fileURLToPath(new URL("./src/lib/ws/ws-invoke.ts", import.meta.url)),
    "#event": fileURLToPath(new URL("./src/lib/ws/ws-event.ts", import.meta.url)),
    "@tauri-apps/plugin-notification": fileURLToPath(
      new URL("./src/lib/ws/ws-notification-mock.ts", import.meta.url),
    ),
  };

  const tauriAlias = {
    "#invoke": "@tauri-apps/api/core",
    "#event": "@tauri-apps/api/event",
  };

  return {
    plugins: [vue(), ...UnoCSS()],
    clearScreen: false,
    build: {
      rollupOptions: {
        input: isNas
          ? { main: fileURLToPath(new URL("./index.html", import.meta.url)) }
          : {
              main: fileURLToPath(new URL("./index.html", import.meta.url)),
              pet: fileURLToPath(new URL("./pet.html", import.meta.url)),
            },
      },
    },
    resolve: {
      alias: isNas ? nasAlias : tauriAlias,
    },
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ["**/src-tauri/**", "**/target/**"],
      },
    },
  };
});
