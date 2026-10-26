import * as path from "node:path";

/**
 * Finds the php-navigator binary. Order: the configured setting, the
 * PHPNAV_SERVER_PATH environment variable, the binary bundled in the extension,
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
  const bundled = path.join(extensionPath, "bin", "php-navigator");
  const debugBuild = path.join(extensionPath, "..", "..", "target", "debug", "php-navigator");
  const candidates = [
    configured,
    env.PHPNAV_SERVER_PATH ?? "",
    ...(development ? [debugBuild, bundled] : [bundled, debugBuild]),
  ].filter((candidate) => candidate !== "");
  const found = candidates.find(exists);
  if (found === undefined) {
    throw new Error(`php-navigator binary not found. Tried: ${candidates.join(", ")}`);
  }
  return found;
}
