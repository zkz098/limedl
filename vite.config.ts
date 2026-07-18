import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import UnoCSS from "unocss/vite";
import { fileURLToPath } from "node:url";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async ({ mode }) => {
  const isNas = mode === "nas";

  return {
    plugins: [vue(), UnoCSS()],
    clearScreen: false,
    resolve: {
      alias: isNas
        ? {
            "#invoke": fileURLToPath(new URL("./src/lib/ws/ws-invoke.ts", import.meta.url)),
            "#event": fileURLToPath(new URL("./src/lib/ws/ws-event.ts", import.meta.url)),
            "@tauri-apps/plugin-notification": fileURLToPath(
              new URL("./src/lib/ws/ws-notification-mock.ts", import.meta.url),
            ),
          }
        : {
            "#invoke": "@tauri-apps/api/core",
            "#event": "@tauri-apps/api/event",
          },
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
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
