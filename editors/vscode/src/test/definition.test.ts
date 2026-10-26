import * as assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";

type Target = vscode.Location | vscode.LocationLink;

function uriOf(target: Target): vscode.Uri {
  return "targetUri" in target ? target.targetUri : target.uri;
}

function startOf(target: Target): vscode.Position {
  return "targetUri" in target ? (target.targetSelectionRange ?? target.targetRange).start : target.range.start;
}

function definitions(document: vscode.TextDocument, position: vscode.Position): Thenable<Target[]> {
  return vscode.commands.executeCommand<Target[]>("vscode.executeDefinitionProvider", document.uri, position);
}

/** The server indexes the workspace after it starts; polls until it answers. */
async function waitForDefinitions(document: vscode.TextDocument, position: vscode.Position): Promise<Target[]> {
  let targets: Target[] = [];
  const deadline = Date.now() + 10_000;
  while (targets.length === 0 && Date.now() < deadline) {
    targets = await definitions(document, position);
    if (targets.length === 0) {
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  }
  return targets;
}

async function open(relative: string): Promise<vscode.TextDocument> {
  const folder = vscode.workspace.workspaceFolders![0].uri;
  const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder, relative));
  await vscode.window.showTextDocument(document);
  return document;
}

suite("definition", () => {
  test("go to definition reaches Target", async () => {
    const document = await open("src/Use.php");

    const targets = await waitForDefinitions(document, new vscode.Position(2, 14));

    assert.strictEqual(targets.length, 1);
    assert.ok(uriOf(targets[0]).path.endsWith("src/Target.php"), uriOf(targets[0]).toString());
    const start = startOf(targets[0]);
    assert.deepStrictEqual([start.line, start.character], [2, 6]);
  });

  test("returns nothing for a document with unsaved edits", async () => {
    const document = await open("src/Dirty.php");
    const other = await waitForDefinitions(document, new vscode.Position(2, 18));
    assert.ok(uriOf(other[0]).path.endsWith("src/Other.php"));

    // Deleting "new Target(); " moves Other to column 4, where the file on disk has Target.
    const editor = vscode.window.activeTextEditor!;
    await editor.edit((edit) => edit.delete(new vscode.Range(2, 0, 2, 14)));
    try {
      assert.deepStrictEqual(await definitions(document, new vscode.Position(2, 4)), []);
      const hovers = await vscode.commands.executeCommand<vscode.Hover[]>("vscode.executeHoverProvider", document.uri, new vscode.Position(2, 4));
      assert.strictEqual(hovers.length, 0);
    } finally {
      await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    }
  });

  test("keeps the index cache in workspace storage", async () => {
    const api = (await vscode.extensions.getExtension("sparkfabrik.php-navigator")!.activate()) as
      | { cacheDir: string | undefined }
      | undefined;
    assert.ok(api?.cacheDir, "activate returns the cache directory");
    await waitForDefinitions(await open("src/Use.php"), new vscode.Position(2, 14));

    const cache = path.join(api.cacheDir, "index-v1.bin");
    const deadline = Date.now() + 10_000;
    while (!fs.existsSync(cache) && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 200));
    }

    assert.ok(fs.existsSync(cache), cache);
  });

  test("follows a class moved on disk", async () => {
    const document = await open("src/UseMoving.php");
    const moving = path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "src/Moving.php");
    const original = fs.readFileSync(moving, "utf8");
    const position = new vscode.Position(2, 4);
    const before = await waitForDefinitions(document, position);
    assert.strictEqual(startOf(before[0]).line, 2);

    try {
      fs.writeFileSync(moving, original.replace("class Moving", "\n\n\nclass Moving"));
      let line = -1;
      const deadline = Date.now() + 5_000;
      while (line !== 5 && Date.now() < deadline) {
        const targets = await definitions(document, position);
        line = targets.length === 1 ? startOf(targets[0]).line : -1;
        if (line !== 5) {
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
      }
      assert.strictEqual(line, 5);
    } finally {
      fs.writeFileSync(moving, original);
    }
  });

  test("hover shows the class signature", async () => {
    const document = await open("src/Use.php");
    await waitForDefinitions(document, new vscode.Position(2, 14));

    const hovers = await vscode.commands.executeCommand<vscode.Hover[]>("vscode.executeHoverProvider", document.uri, new vscode.Position(2, 14));

    const text = hovers
      .flatMap((hover) => hover.contents)
      .map((content) => (typeof content === "string" ? content : content.value))
      .join("\n");
    assert.ok(text.includes("class Target"), text);
  });

  test("go to implementations reaches the implementing class", async () => {
    const document = await open("src/Shape.php");
    await waitForDefinitions(await open("src/Use.php"), new vscode.Position(2, 14));

    const targets = await vscode.commands.executeCommand<Target[]>("vscode.executeImplementationProvider", document.uri, new vscode.Position(2, 10));

    assert.strictEqual(targets.length, 1);
    assert.ok(uriOf(targets[0]).path.endsWith("src/Square.php"), uriOf(targets[0]).toString());
  });

  test("find references lists the uses of Target", async () => {
    const document = await open("src/Target.php");
    await waitForDefinitions(await open("src/Use.php"), new vscode.Position(2, 14));

    const locations = await vscode.commands.executeCommand<vscode.Location[]>("vscode.executeReferenceProvider", document.uri, new vscode.Position(2, 6));

    assert.ok(locations.some((location) => location.uri.path.endsWith("src/Use.php")), JSON.stringify(locations));
  });
});
