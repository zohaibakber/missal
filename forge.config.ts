import { MakerSquirrel } from "@electron-forge/maker-squirrel";
import { MakerZIP } from "@electron-forge/maker-zip";
import { FusesPlugin } from "@electron-forge/plugin-fuses";
import { VitePlugin } from "@electron-forge/plugin-vite";
import { PublisherGithub } from "@electron-forge/publisher-github";
import type { ForgeConfig } from "@electron-forge/shared-types";
import { FuseV1Options, FuseVersion } from "@electron/fuses";
import { readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
const packRoot = path.join(root, "bundled-templates");
const packEntries = readdirSync(packRoot).sort();
if (packEntries.join() !== ["bodies", "manifest.json"].join()) {
  throw new Error(
    `bundled-templates must ship only manifest.json and bodies/, found ${packEntries.join(", ")}`,
  );
}
for (const file of readdirSync(path.join(packRoot, "bodies"))) {
  if (!file.endsWith(".json.gz")) {
    throw new Error(`Unexpected template body ${file}`);
  }
}

const config: ForgeConfig = {
  packagerConfig: {
    asar: true,
    // Packager appends the platform extension (`assets/icon.ico` on Windows).
    icon: "assets/icon",
    extraResource: ["drizzle", "bundled-templates"],
  },
  rebuildConfig: {},
  makers: [
    new MakerSquirrel({ name: "missal", setupIcon: "assets/icon.ico" }),
    new MakerZIP({}, ["win32"]),
  ],
  publishers: [
    new PublisherGithub({
      repository: { owner: "zohaibakber", name: "missal" },
      draft: true,
    }),
  ],
  plugins: [
    new VitePlugin({
      build: [
        {
          entry: "src/main.ts",
          config: "vite.main.config.ts",
          target: "main",
        },
        {
          entry: "src/preload.ts",
          config: "vite.preload.config.ts",
          target: "preload",
        },
      ],
      renderer: [
        {
          name: "main_window",
          config: "vite.renderer.config.ts",
        },
      ],
    }),
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
    }),
  ],
};

export default config;
