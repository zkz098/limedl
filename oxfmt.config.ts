import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: [
    "dist/**",
    ".playwright-mcp/**",
    ".sisyphus/**",
    "out/**",
    "node_modules",
    "*.mdx",
    // Machine-generated ts-rs / WS manifests: `check-rust` regenerates them and
    // requires a clean `git diff`, so they must stay exactly as the generator
    // wrote them (formatting them here desynchronises the freshness gate).
    "src/types/generated/**",
    "src/lib/ws/generated/**",
  ],
});
