import { defineConfig, type UserConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import UnoCSS from "unocss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig(async (): Promise<UserConfig> => {
  // The Vue app is the browser front end for `limedl daemon` (NAS/desktop WebUI);
  // it talks to the server over WebSocket. See src/lib/ws/ws-invoke.ts.
  const wsAlias = {
    "#invoke": fileURLToPath(new URL("./src/lib/ws/ws-invoke.ts", import.meta.url)),
    "#event": fileURLToPath(new URL("./src/lib/ws/ws-event.ts", import.meta.url)),
  };

  return {
    plugins: [vue(), ...UnoCSS()],
    clearScreen: false,
    resolve: {
      alias: wsAlias,
    },
    server: {
      port: 1420,
      strictPort: true,
      watch: {
        ignored: ["**/target/**"],
      },
    },
  };
});
