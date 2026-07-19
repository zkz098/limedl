import { defineConfig } from "oxlint";

export default defineConfig({
  categories: {
    correctness: "error",
    suspicious: "error",
    perf: "warn",
  },
  rules: {
    "no-underscore-dangle": ["error", { allow: ["__TEST_FILE_SERVER__", "_port"] }],
  },
  overrides: [
    {
      files: ["e2e/**/*.ts"],
      rules: {
        "no-await-in-loop": "off",
        "no-unsafe-type-assertion": "off",
      },
    },
  ],
});
