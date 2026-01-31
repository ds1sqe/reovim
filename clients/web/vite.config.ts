import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
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
