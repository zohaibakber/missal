/** @vitest-environment jsdom */
import { afterEach, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { RegistryProvider } from "@effect/atom-react";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { Effect } from "effect";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { GlobalPlaceholderForm } from "#/components/global-placeholder-form";
import { TooltipProvider } from "#/components/ui/tooltip";
import { makeElectronMainRuntime } from "#/electron/main-runtime";
import { dispatchStorageRequest } from "#/electron/storage-dispatch";
import { PlaceholderRepository } from "#/repositories/index";

afterEach(() => {
  cleanup();
  delete window.electronStorage;
});

it("adds and saves a global name and value through the real storage bridge", async () => {
  const directory = mkdtempSync(join(tmpdir(), "missal-global-form-"));
  const runtime = makeElectronMainRuntime({
    databasePath: join(directory, "test.sqlite"),
    migrationsFolder: resolve("drizzle"),
  });
  window.electronStorage = {
    request: (payload) => runtime.runPromise(dispatchStorageRequest(payload)),
  };
  const rootRoute = createRootRoute({ component: GlobalPlaceholderForm });
  const router = createRouter({
    routeTree: rootRoute,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  try {
    await router.load();
    render(
      <RegistryProvider>
        <TooltipProvider>
          <RouterProvider router={router} />
        </TooltipProvider>
      </RegistryProvider>,
    );
    await screen.findByLabelText("Global value for تفتیشی افسر");
    fireEvent.click(screen.getByRole("button", { name: "Add global placeholder" }));
    fireEvent.change(screen.getByLabelText("Placeholder name 6"), {
      target: { value: "دفتر کا پتہ" },
    });
    fireEvent.change(screen.getByLabelText("Global value for دفتر کا پتہ"), {
      target: { value: "لاہور" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Save changes" }).hasAttribute("disabled")).toBe(
        true,
      ),
    );
    const saved = await runtime.runPromise(
      Effect.flatMap(PlaceholderRepository, (repo) => repo.listGlobals),
    );
    expect(saved.find((row) => row.label === "دفتر کا پتہ")?.value).toBe("لاہور");
    expect(screen.getByRole("button", { name: "Copy placeholder for دفتر کا پتہ" })).toBeTruthy();
  } finally {
    cleanup();
    await runtime.dispose();
    rmSync(directory, { recursive: true, force: true });
  }
});
