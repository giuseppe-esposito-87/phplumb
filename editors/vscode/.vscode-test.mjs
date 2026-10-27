import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@vscode/test-cli";

const here = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  files: "out/test/**/*.test.js",
  // Insiders runs alongside the user's open stable VS Code.
  version: "insiders",
  workspaceFolder: "./test-fixtures/basic",
  // macOS caps socket paths at 104 bytes; the default user data dir under
  // .vscode-test makes the main socket path longer than that.
  launchArgs: ["--disable-extensions", `--user-data-dir=${path.join(os.tmpdir(), "phplumb-vscode-test")}`],
  env: {
    PHPLUMB_SERVER_PATH: path.resolve(here, "../../target/debug/phplumb"),
  },
  mocha: {
    ui: "tdd",
    timeout: 20000,
  },
});
