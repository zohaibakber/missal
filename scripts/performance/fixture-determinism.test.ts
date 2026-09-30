import { expect, it } from "@effect/vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { generateFixture, repoRootFromHere } from "./fixture.ts";

it("builds the same small database hash from the same seed", async () => {
  const root = mkdtempSync(path.join(tmpdir(), "missal-perf-determinism-"));
  try {
    const first = await generateFixture({
      fixture: "small",
      seed: 1,
      outDir: path.join(root, "a"),
      repoRoot: repoRootFromHere(),
    });
    const second = await generateFixture({
      fixture: "small",
      seed: 1,
      outDir: path.join(root, "b"),
      repoRoot: repoRootFromHere(),
    });
    expect(first.sha256).toBe(second.sha256);
    expect(first.counts).toEqual(second.counts);
    expect(first.nodeCounts).toEqual(second.nodeCounts);
    expect(first.pageCounts).toEqual(second.pageCounts);
    expect(first.byteSizes.documents).toBe(second.byteSizes.documents);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}, 180_000);
