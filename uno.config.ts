import { defineConfig, presetIcons, presetUno } from "unocss";

export default defineConfig({
  presets: [
    presetUno(),
    presetIcons({
      scale: 1.05,
      extraProperties: {
        display: "inline-block",
        "vertical-align": "middle",
      },
    }),
  ],
  safelist: [
    // Setup wizard — step indicator icons (dynamically bound via :class="step.icon")
    "i-ri-home-smile-line",
    "i-ri-translate-2",
    "i-ri-flashlight-fill",
    "i-ri-server-line",
    "i-ri-speed-up-line",
    "i-ri-folder-download-line",
    "i-ri-palette-line",
    "i-ri-restart-line",
    "i-ri-file-list-3-line",
    "i-ri-check-line",
    // Setup wizard — navigation button icons (dynamically bound via :icon-right)
    "i-ri-arrow-right-s-line",
    "i-ri-arrow-left-s-line",
    // Setup wizard — summary edit button (bound via icon prop → runtime :class)
    "i-ri-edit-line",
    // Setup wizard — autostart step icon
    "i-ri-settings-3-line",
    // Settings About — GitHub link icon
    "i-ri-github-fill",
  ],
  theme: {
    fontFamily: {
      display: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
      body: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
    },
  },
});
