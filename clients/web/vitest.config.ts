import { defineConfig } from "vitest/config";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  test: {
    // Default environment for most tests
    environment: "node",
    // Use jsdom for tests that need DOM
    environmentMatchGlobs: [
      ["tests/render-*.test.ts", "jsdom"],
    ],
  },
  optimizeDeps: {
    exclude: ["reovim-client-model"],
  },
});
