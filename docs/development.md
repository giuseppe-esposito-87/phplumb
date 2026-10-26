# Development

PHP Navigator has two parts: the Rust engine in `crates/` and the VS Code extension in `editors/vscode`. The extension starts the engine binary and talks to it over LSP.

## Prerequisites

- Rust through `rustup`. With Homebrew, `rustup` is keg-only: add `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` to `~/.zshrc`.
- Node.js and npm. Run `npm --prefix editors/vscode install` once.

## Testing in VS Code

1. Open this repository in VS Code and press F5. The `build all` task builds the engine and the extension, then a new window opens with the extension loaded and every other extension disabled.
2. In that window, open a Drupal project once with File > Open Folder. The window reopens it on the next F5.
3. After each change the agent reports as built, press Cmd+R in that window to reload it with the new build.

Indexing progress appears in the status bar. The **PHP Navigator** output channel shows the server log.

To skip paths, set `phpNavigator.exclude` in the workspace settings, for example `["psalm-stubs/**"]`.

## Automated checks

```sh
cargo test --workspace
cargo clippy --all-targets -- -D warnings
npm --prefix editors/vscode run typecheck
npm --prefix editors/vscode test
```

The extension tests download VS Code Insiders on first run, so they work while the stable VS Code is open.

## Benchmark

```sh
cargo run --release -p phpnav-server -- bench --root <drupal-root> --cases <cases.toml>
```

The command prints a JSON report and exits with code 1 when a case fails. Case files that point into client code stay outside the repository. See `docs/benchmarks.md` for recorded results.
