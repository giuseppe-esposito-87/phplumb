import * as fs from "node:fs";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";
import { resolveServerPath } from "./serverPath";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("phpNavigator.restartServer", () => restart(context)),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("phpNavigator.exclude") || event.affectsConfiguration("phpNavigator.server.path")) {
        void restart(context);
      }
    }),
  );
  await start(context);
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("phpNavigator");
  let command: string;
  try {
    command = resolveServerPath(context.extensionPath, config.get<string>("server.path", ""), process.env, fs.existsSync);
  } catch (err) {
    void vscode.window.showErrorMessage(`PHP Navigator: ${(err as Error).message}`);
    return;
  }

  const serverOptions: ServerOptions = { command, args: ["serve"] };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "php" }],
    initializationOptions: { exclude: config.get<string[]>("exclude", []) },
  };
  client = new LanguageClient("phpNavigator", "PHP Navigator", serverOptions, clientOptions);
  await client.start();
}

async function stop(): Promise<void> {
  const running = client;
  client = undefined;
  await running?.stop();
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await stop();
  await start(context);
}
