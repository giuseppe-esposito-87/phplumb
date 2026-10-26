# PHP Navigator 2e: symbols, outline and Linux packaging implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Search symbols across the workspace (Cmd+T), show the outline of the current file, and ship the extension for Linux x64 next to macOS arm64.

**Architecture:** The outline is a pure function of a file's source, built from the declaration and member ranges the summary already records. Workspace symbol search scans the index's lean declarations and members in memory, ranks the matches and returns the best 100; the server computes positions only for those. The Linux binary is a static `x86_64-unknown-linux-musl` build made inside an official Rust Alpine container, with mimalloc as its allocator, and a Node script packages a platform-specific `.vsix` for either target.

**Tech Stack:** existing workspace; LSP document and workspace symbols from `lsp-types` 0.97; Docker for the Linux build; `mimalloc` on musl only; `@vscode/vsce` platform targets.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** Last part of delivery step 2. Broken reference diagnostics (step 3) and the Drupal module (step 4) have their own plans.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main` with no push, never kill processes by name.
- **Platforms:** macOS arm64 and Linux x64. A `.vsix` per platform, built locally; no marketplace publishing.
- **Host scripts** run on macOS and Linux. The packaging script is Node, so it needs no shell differences; the Linux build needs only Docker on the host.
- **Symbol kinds:** class `CLASS`, interface `INTERFACE`, trait `STRUCT` (as in the type hierarchy), enum `ENUM`, function `FUNCTION`, constant and class constant `CONSTANT`, method `METHOD`, `__construct` `CONSTRUCTOR`, property `PROPERTY`, enum case `ENUM_MEMBER`.
- **Targets:** a workspace symbol query answers in under 100 ms on the reference project (agent's call: it runs on every keystroke); cold index under 10 seconds and memory under 1 GB, on macOS and on the Linux binary.

## Review focus

- **A file with a syntax error** still shows the declarations around the error in its outline. Test in Task 1.
- **Promoted constructor properties, grouped constants and enum cases** get a range that contains their name; VS Code rejects the whole outline otherwise. Test in Task 1.
- **A name after multibyte text on the same line** gets UTF-16 columns in the outline and in workspace symbols. Test in Task 3.
- **A camel-hump query** (`gEnt` for `getEntity`) and a query in another case find the symbol; an exact name ranks above longer matches, project code above `vendor`. Test in Task 2.
- **The Linux binary** runs on an older distribution than the build image, and the Rust test suite passes on Linux. Checked in Task 4.

---

### Task 1: Outline

**Files:**

- Create: `crates/phpnav-index/src/symbols.rs`
- Modify: `crates/phpnav-index/src/lib.rs`
- Test: `crates/phpnav-index/tests/symbols.rs`

**Interfaces:**

- Produces:

```rust
/// What a symbol is: a top-level declaration or a member of a class-like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Declaration(SymbolKind),
    Member(MemberKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineSymbol {
    pub name: String,          // short name; members by their own name
    pub kind: Kind,
    pub range: (u32, u32),     // whole declaration, always containing `selection`
    pub selection: (u32, u32), // the name
    pub children: Vec<OutlineSymbol>, // members of a class-like, in source order
}

/// The named declarations of `source`, in source order, with class-like members as
/// children. Anonymous classes and closures are left out.
pub fn outline(source: &[u8]) -> Vec<OutlineSymbol>;
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn outline_nests_members_in_source_order()
// namespace App; const LIMIT = 1; function helper() {} interface I { function run(); }
// trait T { public $t; } enum E: string { case A = 'a'; const D = self::A; }
// class C implements I { use T; const X = 1, Y = 2; public function __construct(private int $p) {} public function run() {} }
// names and kinds, top level: LIMIT Constant, helper Function, I Interface, T Trait, E Enum, C Class
// C children: X, Y (Constant), __construct (Method), p (Property), run (Method), in source order
#[test] fn outline_ranges_contain_their_names()     // every symbol, children included: range.0 <= selection.0 && selection.1 <= range.1
#[test] fn outline_skips_anonymous_classes()        // $x = new class { function f() {} }; function g() {} -> only g
#[test] fn outline_survives_syntax_errors()         // class A { function a() {} } function ( { class B {} -> A (with a) and B listed
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phpnav-index --test symbols`. Expected: FAIL to compile, `symbols` not found.

- [ ] **Step 3: Implement `outline`** from `summarize(source)`: declarations sorted by `range.0`, members by `range.0`; a range that does not contain its selection is widened to it.

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phpnav-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): build file outlines`.

### Task 2: Workspace symbol search

**Files:**

- Modify: `crates/phpnav-index/src/symbols.rs`, `crates/phpnav-index/src/index.rs`
- Test: `crates/phpnav-index/tests/symbols.rs`

**Interfaces:**

- Consumes: `Kind` (Task 1); `references::rank` for path ordering.
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolMatch {
    pub name: String,
    pub kind: Kind,
    /// The namespace of a declaration (`None` when global), the class of a member.
    pub container: Option<String>,
    pub location: Location, // the name
}

impl Index {
    /// Declarations and members matching `query`, best first, at most `limit`.
    pub fn workspace_symbols(&self, query: &str, limit: usize) -> Vec<SymbolMatch>;
}
```

Matching is ASCII case-insensitive on bytes. A query without `\` matches the short name; a query with `\` (a leading one is ignored) matches the fully qualified name of declarations only. Tiers: exact, prefix, substring, then subsequence; anything else is no match. Order: tier, declarations before members, `references::rank` of the path, name length, name, path, offset. A blank query returns nothing.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn symbols_rank_exact_then_prefix_then_substring_then_subsequence()
// classes Node, NodeStorage, BaseNode, NamedObjectDefault; query "node" -> Node, NodeStorage, BaseNode, NamedObjectDefault
#[test] fn symbols_match_camel_humps_and_any_case()   // method getEntity: "gEnt" and "GETENTITY" find it, container "App\\Repo"
#[test] fn symbols_with_a_backslash_match_qualified_names() // "app\\node" finds App\Node, not Other\Node; "\\App\\Node" too
#[test] fn symbols_list_project_code_before_vendor()  // vendor/acme/lib/Node.php and src/Node.php: src first
#[test] fn symbols_respect_the_limit_and_ignore_blank_queries() // limit 2 -> 2 results; "  " -> empty
```

- [ ] **Step 2: Run them and watch them fail.** Expected: FAIL to compile, `workspace_symbols` not found.

- [ ] **Step 3: Implement `workspace_symbols`** with no allocation per non-matching name, and a partial sort (`select_nth_unstable_by`) before the final sort of the kept matches.

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): search workspace symbols`.

### Task 3: Server, extension and benchmark

**Files:**

- Modify: `crates/phpnav-server/src/lsp.rs`, `crates/phpnav-server/src/bench.rs`, `crates/phpnav-server/tests/bench.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`, `docs/benchmarks.md`
- Create: `crates/phpnav-server/tests/lsp_symbols.rs`

**Interfaces:**

- Consumes: `outline`, `workspace_symbols` (Tasks 1 and 2).
- Produces: capabilities `documentSymbolProvider` and `workspaceSymbolProvider`. `textDocument/documentSymbol` answers nested `DocumentSymbol`s from the file on disk. `workspace/symbol` answers at most 100 `WorkspaceSymbol`s with full locations and `containerName`. The extension skips document symbols on documents with unsaved edits, like the other requests. The bench report gains `symbols_ms`: the slowest of the queries `node`, `EntityStorage`, `gEnt`, `load` and `zzqqxx` after indexing.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn document_symbols_over_stdio()   // "<?php\n/* é */ class Target { public function run() {} }": Target CLASS at line 1, selection start character 11 (UTF-16), child run METHOD
#[test] fn workspace_symbols_over_stdio()  // query "targ" -> first name "Target", kind CLASS, uri ends with /src/Target.php, containerName absent
#[test] fn bench_reports_symbol_search_time() // fixture names: report["symbols_ms"] is a number
```

