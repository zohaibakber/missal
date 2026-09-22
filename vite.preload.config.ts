import path from "node:path";
import { defineConfig } from "vite";

// Forge's Vite plugin emits CommonJS. With "type": "module" the output
// must use a .cjs extension so Electron does not load it as ESM.
export default defineConfig({
  plugins: [
    {
      // Forge's preload defaults still set Rollup's `inlineDynamicImports`, which Rolldown has
      // replaced with `codeSplitting: false` (set below); drop it so builds don't warn.
      name: "missal:drop-forge-inline-dynamic-imports",
      config(config) {
        const output = config.build?.rollupOptions?.output;
        for (const entry of Array.isArray(output) ? output : output ? [output] : []) {
          delete entry.inlineDynamicImports;
        }
      },
    },
  ],
  resolve: {
    alias: {
      "#": path.resolve(__dirname, "src"),
    },
  },
  build: {
    rollupOptions: {
      output: {
        format: "cjs",
        codeSplitting: false,
        entryFileNames: "[name].cjs",
        chunkFileNames: "[name].cjs",
        assetFileNames: "[name].[ext]",
      },
    },
  },
});
