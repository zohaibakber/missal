// Lets `node --experimental-strip-types` load Missal's `#/` imports from scripts.
import { register } from "node:module";
import { pathToFileURL } from "node:url";

register("./resolve-ts.mjs", pathToFileURL(import.meta.dirname + "/"));
