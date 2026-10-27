import * as path from "node:path";

/**
 * Finds the phplumb binary. Order: the configured setting, the
 * PHPLUMB_SERVER_PATH environment variable, the binary bundled in the extension,
 * then the debug build of the repository the extension runs from. In
 * development mode the debug build comes before the bundled binary, so a binary
 * left behind by packaging never shadows the build under test.
 * Throws an Error listing every path tried when none exists.
 */
export function resolveServerPath(
  extensionPath: string,
  configured: string,
  env: NodeJS.ProcessEnv,
  exists: (p: string) => boolean,
  development = false,
): string {
  const bundled = path.join(extensionPath, "bin", "phplumb");
  const debugBuild = path.join(extensionPath, "..", "..", "target", "debug", "phplumb");
  const candidates = [
    configured,
    env.PHPLUMB_SERVER_PATH ?? "",
    ...(development ? [debugBuild, bundled] : [bundled, debugBuild]),
  ].filter((candidate) => candidate !== "");
  const found = candidates.find(exists);
  if (found === undefined) {
    throw new Error(`phplumb binary not found. Tried: ${candidates.join(", ")}`);
  }
  return found;
}
