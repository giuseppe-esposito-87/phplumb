# PHP Navigator skeleton implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 1 of the design: a Rust LSP engine and a thin VS Code extension that give "go to definition" for PHP classes, interfaces, traits, enums and functions across a whole project, with every test layer and the benchmark harness in place.

**Architecture:** A Cargo workspace with two crates. `phpnav-index` discovers PHP files, parses them in parallel with `mago-syntax`, resolves names with `mago-names` and keeps a case-insensitive symbol table. `phpnav-server` builds the `php-navigator` binary, which serves LSP over stdio and has a `bench` subcommand. The extension in `editors/vscode` starts the binary through `vscode-languageclient`.

**Tech Stack:** Rust stable (edition 2024), mago 1.59, lsp-server, lsp-types, rayon, ignore, globset. TypeScript, esbuild, vscode-languageclient, @vscode/test-cli.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** This plan covers delivery step 1 (Skeleton). Steps 2 to 4 get their own plans after this one ships, using the benchmark numbers it records. Warm start, incremental updates, Linux x64 packaging and every other navigation feature belong to those plans.

## Global constraints

- **Rust toolchain:** stable, at least `1.99` (the mago crates require it). Edition `2024`.
- **Rust crates:** `mago-syntax`, `mago-names`, `mago-database`, `mago-allocator` all pinned to `=1.59.0`. `lsp-server = "0.10"`, `lsp-types = "0.97"`, `rayon = "1.12"`, `ignore = "0.4"`, `globset = "0.4"`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `toml = "1"`, `url = "2"`, `anyhow = "1"`, `libc = "0.2"`. Dev: `tempfile = "3"`.
- **npm packages:** `vscode-languageclient@^10.1.2`, `typescript@~6.0.3`, `esbuild@^0.28.2`, `@vscode/test-cli@^0.0.15`, `@vscode/test-electron@^3.1.0`, `@types/vscode@^1.140.0`, `@types/mocha@^10.0.10`, `@types/node@^26`, `@vscode/vsce@^4.0.0`. `engines.vscode` is `^1.140.0`.
- **Names:** binary `php-navigator`, extension `sparkfabrik.php-navigator`, settings prefix `phpNavigator.`, environment variables `PHPNAV_REFERENCE_ROOT`, `PHPNAV_REFERENCE_CASES` and `PHPNAV_SERVER_PATH`.
- **PHP file extensions:** `php`, `module`, `inc`, `install`, `theme`, `profile`, `engine`.
- **Name lookup:** class-like and function names are case-insensitive. Index keys are the lowercased fully qualified name without a leading backslash.
- **No wrong jumps:** results come only from resolved names. Never match on bare identifier text.
- **LSP positions** use UTF-16 columns, the LSP default.
- **stdout is the protocol channel** in `serve` mode. Logs go to stderr only.
- **Reference project** (`PHPNAV_REFERENCE_ROOT`) is read-only. No file containing its code or its case list is committed.
- **Code comments** describe what the code is and why, never its history.
- **Commits** use conventional commits with the trailer `Assisted-by: claude-code/claude-opus-5-5`, following the `sf-commit-convention` skill. Before the first commit, ask the user for the issue reference and whether to work on `main` or a branch.
- **Host:** macOS on Apple Silicon. Commands use BSD userland.

## Review focus

- **Paths with spaces or non-ASCII characters** (for example `Application Support` or `Città`) must round-trip between file paths and URIs. Test in Task 6.
- **Multibyte characters and CRLF line endings** before a target must not shift LSP columns. Test in Task 3.
- **Gitignored `vendor` and `web/core`** must still be indexed, because Drupal projects ignore them in git. Test in Task 2.
- **Symlinked module directories and symlink loops** must yield each file once and terminate. Test in Task 2.
- **Non-UTF-8 files and files with syntax errors** must not crash indexing, and their declarations must still be found. Test in Task 4.

---

### Task 1: Toolchain and workspace

**Files:**

- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`
- Create: `crates/phpnav-index/Cargo.toml`, `crates/phpnav-index/src/lib.rs`
- Create: `crates/phpnav-server/Cargo.toml`, `crates/phpnav-server/src/main.rs`

**Interfaces:**

- Produces: a workspace where `cargo build` creates `target/debug/php-navigator`, and `php-navigator --version` prints `php-navigator 0.1.0`.

- [ ] **Step 1: Install Rust if missing**

Run: `command -v cargo || (brew install rustup && rustup default stable)`, then `rustup component add clippy rustfmt`.
Expected: `rustc --version` prints `1.99` or later.

- [ ] **Step 2: Create the repository and workspace**

Run `git init -b main`. The `.gitignore` lists `target/`, `node_modules/`, `editors/vscode/dist/`, `editors/vscode/out/`, `editors/vscode/bin/`, `editors/vscode/.vscode-test/`, `*.vsix` and `.DS_Store`. The root `Cargo.toml` declares `members = ["crates/*"]`, `resolver = "3"`, the shared `[workspace.package]` (version `0.1.0`, edition `2024`, rust-version `1.99`) and every crate from the global constraints under `[workspace.dependencies]`. `rust-toolchain.toml` sets channel `stable` with components `rustfmt` and `clippy`. `phpnav-server` declares `[[bin]] name = "php-navigator"`.

- [ ] **Step 3: Verify**

Run: `cargo build && cargo clippy --all-targets -- -D warnings && ./target/debug/php-navigator --version`
Expected: `php-navigator 0.1.0`

- [ ] **Step 4: Commit** the workspace together with `docs/`.

```bash
git add -A
git commit -m "chore: set up cargo workspace and design docs" --trailer "Assisted-by: claude-code/claude-opus-5-5"
```

### Task 2: File discovery

**Files:**

- Create: `crates/phpnav-index/src/discover.rs`
- Modify: `crates/phpnav-index/src/lib.rs` (declare the module public)
- Test: `crates/phpnav-index/tests/discover.rs`

**Interfaces:**

- Produces:

```rust
pub const PHP_EXTENSIONS: &[&str] = &["php", "module", "inc", "install", "theme", "profile", "engine"];

pub struct DiscoverOptions {
    /// Globs relative to the root, such as `vendor/rector/rector/**`.
    pub exclude: Vec<String>,
    pub max_file_bytes: u64, // Default: 4 * 1024 * 1024
}

