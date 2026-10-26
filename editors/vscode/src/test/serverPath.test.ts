import * as assert from "node:assert";
import * as path from "node:path";
import { resolveServerPath } from "../serverPath";

const ext = "/ext/editors/vscode";
const bundled = path.join(ext, "bin", "php-navigator");
const dev = path.join(ext, "..", "..", "target", "debug", "php-navigator");

function existing(...paths: string[]): (p: string) => boolean {
  return (p) => paths.includes(p);
}

suite("resolveServerPath", () => {
  test("prefers the configured path", () => {
    assert.strictEqual(resolveServerPath(ext, "/a", { PHPNAV_SERVER_PATH: "/b" }, existing("/a", "/b", bundled)), "/a");
  });

  test("falls back to the env var", () => {
    assert.strictEqual(resolveServerPath(ext, "", { PHPNAV_SERVER_PATH: "/b" }, existing("/b", bundled)), "/b");
  });

  test("falls back to the bundled binary, then the dev build", () => {
    assert.strictEqual(resolveServerPath(ext, "", {}, existing(bundled, dev)), bundled);
    assert.strictEqual(resolveServerPath(ext, "", {}, existing(dev)), dev);
  });

  test("prefers the dev build in development mode", () => {
    assert.strictEqual(resolveServerPath(ext, "", {}, existing(bundled, dev), true), dev);
    assert.strictEqual(resolveServerPath(ext, "", {}, existing(bundled), true), bundled);
  });

  test("throws listing every path tried when none exists", () => {
    assert.throws(
      () => resolveServerPath(ext, "/a", { PHPNAV_SERVER_PATH: "/b" }, existing()),
      (err: Error) => ["/a", "/b", bundled, dev].every((p) => err.message.includes(p)),
    );
  });
});
