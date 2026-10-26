import * as assert from "node:assert";
import * as vscode from "vscode";

type Target = vscode.Location | vscode.LocationLink;

function uriOf(target: Target): vscode.Uri {
  return "targetUri" in target ? target.targetUri : target.uri;
}

function startOf(target: Target): vscode.Position {
  return "targetUri" in target ? (target.targetSelectionRange ?? target.targetRange).start : target.range.start;
}

suite("definition", () => {
  test("go to definition reaches Target", async () => {
    const folder = vscode.workspace.workspaceFolders![0].uri;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder, "src/Use.php"));
    await vscode.window.showTextDocument(document);
    const position = new vscode.Position(2, 14);

    // The server indexes the workspace after it starts; poll until it answers.
    let targets: Target[] = [];
    const deadline = Date.now() + 10_000;
    while (targets.length === 0 && Date.now() < deadline) {
      targets = await vscode.commands.executeCommand<Target[]>("vscode.executeDefinitionProvider", document.uri, position);
      if (targets.length === 0) {
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
    }

    assert.strictEqual(targets.length, 1);
    assert.ok(uriOf(targets[0]).path.endsWith("src/Target.php"), uriOf(targets[0]).toString());
    const start = startOf(targets[0]);
    assert.deepStrictEqual([start.line, start.character], [2, 6]);
  });
});
