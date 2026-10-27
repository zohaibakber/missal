import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

const electronBuild = (build: Record<string, unknown>) => ({
  resolve: { tsconfigPaths: true },
  build: { outDir: "dist-electron", sourcemap: true, ...build },
});

export default defineConfig(({ command, mode }) => ({
  base: "/",
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    ignorePatterns: ["src/routeTree.gen.ts"],
  },
  lint: {
    ignorePatterns: ["dist/**", "dist-electron/**", "release/**"],
    jsPlugins: ["@shadcn/lint"],
    options: { typeAware: true, typeCheck: true },
    rules: {
      "shadcn/no-restyle": [
        "error",
        {
          allow: ["layout"],
          contracts: [
            {
              pattern: "^InputGroup$",
              allow: ["layout", "bg-background", "shadow-none"],
            },
            { pattern: "^TableRow$", allow: ["layout", "hover:bg-transparent"] },
            { pattern: "^PopoverContent$", allow: ["layout", "p-0"] },
            { pattern: "^DialogContent$", allow: ["layout", "p-4"] },
            { pattern: "^Field(Group|Set)$", allow: ["layout", "spacing"] },
            { pattern: "^RadioGroup$", allow: ["layout", "spacing"] },
            { pattern: "^Tabs$", allow: ["layout", "spacing"] },
            { pattern: "^Sidebar$", allow: ["layout", "border-*"] },
            {
              pattern: "^Sidebar(GroupContent|Header|Menu|MenuItem)$",
              allow: ["layout", "spacing"],
            },
            { pattern: "^BreadcrumbPage$", allow: ["layout", "truncate"] },
          ],
        },
      ],
      "shadcn/no-raw-colors": "error",
      "shadcn/no-arbitrary-values": ["error", { allow: ["layout"] }],
      "shadcn/no-inline-styles": "error",
      "shadcn/no-unknown-classes": "error",
      "shadcn/require-static-classes": "error",
    },
    overrides: [
      {
        files: ["src/components/ui/**"],
        rules: {
          "shadcn/no-restyle": "off",
          "shadcn/no-arbitrary-values": "off",
          "shadcn/require-static-classes": "off",
        },
      },
    ],
  },
  resolve: { tsconfigPaths: true },
  plugins: lazyPlugins(async () => {
    const renderer = [
      tailwindcss(),
      tanstackRouter({
        target: "react",
        autoCodeSplitting: true,
      }),
      viteReact({ compiler: { logDiagnostics: true } }),
    ];
    if (mode === "test") return renderer;

    const { default: electron } = await import("vite-plugin-electron/simple");
    return [
      ...renderer,
      ...(await electron({
        main: {
          vite: electronBuild({
            emptyOutDir: command === "build",
            lib: {
              entry: { main: "src/main.ts", "storage-worker": "src/storage-worker.ts" },
              formats: ["cjs"],
              fileName: () => "[name].cjs",
            },
            rolldownOptions: {
              external: ["electron-updater"],
              output: { chunkFileNames: "[name]-[hash].cjs" },
            },
          }),
        },
        preload: {
          input: "src/preload.ts",
          vite: electronBuild({
            rolldownOptions: {
              output: { entryFileNames: "[name].cjs", chunkFileNames: "[name].cjs" },
            },
          }),
        },
      })),
    ];
  }),
}));