```ts
test("outline lists the class and its members", ...)   // vscode.executeDocumentSymbolProvider on src/Shape.php
test("workspace symbols find a class by camel humps", ...) // vscode.executeWorkspaceSymbolProvider("Squ") includes Square
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phpnav-server --test lsp_symbols --test bench`. Expected: FAIL.

- [ ] **Step 3: Implement the handlers, the middleware and `symbols_ms`.** Positions come from one `LineIndex` per result file.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phpnav-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Run the reference benchmark.** Run: `cargo build --release -p phpnav-server && ./target/release/php-navigator bench --root <reference-project> --cases "$HOME/.local/share/php-navigator/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0; `symbols_ms` under 100; cold index under 10000; `peak_rss_mb` under 1024. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 6: Add a dated section to `docs/benchmarks.md`** and commit with message `feat: serve document and workspace symbols`.

### Task 4: Linux x64 packaging

**Files:**

- Modify: `crates/phpnav-server/Cargo.toml`, `crates/phpnav-server/src/main.rs`, `Cargo.toml`, `editors/vscode/package.json`, `docs/development.md`, `docs/benchmarks.md`
- Create: `editors/vscode/scripts/package.mjs`

**Interfaces:**

- Produces: `npm --prefix editors/vscode run package -- <target>`, with `<target>` `darwin-arm64` (default on a Mac, built with the host toolchain) or `linux-x64` (built in Docker on any host). Output: `editors/vscode/php-navigator-<target>-<version>.vsix` containing `bin/php-navigator` for that platform. The Linux build runs the official `rust:<version>-alpine` image for `linux/amd64`, pinned by version and digest, with `RUSTUP_TOOLCHAIN` set to the image's toolchain so `rust-toolchain.toml` does not trigger a download, `CARGO_TARGET_DIR=target/linux-x64`, a named volume for the Cargo registry, and `cargo build --release --locked -p phpnav-server`.

Load the `sf-container-build` skill before writing the Docker invocation.

- [ ] **Step 1: Write the failing check.** Run: `npm --prefix editors/vscode run package -- linux-x64`. Expected: FAIL, the script does not exist.

- [ ] **Step 2: Add mimalloc as the global allocator on musl targets only** (`[target.'cfg(target_env = "musl")'.dependencies]`, `#[cfg(target_env = "musl")] #[global_allocator]`), then write `scripts/package.mjs` and point the `package` script at it.

- [ ] **Step 3: Build both packages.** Run: `npm --prefix editors/vscode run package -- linux-x64 && npm --prefix editors/vscode run package -- darwin-arm64`. Expected: both `.vsix` files exist; `unzip -Z` shows `extension/bin/php-navigator` as `-rwxr-xr-x` in each; `file` on the Linux binary reports a static x86-64 ELF.

- [ ] **Step 4: Check the binary on Linux.** Run the Linux binary's `--version` in `debian:11` for `linux/amd64` (an older distribution than the build image). Run `cargo test --workspace --locked` in the build image for `linux/amd64`. Run the reference benchmark with the Linux binary in a `linux/amd64` container with the project mounted read-only. Expected: the version prints; all tests pass; `cases_failed` 0, cold index under 10000 and `peak_rss_mb` under 1024, noting that the Mac runs it under emulation.

- [ ] **Step 5: Document packaging and installing** (`code --install-extension <file>.vsix`) in `docs/development.md`, add the Linux numbers to `docs/benchmarks.md`, and commit with message `build: package the extension for linux x64`.