/// Absolute paths under the canonicalized root, sorted, each file once.
pub fn discover(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<Vec<PathBuf>>;
```

- [ ] **Step 1: Write the failing tests**

Each test builds a tree in a `tempfile::TempDir` and compares against paths under `root.canonicalize()` (macOS maps `/var` to `/private/var`).

```rust
#[test] fn finds_all_php_extensions()      // a.php b.module c.inc d.install e.theme f.profile g.engine h.txt i.js -> exactly the 7 PHP files
#[test] fn ignores_gitignore_rules()       // .gitignore contains "vendor/"; vendor/x/A.php is returned
#[test] fn skips_hidden_dirs_and_node_modules() // .ds-clone/A.php and node_modules/p/B.php are not returned
#[test] fn applies_exclude_globs()         // exclude ["vendor/rector/rector/**"]: vendor/rector/rector/vendor/X.php absent, vendor/other/Y.php present
#[test] fn skips_files_over_size_limit()   // max_file_bytes = 10: an 11-byte file is absent, a 10-byte file is present
#[test] fn dedupes_symlinked_dirs()        // modules/foo/Foo.php real, web/modules/custom/foo -> ../../../modules/foo: result == [root/modules/foo/Foo.php]
#[test] fn survives_symlink_loop()         // a/A.php and a/loop -> ..: returns, and a/A.php appears once
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p phpnav-index --test discover`
Expected: FAIL to compile, `discover` not found.

- [ ] **Step 3: Implement `discover` in `crates/phpnav-index/src/discover.rs`**

Canonicalize the root first. Walk with `ignore::WalkBuilder` with every ignore-file source turned off (`git_ignore`, `git_global`, `git_exclude`, `ignore`, `parents` all false), `hidden(true)` and `follow_links(true)`. Skip directories named `node_modules`. Match `exclude` with a `globset::GlobSet` against the path relative to the root. Deduplicate by canonical path: keep the canonical path when it is inside the root, otherwise the first path in sorted order.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p phpnav-index --test discover`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/phpnav-index
git commit -m "feat(index): discover php files under a workspace root" --trailer "Assisted-by: claude-code/claude-opus-5-5"
```

### Task 3: Line index

**Files:**

- Create: `crates/phpnav-index/src/line_index.rs`
- Modify: `crates/phpnav-index/src/lib.rs` (declare the module public)
- Test: unit tests in the same file

**Interfaces:**

- Produces:

```rust
pub struct LineIndex { /* byte offset of each line start */ }
impl LineIndex {
    pub fn new(text: &[u8]) -> Self;
    /// 0-based line and UTF-16 column. Invalid UTF-8 bytes count as one unit each.
    pub fn position(&self, text: &[u8], offset: u32) -> (u32, u32);
    /// None when the line does not exist or the column is past the line end.
    pub fn offset(&self, text: &[u8], line: u32, utf16_col: u32) -> Option<u32>;
}
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn ascii()            { /* "a\nbc\n": position(3) == (1, 1); offset(1, 1) == Some(3) */ }
#[test] fn two_byte_char()    { /* "$x = 'è'; Foo": position(11) == (0, 10); offset(0, 10) == Some(11) */ }
#[test] fn surrogate_pair()   { /* "'😀';Foo": position(7) == (0, 5); offset(0, 5) == Some(7) */ }
#[test] fn crlf()             { /* "a\r\nFoo": position(3) == (1, 0); offset(1, 0) == Some(3) */ }
#[test] fn past_line_end()    { /* "ab\n": offset(0, 99) == None; offset(5, 0) == None */ }
#[test] fn invalid_utf8()     { /* b"\xE8\xE8Foo": position(2) == (0, 2); offset(0, 2) == Some(2) */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p phpnav-index line_index`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `LineIndex`** with a `Vec<u32>` of line starts split on `\n`. A `\r` before `\n` belongs to the line it ends.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p phpnav-index line_index`
Expected: 6 passed.

- [ ] **Step 5: Commit** with message `feat(index): map byte offsets to lsp positions`.

### Task 4: File summary

**Files:**

- Create: `crates/phpnav-index/src/summary.rs`
- Modify: `crates/phpnav-index/src/lib.rs` (declare the module public)
- Test: `crates/phpnav-index/tests/summary.rs`

**Interfaces:**

- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind { Class, Interface, Trait, Enum, Function }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind { ClassLike, Function, Constant }

pub struct Declaration { pub kind: SymbolKind, pub fqn: String, pub name_start: u32, pub name_end: u32 }

/// `qualified` is true when the source text of the name contains a backslash.
pub struct NameRef { pub kind: NameKind, pub fqn: String, pub start: u32, pub end: u32, pub qualified: bool }

pub struct FileSummary { pub declarations: Vec<Declaration>, pub names: Vec<NameRef>, pub has_parse_errors: bool }

pub fn summarize(source: &[u8]) -> FileSummary;
```

`fqn` keeps the source casing and has no leading backslash. `name_start..name_end` covers the declared name token only.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn declarations_in_namespace()
// "namespace App\\Model; class Foo {} interface Bar {} trait Baz {} enum Qux {} function helper() {}"
// kinds and fqns: Class App\Model\Foo, Interface App\Model\Bar, Trait App\Model\Baz, Enum App\Model\Qux, Function App\Model\helper
// &source[name_start..name_end] == b"Foo" for the class

#[test] fn declarations_in_global_namespace()   // "class Foo {}" -> fqn "Foo"
#[test] fn braced_namespaces()                  // "namespace A { class X {} } namespace B { class X {} }" -> "A\X", "B\X"

#[test] fn name_refs_resolve_imports()
// "namespace App; use Lib\\Thing as Alias; new Alias(); helper(); \\strlen('x'); FOO;"
// Alias: ClassLike, fqn "Lib\Thing"; helper: Function, qualified false; \strlen: Function, qualified true; FOO: Constant

#[test] fn class_and_function_with_same_name()  // "function Foo() {} new Foo(); Foo();" -> the two Foo refs have kinds ClassLike and Function
#[test] fn attribute_names_are_class_refs()     // "use Drupal\\Core\\Hook\\Attribute\\Hook; #[Hook('form_alter')] function f() {}" -> Hook: ClassLike, fqn "Drupal\Core\Hook\Attribute\Hook"
#[test] fn recovers_after_syntax_error()        // "class A { public function f( } class B {}" -> has_parse_errors, declarations contain B
#[test] fn tolerates_invalid_utf8()             // b"<?php\n// \xE8\nclass Foo {}" -> declaration Foo
#[test] fn parses_php_8_4_and_8_5_syntax()      // "class H { public string $n { get => 'x'; } } $r = $x |> trim(...); class After {}" -> no parse errors, After declared
```

Every test source starts with `<?php\n`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p phpnav-index --test summary`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `summarize`**

Use one `mago_allocator::LocalArena` per call, `mago_syntax::parser::parse_file_content` and `mago_names::resolver::NameResolver`. Walk the CST with `mago_syntax::walker::Walker` to collect declarations (tracking the current namespace) and the spans of function-call names, `use function` items and constant accesses. A resolved name is `Function` or `Constant` when its span is in those sets, otherwise `ClassLike`. Copy everything into owned values before the arena drops.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p phpnav-index --test summary`
Expected: 9 passed.

- [ ] **Step 5: Commit** with message `feat(index): extract declarations and resolved names`.

### Task 5: Symbol index, definition query and fixture tests

**Files:**

- Create: `crates/phpnav-index/src/index.rs`
- Modify: `crates/phpnav-index/src/lib.rs` (re-export the public API)
- Test: `crates/phpnav-index/tests/fixtures.rs`, `crates/phpnav-index/tests/fixtures/<case>/**`

**Interfaces:**

- Consumes: `discover`, `DiscoverOptions` (Task 2), `summarize`, `SymbolKind`, `NameKind` (Task 4).
- Produces:

```rust
pub struct Location { pub path: PathBuf, pub start: u32, pub end: u32 }
pub struct IndexStats { pub files: usize, pub declarations: usize, pub files_with_parse_errors: usize, pub elapsed_ms: u128 }

pub struct Index { /* paths, by_class and by_function: HashMap<String, Vec<(file, start, end, SymbolKind)>> */ }
impl Index {
    pub fn build(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<(Index, IndexStats)>;
    /// `source` is the current content of `path`; results are sorted by (path, start).
    pub fn definition(&self, path: &Path, source: &[u8], offset: u32) -> Vec<Location>;
}
```

Definition rules, in order:

1. Summarize `source` and take the name or declaration whose range covers `offset` (`start <= offset <= end`).
2. On a declaration name, return that declaration.
3. `ClassLike` returns every class, interface, trait or enum with that lowercased fqn.
4. `Function` returns every function with that lowercased fqn. If there is none and the name is not `qualified`, retry with the last segment (PHP global fallback).
5. `Constant`, unknown names and positions outside any name return nothing.

**Fixture format.** Each directory under `tests/fixtures/` is one workspace. `/*@cursor:ID*/` places a cursor at the byte right after the marker. `/*@target:ID*/` marks a target at the first non-whitespace byte after the marker. A case passes only when the set of `(relative path, start)` results equals the set of targets with the same ID. A cursor with no target expects nothing. Example `tests/fixtures/names/src/Use.php`:

```php
<?php
namespace App;
use Lib\Thing as Alias;
new /*@cursor:alias*/Alias();
```

with `tests/fixtures/names/lib/Thing.php` containing `namespace Lib; class /*@target:alias*/Thing {}`.

- [ ] **Step 1: Write the fixture runner and fixtures**

`tests/fixtures.rs` has one test per fixture directory (`names`, `functions`, `kinds`, `ambiguous`, `tolerant`). Each builds the `Index` for its directory, runs `definition` for every cursor and fails with a list of every mismatching ID. Required cases:

- `names`: same namespace, `use` import, alias, fully qualified `\Lib\Thing`, `extends`, `implements`, static call `Thing::make()`, `instanceof`, `catch`, attribute, parameter, return and property types, and `new thing()` matching `Thing` regardless of case.
- `functions`: namespaced function, global fallback for an unqualified call, namespaced function preferred over a global one with the same name, `use function`, and a function declared in a `.module` file.
- `kinds`: interface, trait and enum targets, plus a class and a function with the same name each reached from the right call site.
- `ambiguous`: one class declared in two files returns both targets, and an unknown class returns nothing.
- `tolerant`: a class declared after a syntax error in its file is still found.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p phpnav-index --test fixtures`
Expected: FAIL to compile, `Index` not found.

- [ ] **Step 3: Implement `Index::build` and `Index::definition`**

`build` reads and summarizes files in parallel with `rayon`, keeps only declarations, and fills the two maps. Unreadable files are counted and skipped, never fatal.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p phpnav-index`
Expected: all tests pass, including 5 fixture tests.

- [ ] **Step 5: Commit** with message `feat(index): resolve definitions for classes and functions`.

### Task 6: LSP server

**Files:**

- Create: `crates/phpnav-server/src/convert.rs`, `crates/phpnav-server/src/lsp.rs`
- Modify: `crates/phpnav-server/src/main.rs` (subcommands `serve`, the default, and `--version`)
- Test: unit tests in `convert.rs`, `crates/phpnav-server/tests/lsp_definition.rs`

**Interfaces:**

- Consumes: `Index`, `IndexStats`, `Location` (Task 5), `LineIndex` (Task 3).
- Produces:

```rust
pub fn path_to_uri(path: &Path) -> lsp_types::Uri;
pub fn uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf>;

/// Parsed from `initializationOptions`: `{ "exclude": ["vendor/rector/rector/**"] }`.
pub struct ServerSettings { pub exclude: Vec<String> }

pub fn run(connection: lsp_server::Connection) -> anyhow::Result<()>;
```

Server behavior:

- `initialize` advertises `definitionProvider: true` only. The workspace root is the first workspace folder, or `rootUri` when there are none.
- After `initialized`, the server builds the index on the main loop, so requests that arrive meanwhile wait in the queue. When the client supports `window.workDoneProgress`, it reports progress titled `PHP Navigator` with the message `Indexing`, and ends with `Indexed <files> files in <ms> ms`.
- `textDocument/definition` reads the requested file from disk, converts the position with `LineIndex::offset`, calls `Index::definition` and converts each result back with the target file's `LineIndex`.
- `shutdown` and `exit` follow the LSP spec; the process exits with code 0.

- [ ] **Step 1: Write the failing tests**

```rust
// convert.rs
#[test] fn uri_round_trip_with_space_and_accent()
// path "/Users/x/Application Support/Città/Foo.php"
// path_to_uri(..).as_str() == "file:///Users/x/Application%20Support/Citt%C3%A0/Foo.php"
// uri_to_path(&path_to_uri(p)) == Some(p)
```

```rust
// tests/lsp_definition.rs
#[test] fn definition_over_stdio()
// Workspace in a TempDir:
//   src/Target.php: "<?php\nnamespace App;\nclass Target {}\n"
//   src/Use.php:    "<?php\nnamespace App;\n$x = 'è'; new Target();\n"
// Spawn env!("CARGO_BIN_EXE_php-navigator") with piped stdio and speak Content-Length framed JSON-RPC.
// Send initialize (workspaceFolders = [the temp dir]), initialized,
// then textDocument/definition on Use.php at line 2, character 14 (the "T" of Target; "è" is one UTF-16 unit).
// Assert: one Location, uri ends with "/src/Target.php", range.start == { line: 2, character: 6 }.
// Send shutdown and exit; the process exits with status 0 within 5 seconds.
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p phpnav-server`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `convert.rs`, `lsp.rs` and the `serve` subcommand**

Use `url::Url::from_file_path` and `Url::to_file_path` for conversions, parsing into `lsp_types::Uri` with `FromStr`. Use `lsp_server::Connection::stdio()`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p phpnav-server && cargo clippy --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit** with message `feat(server): serve go to definition over lsp`.

### Task 7: Benchmark command and reference cases

**Files:**

- Create: `crates/phpnav-server/src/bench.rs`
- Modify: `crates/phpnav-server/src/main.rs` (subcommand `bench`)
- Test: `crates/phpnav-server/tests/bench.rs`
- Create: `docs/benchmarks.md`

**Interfaces:**

- Consumes: `Index`, `IndexStats`, `LineIndex`.
- Produces: `php-navigator bench --root <DIR> [--cases <FILE>] [--exclude <GLOB>]...`. It prints one JSON object to stdout and exits with code 1 when any case fails:

```json
{
  "files": 0,
  "declarations": 0,
  "files_with_parse_errors": 0,
  "cold_index_ms": 0,
  "peak_rss_mb": 0,
  "cases_passed": 0,
  "cases_failed": 0,
  "failures": [{ "name": "", "expected": [], "actual": [] }]
}
```

Case file format. Positions are `relative/path:line:column`, 1-based, columns in UTF-16 units, pointing at the start of a name. An empty `expect` means "nothing":

```toml
[[case]]
name = "custom block extends core BlockBase"
kind = "definition"
at = "web/modules/custom/foo/src/Plugin/Block/FooBlock.php:12:28"
expect = ["web/core/lib/Drupal/Core/Block/BlockBase.php:24:16"]
```

`peak_rss_mb` comes from `libc::getrusage(RUSAGE_SELF)`: `ru_maxrss` is in bytes on macOS and in KiB on Linux.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn bench_reports_cases()
// Run the binary: bench --root crates/phpnav-index/tests/fixtures/names --cases <temp file>
// The temp file has one case whose expect matches and one whose expect is a wrong location.
// Assert: exit status 1; JSON files == number of PHP files in the fixture; cases_passed == 1; cases_failed == 1;
// failures[0].name is the failing case's name.
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p phpnav-server --test bench`
Expected: FAIL, unknown subcommand.

- [ ] **Step 3: Implement `bench.rs`**

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p phpnav-server --test bench`
Expected: PASS.

- [ ] **Step 5: Write the reference cases**

Write at least 15 definition cases for the reference project in `$HOME/.local/share/php-navigator/cases/reference.toml`, outside the repository. Cover custom code reaching core, contrib and vendor classes, a `.module` function, an aliased import, a class found in two places, and at least 3 cases that expect nothing. Verify each expected location by reading the target file.

- [ ] **Step 6: Run the reference benchmark**

Run: `PHPNAV_REFERENCE_ROOT=<reference-project> PHPNAV_REFERENCE_CASES=$HOME/.local/share/php-navigator/cases/reference.toml; cargo run --release -p phpnav-server -- bench --root "$PHPNAV_REFERENCE_ROOT" --cases "$PHPNAV_REFERENCE_CASES"`
Expected: `cold_index_ms` under `10000`, `peak_rss_mb` under `1024`, `cases_failed` equal to `0`.

If `cold_index_ms` is 10000 or more, stop and report the numbers to the user before continuing. The parser choice depends on it.

- [ ] **Step 7: Record the results** in `docs/benchmarks.md`: date, machine, file count and the JSON numbers, without case names or paths from the reference project.

- [ ] **Step 8: Commit** with message `feat(server): add benchmark command with case files`.

### Task 8: VS Code extension and development loop

**Files:**

- Create: `editors/vscode/package.json`, `editors/vscode/tsconfig.json`, `editors/vscode/tsconfig.test.json`, `editors/vscode/esbuild.mjs`, `editors/vscode/.vscode-test.mjs`, `editors/vscode/.vscodeignore`
- Create: `editors/vscode/src/extension.ts`, `editors/vscode/src/serverPath.ts`
- Create: `editors/vscode/test-fixtures/basic/src/Target.php`, `editors/vscode/test-fixtures/basic/src/Use.php` (same content as the Task 6 test workspace)
- Test: `editors/vscode/src/test/serverPath.test.ts`, `editors/vscode/src/test/definition.test.ts`
- Create: `.vscode/launch.json`, `.vscode/tasks.json`, `docs/development.md`

**Interfaces:**

- Consumes: the `php-navigator` binary and its `initializationOptions` (Task 6).
- Produces:

```ts
/** Order: configured setting, env PHPNAV_SERVER_PATH, <ext>/bin/php-navigator, <ext>/../../target/debug/php-navigator.
 *  Throws an Error listing every path tried when none exists. */
export function resolveServerPath(
  extensionPath: string,
  configured: string,
  env: NodeJS.ProcessEnv,
  exists: (p: string) => boolean,
): string;
```

`package.json` contributes:

- `activationEvents`: `["onLanguage:php"]`.
- `languages`: id `php` with extensions `.module`, `.inc`, `.install`, `.theme`, `.profile`, `.engine`.
- Settings: `phpNavigator.server.path` (string, default `""`), `phpNavigator.exclude` (string array, default `[]`, globs relative to the workspace folder) and `phpNavigator.trace.server` (`off`, `messages` or `verbose`).
- Command `phpNavigator.restartServer`, titled `PHP Navigator: Restart Server`.
- Scripts: `build` (esbuild to `dist/extension.js`), `watch`, `typecheck`, `pretest` (build, then `tsc -p tsconfig.test.json` to `out/`) and `test` (`vscode-test`).

`extension.ts` starts a `LanguageClient` with id `phpNavigator`, the server command with argument `serve`, document selector `{ scheme: "file", language: "php" }` and `initializationOptions: { exclude }`. It restarts the client when `phpNavigator.exclude` changes or the restart command runs.

`.vscode-test.mjs` uses `version: "insiders"`, `workspaceFolder: "./test-fixtures/basic"`, `launchArgs: ["--disable-extensions"]`, `env: { PHPNAV_SERVER_PATH: <absolute path of target/debug/php-navigator> }` and a mocha timeout of 20 seconds.

- [ ] **Step 1: Write the failing tests**

```ts
// serverPath.test.ts
test("prefers the configured path"); // configured "/a" exists -> "/a"
test("falls back to the env var"); // configured "", env PHPNAV_SERVER_PATH "/b" exists -> "/b"
test("falls back to the bundled binary, then the dev build");
test("throws listing every path tried when none exists");

// definition.test.ts
test("go to definition reaches Target");
// Open src/Use.php. Poll vscode.executeDefinitionProvider at line 2, character 14 for up to 10 seconds
// until it returns a result. Assert: one location, uri ends with "src/Target.php", range.start is line 2, character 6.
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo build -p phpnav-server && npm --prefix editors/vscode install && npm --prefix editors/vscode test`
Expected: FAIL to compile, `resolveServerPath` not found.

- [ ] **Step 3: Implement `serverPath.ts` and `extension.ts`**

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test`
Expected: 5 passing.

- [ ] **Step 5: Set up the user's loop**

`.vscode/tasks.json` defines `build all`: `cargo build -p phpnav-server && npm --prefix editors/vscode run build`. `.vscode/launch.json` defines `Run PHP Navigator`: type `extensionHost`, args `["--disable-extensions", "--extensionDevelopmentPath=${workspaceFolder}/editors/vscode"]`, `preLaunchTask: "build all"`. `docs/development.md` explains the loop in a few lines: press F5, open the Drupal project once in the new window, and press Cmd+R after each change the agent reports as built.

- [ ] **Step 6: Commit** with message `feat(vscode): add extension client and development loop`.

### Task 9: Package and hand over

**Files:**

- Modify: `editors/vscode/package.json` (script `package`)

**Interfaces:**

- Produces: `editors/vscode/php-navigator-darwin-arm64-0.1.0.vsix`, containing `extension/bin/php-navigator` and `extension/dist/extension.js`.

- [ ] **Step 1: Add the `package` script**

It runs `cargo build --release -p phpnav-server`, copies `target/release/php-navigator` to `editors/vscode/bin/`, runs `build`, then `vsce package --target darwin-arm64 --no-dependencies`.

- [ ] **Step 2: Verify the package**

Run: `npm --prefix editors/vscode run package && unzip -l editors/vscode/php-navigator-darwin-arm64-0.1.0.vsix | grep -E 'bin/php-navigator|dist/extension.js'`
Expected: both entries listed.

- [ ] **Step 3: Run every automated check**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && npm --prefix editors/vscode test`
Expected: all pass.

- [ ] **Step 4: Commit** with message `build(vscode): package the extension for macos arm64`.

- [ ] **Step 5: Hand over to the user** with these checks to run in the Extension Development Host on the reference project:

1. The progress notification shows the file count and indexing time.
2. Cmd+click on a class name in a custom module opens the right core, contrib or vendor file.
3. Cmd+click on a function defined in a `.module` file opens it.
4. Cmd+click on an unknown name does nothing.
