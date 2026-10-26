import * as path from "node:path";

/**
 * Finds the php-navigator binary. Order: the configured setting, the
 * PHPNAV_SERVER_PATH environment variable, the binary bundled in the extension,
 * then the debug build of the repository the extension runs from.
 * Throws an Error listing every path tried when none exists.
 */
export function resolveServerPath(
  extensionPath: string,
  configured: string,
  env: NodeJS.ProcessEnv,
  exists: (p: string) => boolean,
): string {
  const candidates = [
    configured,
    env.PHPNAV_SERVER_PATH ?? "",
    path.join(extensionPath, "bin", "php-navigator"),
    path.join(extensionPath, "..", "..", "target", "debug", "php-navigator"),
  ].filter((candidate) => candidate !== "");
  const found = candidates.find(exists);
  if (found === undefined) {
    throw new Error(`php-navigator binary not found. Tried: ${candidates.join(", ")}`);
  }
  return found;
}
