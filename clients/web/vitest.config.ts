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
      ["tests/extensions*.test.ts", "jsdom"],
      ["tests/core-platform-adapter.test.ts", "jsdom"],
      ["tests/core-dom-surface.test.ts", "jsdom"],
      ["tests/core-chrome-dispatcher.test.ts", "jsdom"],
      // Integration tests run in Node (need child_process, etc.)
      ["tests/integration/**/*.test.ts", "node"],
    ],
    // Longer timeout for integration tests (server startup)
    testTimeout: 30000,
    // Hook timeout for server setup/teardown
    hookTimeout: 20000,
  },
  optimizeDeps: {
    exclude: ["reovim-client-model"],
  },
});
