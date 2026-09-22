import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import { devtools } from "@tanstack/devtools-vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact, { reactCompilerPreset } from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    ignorePatterns: ["src/routeTree.gen.ts"],
  },
  lint: {
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
  plugins: [
    devtools(),
    tailwindcss(),
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
    }),
    viteReact(),
    babel({ presets: [reactCompilerPreset()] }),
  ],
});
