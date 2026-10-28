import * as assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";

import { DOCUMENT_SELECTOR } from "../extension";

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

  test("contributes the Drupal activation setting", () => {
    const manifest = JSON.parse(fs.readFileSync(path.join(extensionDir, "package.json"), "utf8"));
    const setting = manifest.contributes.configuration.properties["phplumb.frameworks.drupal"];

    assert.deepStrictEqual([setting?.enum, setting?.default], [["auto", "on", "off"], "auto"]);
  });

  test("contributes the Symfony activation setting", () => {
    const manifest = JSON.parse(fs.readFileSync(path.join(extensionDir, "package.json"), "utf8"));
    const setting = manifest.contributes.configuration.properties["phplumb.frameworks.symfony"];

    assert.deepStrictEqual([setting?.enum, setting?.default], [["auto", "on", "off"], "auto"]);
  });

  test("sends Twig files to the engine", () => {
    assert.ok(
      DOCUMENT_SELECTOR.some((filter) => filter.pattern === "**/*.twig"),
      JSON.stringify(DOCUMENT_SELECTOR),
    );
  });

  test("sends Doctrine mappings to the engine", () => {
    assert.ok(
      DOCUMENT_SELECTOR.some((filter) => filter.pattern === "**/*.orm.xml"),
      JSON.stringify(DOCUMENT_SELECTOR),
    );
  });

  test("contributes the project folder setting", () => {
    const manifest = JSON.parse(fs.readFileSync(path.join(extensionDir, "package.json"), "utf8"));
    const setting = manifest.contributes.configuration.properties["phplumb.root"];

    assert.deepStrictEqual([setting?.type, setting?.default], ["string", ""]);
  });
});
