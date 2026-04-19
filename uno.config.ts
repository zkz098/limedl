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
  theme: {
    fontFamily: {
      display: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
      body: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
    },
  },
});
