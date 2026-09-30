import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const sourceRoot = new URL("../../src/", import.meta.url);

export async function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith("#/")) {
    const name = specifier.slice(2).replace(/\.ts$/, "");
    return nextResolve(new URL(`${name}.ts`, sourceRoot).href, context);
  }
  if (
    (specifier.startsWith("./") || specifier.startsWith("../")) &&
    path.extname(specifier) === "" &&
    context.parentURL
  ) {
    const candidate = path.resolve(
      path.dirname(fileURLToPath(context.parentURL)),
      `${specifier}.ts`,
    );
    if (existsSync(candidate)) return nextResolve(pathToFileURL(candidate).href, context);
  }
  return nextResolve(specifier, context);
}
