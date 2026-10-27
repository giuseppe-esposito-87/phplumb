import * as assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";

// Compiled to out/test, two levels below the extension folder.
const extensionDir = path.resolve(__dirname, "..", "..");

suite("manifest", () => {
  test("activates in a workspace with PHP files before any is opened", () => {
    const manifest = JSON.parse(fs.readFileSync(path.join(extensionDir, "package.json"), "utf8"));

    assert.ok(manifest.activationEvents.includes("workspaceContains:**/*.php"), JSON.stringify(manifest.activationEvents));
  });

  test("the Linux build uses no unpinned Dockerfile frontend", () => {
    const dockerfile = fs.readFileSync(path.join(extensionDir, "scripts", "linux-x64.Dockerfile"), "utf8");

    assert.ok(!/^#\s*syntax=(?!\S*@sha256:)/m.test(dockerfile), "unpinned # syntax= directive");
  });
});
