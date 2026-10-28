// Builds a platform-specific .vsix with the phplumb binary for that platform and,
// beside it in bin/stubs, the phpstorm stubs the engine reads built-in symbols from.
//
//   npm run package -- [darwin-arm64 | linux-x64]
//
// darwin-arm64 (the default on an Apple silicon Mac) builds with the host
// toolchain. linux-x64 builds a static binary in Docker on any host.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, cpSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const extensionDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(extensionDir, "..", "..");
const host = `${process.platform}-${process.arch}`;

const builds = {
  "darwin-arm64": () => {
    if (host !== "darwin-arm64") {
      throw new Error(`darwin-arm64 packages build on an Apple silicon Mac, not on ${host}`);
    }
    run("cargo", ["build", "--release", "--locked", "-p", "phplumb-server"], repoRoot);
    return path.join(repoRoot, "target", "release", "phplumb");
  },
  "linux-x64": () => {
    const out = path.join(repoRoot, "target", "linux-x64");
    run(
      "docker",
      [
        "build",
        "--platform",
        "linux/amd64",
        "--target",
        "artifact",
        "--output",
        `type=local,dest=${out}`,
        "--file",
        path.join(extensionDir, "scripts", "linux-x64.Dockerfile"),
        repoRoot,
      ],
      repoRoot,
    );
    return path.join(out, "phplumb");
  },
};

function run(command, args, cwd) {
  execFileSync(command, args, { cwd, stdio: "inherit" });
}

const target = process.argv[2] ?? host;
const build = builds[target];
if (build === undefined) {
  console.error(`Unknown target ${target}. Targets: ${Object.keys(builds).join(", ")}`);
  process.exit(1);
}

const binary = build();
const bin = path.join(extensionDir, "bin");
mkdirSync(bin, { recursive: true });
copyFileSync(binary, path.join(bin, "phplumb"));
chmodSync(path.join(bin, "phplumb"), 0o755);
const stubs = path.join(bin, "stubs");
rmSync(stubs, { recursive: true, force: true });
cpSync(path.join(repoRoot, "crates", "phplumb-index", "data", "stubs"), stubs, { recursive: true });

const { version } = JSON.parse(readFileSync(path.join(extensionDir, "package.json"), "utf8"));
const vsix = `phplumb-${target}-${version}.vsix`;
run("npm", ["run", "build"], extensionDir);
run(
  "npx",
  ["--no-install", "vsce", "package", "--target", target, "--no-dependencies", "--allow-missing-repository", "--skip-license", "--out", vsix],
  extensionDir,
);
console.log(`Packaged ${path.join(extensionDir, vsix)}`);
