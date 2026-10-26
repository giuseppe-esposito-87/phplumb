import * as fs from "node:fs";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";
import { resolveServerPath } from "./serverPath";

let client: LanguageClient | undefined;

/** What `activate` returns; tests use it to find the index cache. */
export interface PhpNavigatorApi {
  cacheDir: string | undefined;
}

export async function activate(context: vscode.ExtensionContext): Promise<PhpNavigatorApi> {
  context.subscriptions.push(
    vscode.commands.registerCommand("phpNavigator.restartServer", () => restart(context)),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("phpNavigator.exclude") || event.affectsConfiguration("phpNavigator.server.path")) {
        void restart(context);
      }
    }),
  );
  await start(context);
  return { cacheDir: cacheDir(context) };
}

/** The index cache lives in the workspace storage; without a workspace there is none. */
function cacheDir(context: vscode.ExtensionContext): string | undefined {
  const dir = context.storageUri?.fsPath;
  if (dir !== undefined) {
    fs.mkdirSync(dir, { recursive: true });
  }
  return dir;
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("phpNavigator");
  let command: string;
  try {
    command = resolveServerPath(
      context.extensionPath,
      config.get<string>("server.path", ""),
      process.env,
      fs.existsSync,
      context.extensionMode === vscode.ExtensionMode.Development,
    );
  } catch (err) {
    void vscode.window.showErrorMessage(`PHP Navigator: ${(err as Error).message}`);
    return;
  }

  const serverOptions: ServerOptions = { command, args: ["serve"] };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "php" }],
    initializationOptions: { exclude: config.get<string[]>("exclude", []), cacheDir: cacheDir(context) },
    middleware: {
      // The server reads files from disk; with unsaved edits a position would map to
      // different code there, so returning nothing beats a wrong target.
      provideDefinition: (document, position, token, next) =>
        document.isDirty ? undefined : next(document, position, token),
      provideHover: (document, position, token, next) =>
        document.isDirty ? undefined : next(document, position, token),
      provideImplementation: (document, position, token, next) =>
        document.isDirty ? undefined : next(document, position, token),
      prepareTypeHierarchy: (document, position, token, next) =>
        document.isDirty ? undefined : next(document, position, token),
      provideReferences: (document, position, options, token, next) =>
        document.isDirty ? undefined : next(document, position, options, token),
      prepareCallHierarchy: (document, position, token, next) =>
        document.isDirty ? undefined : next(document, position, token),
      provideDocumentSymbols: (document, token, next) =>
        document.isDirty ? undefined : next(document, token),
    },
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
