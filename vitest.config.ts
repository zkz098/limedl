import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "#invoke": fileURLToPath(new URL("./src/lib/ws/ws-invoke.ts", import.meta.url)),
      "#event": fileURLToPath(new URL("./src/lib/ws/ws-event.ts", import.meta.url)),
      "@tauri-apps/plugin-notification": fileURLToPath(
        new URL("./src/lib/ws/ws-notification-mock.ts", import.meta.url),
      ),
    },
  },
  test: {
    environment: "jsdom",
    exclude: ["node_modules/", "e2e/", ".opencode/", "**/node_modules/**"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      exclude: ["node_modules/", "src/__tests__/", "**/*.d.ts"],
    },
  },
});
