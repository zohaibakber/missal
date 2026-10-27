import { Atom } from "effect/reactivity";
import { RendererRepositoriesLive } from "#/repositories/rpc";
import type { AppRepositories } from "#/repositories/index";

export const appRuntime: Atom.AtomRuntime<AppRepositories> = Atom.runtime(RendererRepositoriesLive);
