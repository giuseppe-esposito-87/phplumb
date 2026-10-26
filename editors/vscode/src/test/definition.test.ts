import * as assert from "node:assert";
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
    } finally {
      await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    }
  });
});
