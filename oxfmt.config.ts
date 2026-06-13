import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: [
    "dist/**",
    ".playwright-mcp/**",
    ".sisyphus/**",
    "out/**",
    "node_modules",
    "*.mdx",
  ],
});
