# Development

PHPlumb has two parts: the Rust engine in `crates/` and the VS Code extension in `editors/vscode`. The extension starts the engine binary and talks to it over LSP.

## Prerequisites

- Rust through `rustup`. With Homebrew, `rustup` is keg-only: add `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` to `~/.zshrc`.
- Node.js and npm. Run `npm --prefix editors/vscode install` once.
- Docker, only for building the Linux package.

## Testing in VS Code

1. Open this repository in VS Code and press F5. The `build all` task builds the engine and the extension, then a new window opens with the extension loaded and every other extension disabled.
2. In that window, open a Drupal project once with File > Open Folder. The window reopens it on the next F5.
3. After each change the agent reports as built, press Cmd+R in that window to reload it with the new build.

The index follows file changes made outside the editor, such as an agent's edits or a branch switch, and is cached in the workspace storage, so reopening a project skips parsing unchanged files. Indexing progress appears in the status bar. The **PHPlumb** output channel shows the server log.

To skip paths, set `phplumb.exclude` in the workspace settings, for example `["psalm-stubs/**"]`.

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
cargo run --release -p phplumb-server -- bench --root <drupal-root> --cases <cases.toml>
```

The command prints a JSON report and exits with code 1 when a case fails. Case files that point into client code stay outside the repository. See `docs/benchmarks.md` for recorded results.

## Built-in PHP names

`crates/phplumb-index/data/builtins.txt` lists the classes, functions and constants that PHP and its extensions provide, so diagnostics never flag them. To regenerate it from a newer `phpstorm-stubs` tag, download that tag into an empty directory, then keep the four header lines and update their tag and commit:

```sh
{ sed -n '1,4p' crates/phplumb-index/data/builtins.txt; cargo run -q -p phplumb-server -- builtins <stubs-dir>; } > builtins.new
mv builtins.new crates/phplumb-index/data/builtins.txt
```

## Packaging

Build one `.vsix` per platform:

```sh
npm --prefix editors/vscode run package -- darwin-arm64
npm --prefix editors/vscode run package -- linux-x64
```

- **`darwin-arm64`** builds with the host toolchain, so it needs an Apple silicon Mac. It is the default target there.
- **`linux-x64`** builds a static binary inside the pinned `rust` Alpine image for `linux/amd64`, so it needs only Docker and runs on any x64 distribution. On a Mac, Docker emulates x64, which makes the first build slow; later builds reuse the Cargo caches.

Each command writes `editors/vscode/phplumb-<target>-<version>.vsix`. Install it in a dedicated profile to test it as a user would:

```sh
code --profile "PHPlumb" --install-extension editors/vscode/phplumb-linux-x64-0.2.0.vsix
```
