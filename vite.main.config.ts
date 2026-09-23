import path from "node:path";
import { defineConfig } from "vite";

// Forge's Vite plugin emits CommonJS. With "type": "module" the output
// must use a .cjs extension so Electron does not load it as ESM.
export default defineConfig({
  resolve: {
    alias: {
      "#": path.resolve(__dirname, "src"),
    },
  },
  build: {
    lib: {
      entry: { main: "src/main.ts", "storage-worker": "src/storage-worker.ts" },
      fileName: () => "[name].cjs",
      formats: ["cjs"],
    },
    rollupOptions: {
      output: { chunkFileNames: "[name]-[hash].cjs" },
    },
  },
});
