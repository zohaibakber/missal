import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { cpus, freemem, platform, release, totalmem } from "node:os";
import path from "node:path";

export type EnvironmentMetadata = {
  readonly cpuModel: string;
  readonly cores: number;
  readonly totalRamBytes: number;
  readonly freeRamBytes: number;
  readonly os: string;
  readonly node: string;
  readonly electron: string | null;
  readonly electronPackage: string;
  readonly gitCommit: string;
  readonly patchHash: string;
  readonly note: string;
};

export function collectEnvironment(repoRoot: string): EnvironmentMetadata {
  const cpu = cpus();
  const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8")) as {
    devDependencies?: { electron?: string };
  };
  const patch = git(repoRoot, ["diff", "HEAD"]);
  return {
    cpuModel: cpu[0]?.model ?? "unknown",
    cores: cpu.length,
    totalRamBytes: totalmem(),
    freeRamBytes: freemem(),
    os: `${platform()} ${release()}`,
    node: process.version,
    electron: process.versions.electron ?? null,
    electronPackage: packageJson.devDependencies?.electron ?? "unknown",
    gitCommit: git(repoRoot, ["rev-parse", "HEAD"]),
    patchHash: createHash("sha256").update(patch).digest("hex"),
    note: "Dev-machine sample. Not the Windows acceptance baseline.",
  };
}

function git(repoRoot: string, args: readonly string[]) {
  return execFileSync("git", args, { cwd: repoRoot, encoding: "utf8" }).trim();
}
