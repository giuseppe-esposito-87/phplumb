import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions, type vsdiag } from "vscode-languageclient/node";
import { resolveServerPath } from "./serverPath";

let client: LanguageClient | undefined;
/** Switches files the server reads as PHP to the PHP language while the server runs. */
let phpFiles: vscode.Disposable | undefined;

/** What `activate` returns; tests use it to find the index cache. */
export interface PhplumbApi {
  cacheDir: string | undefined;
}

/** The files the engine answers in: PHP, and the YAML and Twig files framework modules read. */
export const DOCUMENT_SELECTOR: { scheme: string; language?: string; pattern?: string }[] = [
  { scheme: "file", language: "php" },
  // Framework modules answer in YAML files, such as Drupal's services.
  { scheme: "file", language: "yaml" },
  // Twig templates hold route and template names; no Twig extension is needed.
  { scheme: "file", pattern: "**/*.twig" },
  // Doctrine XML mappings name classes and properties; no XML extension is needed.
  { scheme: "file", pattern: "**/*.orm.xml" },
];

export async function activate(context: vscode.ExtensionContext): Promise<PhplumbApi> {
  context.subscriptions.push(
    vscode.commands.registerCommand("phplumb.restartServer", () => restart(context)),
    vscode.workspace.onDidChangeConfiguration((event) => {
      const restarts = ["phplumb.root", "phplumb.exclude", "phplumb.server.path", "phplumb.diagnostics.enabled", "phplumb.frameworks.drupal", "phplumb.frameworks.symfony"];
      if (restarts.some((section) => event.affectsConfiguration(section))) {
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
  const config = vscode.workspace.getConfiguration("phplumb");
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
    void vscode.window.showErrorMessage(`PHPlumb: ${(err as Error).message}`);
    return;
  }

  const serverOptions: ServerOptions = { command, args: ["serve"] };
  const clientOptions: LanguageClientOptions = {
    documentSelector: DOCUMENT_SELECTOR,
    initializationOptions: {
      root: config.get<string>("root", ""),
      exclude: config.get<string[]>("exclude", []),
      cacheDir: cacheDir(context),
      diagnostics: config.get<boolean>("diagnostics.enabled", true),
      frameworks: {
        drupal: config.get<string>("frameworks.drupal", "auto"),
        symfony: config.get<string>("frameworks.symfony", "auto"),
      },
    },
    diagnosticPullOptions: { onChange: true, onSave: true },
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
      provideDiagnostics: (document, previousResultId, token, next) =>
        isDirty(document) ? noDiagnostics : next(document, previousResultId, token),
    },
  };
  client = new LanguageClient("phplumb", "PHPlumb", serverOptions, clientOptions);
  client.onNotification("phplumb/phpExtensions", (params: { extensions: string[] }) => {
    phpFiles?.dispose();
    phpFiles = openAsPhp(new Set(params.extensions));
  });
  await client.start();
}

/**
 * Switches workspace files with one of `extensions`, such as Drupal's `.module`, to
 * the PHP language, now and whenever one opens. Each file switches once, so a
 * language the user picks for it afterwards stays.
 */
function openAsPhp(extensions: Set<string>): vscode.Disposable {
  const switched = new Set<string>();
  const use = (document: vscode.TextDocument) => {
    const uri = document.uri.toString();
    if (
      document.uri.scheme !== "file" ||
      document.languageId === "php" ||
      switched.has(uri) ||
      !extensions.has(path.posix.extname(document.uri.path).slice(1)) ||
      vscode.workspace.getWorkspaceFolder(document.uri) === undefined
    ) {
      return;
    }
    switched.add(uri);
    void vscode.languages.setTextDocumentLanguage(document, "php");
  };
  vscode.workspace.textDocuments.forEach(use);
  return vscode.workspace.onDidOpenTextDocument(use);
}

/** The report for a document whose unsaved edits the server cannot see. */
const noDiagnostics = { kind: "full", items: [] } as unknown as vsdiag.DocumentDiagnosticReport;

function isDirty(document: vscode.TextDocument | vscode.Uri): boolean {
  if (!(document instanceof vscode.Uri)) {
    return document.isDirty;
  }
  return vscode.workspace.textDocuments.some((open) => open.uri.toString() === document.toString() && open.isDirty);
}

async function stop(): Promise<void> {
  phpFiles?.dispose();
  phpFiles = undefined;
  const running = client;
  client = undefined;
  await running?.stop();
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await stop();
  await start(context);
}
