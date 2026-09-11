import { Atom } from "effect/unstable/reactivity";
import { RendererRepositoriesLive } from "#/repositories/ipc";
import type { AppRepositories } from "#/repositories/index";

export const appRuntime: Atom.AtomRuntime<AppRepositories> = Atom.runtime(RendererRepositoriesLive);
