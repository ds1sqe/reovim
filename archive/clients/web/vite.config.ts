import { readFileSync } from "node:fs";

import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

const pkg = JSON.parse(readFileSync("./package.json", "utf-8"));

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  plugins: [wasm(), topLevelAwait()],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "ES2022",
  },
  optimizeDeps: {
    exclude: ["reovim-client-model"],
  },
});
