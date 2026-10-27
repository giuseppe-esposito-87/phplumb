# PHPlumb 3: broken reference diagnostics implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flag classes, functions, methods and constants that do not exist, so code an agent invented stands out, without ever flagging code that works.

**Architecture:** A diagnostics pass runs on one file at a time, from its source on disk, and reports a reference only when every rule below proves the target exists nowhere: not in the index, not among PHP's built-in symbols, not behind a magic method or a docblock tag, not guarded by an `*_exists()` check. PHP's built-in names come from a list generated once from JetBrains `phpstorm-stubs` and compiled into the engine. A method is missing only when no class compatible with the receiver declares it: the receiver's class, its ancestors and every subtype. The server answers LSP pull diagnostics for project code and asks the editor to pull again after each index update.

**Tech Stack:** existing workspace; LSP 3.17 pull diagnostics (`textDocument/diagnostic`, `workspace/diagnostic/refresh`) from `lsp-types` 0.97; `phpstorm-stubs` tag `v2026.2` (commit `709e512210784a7c0a677b3a89d35def844a59b9`, Apache-2.0) as a build-time data source only.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** Delivery step 3, core broken references. Drupal ids (services, routes, plugins) belong to step 4. Properties are not checked: the spec lists classes, functions, methods and constants, and PHP allows dynamic properties.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name. **No client names or client paths in any committed file**; the reference project is `<reference-project>` and its case file `$HOME/.local/share/phplumb/cases/reference.toml`.
- **False alarms are the failure that matters.** When any part of a resolution is uncertain (unknown receiver, unknown ancestor, magic, a guard), the engine stays silent.
- **Scope:** diagnostics cover project code only: files whose path relative to the workspace root has no `vendor`, `core` or `contrib` component. The setting `phplumb.diagnostics.enabled` (default `true`) turns them off.
- **Messages and severities:** `Undefined class 'App\Foo'`, `Undefined function 'foo'`, `Undefined method 'App\Foo::bar()'`, `Undefined constant 'FOO'`, `Undefined class constant 'App\Foo::BAR'`; source `phplumb`. Severity Error for instantiation, calls, constant reads, `extends`, `implements` and trait `use`; Warning for type declarations, `instanceof` and `catch`.
- **Targets on the reference project:** zero false alarms in `web/modules/custom`, every diagnostic there checked by hand; diagnostics for the slowest custom file under 500 ms; cold index under 10 seconds and memory under 1 GB.

## Review focus

- **Optional dependencies:** `if (class_exists(Foo::class)) { new Foo(); }`, `function_exists('bar')` and `defined('BAZ')` guards silence those names in the file. Test in Task 3.
- **Methods that exist only on subtypes** (`$entity->get()` on an `EntityInterface`) are never flagged; a method declared nowhere in the receiver's family is. Test in Task 3.
- **Built-in ancestors and magic:** a class extending `\Exception`, a class or ancestor with `__call` or `__callStatic`, or a docblock with `@method` or `@mixin` silences its method diagnostics; enum `cases()`, `from()` and `tryFrom()` are never flagged. Test in Task 3.
- **Global fallback:** an unqualified function or constant in a namespace is flagged only when neither the namespaced nor the global name exists. Test in Task 3.
- **A file changing while the editor shows its diagnostics:** dirty documents get no diagnostics, and an index update makes the editor pull again, so a diagnostic disappears once the missing class is created. Test in Task 4.

---

### Task 1: Built-in PHP names

**Files:**

- Create: `crates/phplumb-index/src/builtins.rs`, `crates/phplumb-index/data/builtins.txt`, `crates/phplumb-index/data/README.md`
- Modify: `crates/phplumb-index/src/lib.rs`, `crates/phplumb-server/src/main.rs`, `docs/development.md`
- Test: `crates/phplumb-index/tests/builtins.rs`

**Interfaces:**

- Produces:

```rust
// phplumb_index::builtins
pub fn is_class(fqn: &str) -> bool;     // ASCII case-insensitive, no leading backslash
pub fn is_function(fqn: &str) -> bool;  // ASCII case-insensitive
pub fn is_constant(fqn: &str) -> bool;  // namespace case-insensitive, name exact
/// The list for `stubs`: one `c`, `f` or `k` line per class-like, function or
/// constant declared in any PHP file below it, sorted and deduplicated.
pub fn generate(stubs: &Path) -> anyhow::Result<String>;
```

The subcommand `phplumb builtins <stubs-dir>` prints `generate`'s output. `data/builtins.txt` starts with `#` comment lines naming the source repository, tag, commit and license, followed by the generated lines; `data/README.md` carries the Apache-2.0 attribution. The list is embedded with `include_str!` and parsed once into hash sets.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn builtins_know_core_and_extension_names()  // is_class("DateTime"), is_class("datetimeinterface"), is_class("MongoDB\\Driver\\Manager"), is_function("strlen"), is_function("STR_CONTAINS"), is_constant("PHP_EOL"), is_constant("E_ALL")
#[test] fn builtins_reject_unknown_names()            // !is_class("App\\Foo"), !is_function("my_helper"), !is_constant("php_eol")
#[test] fn generate_lists_stub_declarations()        // temp dir with "<?php namespace NS; class A {} function f() {} const K = 1; define('D', 1);" -> lines "c ns\\a", "f ns\\f", "k ns\\K", "k D"
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test builtins`. Expected: FAIL to compile, `builtins` not found.

- [ ] **Step 3: Implement `generate`, the subcommand and the loader;** fetch `phpstorm-stubs` at commit `709e512210784a7c0a677b3a89d35def844a59b9` into the scratchpad, generate `data/builtins.txt`, and document how to regenerate it in `docs/development.md`.

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): know php built-in names`.

### Task 2: Guards and magic in the summary

**Files:**

- Modify: `crates/phplumb-index/src/summary.rs`
- Test: `crates/phplumb-index/tests/summary.rs`

**Interfaces:**

- Produces:

```rust
pub struct ClassShape { /* existing */ #[serde(default)] pub magic: bool }
// true when the class-like declares __call or __callStatic, or its docblock has an
// @method or @mixin tag.

pub struct FileSummary { /* existing */ pub guarded: Vec<String> }
// Lowercased names the file checks before use: string literal or `X::class`
// arguments of class_exists, interface_exists, trait_exists, enum_exists,
// function_exists, method_exists and defined, resolved like the names they stand
// for (classes fully qualified, functions and constants as written without a
// leading backslash, methods as the bare name).
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn magic_marks_call_handlers_and_docblock_tags() // __call, __callStatic, "/** @method int foo() */", "/** @mixin Other */" -> magic; a plain class -> not
#[test] fn guarded_collects_exists_checks()              // namespace App; use Lib\Foo; if (class_exists(Foo::class)) {} if (class_exists('Lib\\Bar')) {} function_exists('helper'); method_exists($x, 'run'); defined('LIMIT'); -> {"lib\\foo", "lib\\bar", "helper", "run", "limit"}
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test summary`. Expected: FAIL to compile.

- [ ] **Step 3: Implement both in the summary walker.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): record exists guards and magic classes`.

### Task 3: The diagnostics pass

**Files:**

- Create: `crates/phplumb-index/src/diagnostics.rs`, `crates/phplumb-index/tests/fixtures/diagnostics/**`
- Modify: `crates/phplumb-index/src/lib.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `builtins` (Task 1), `magic` and `guarded` (Task 2), `find_member`, `all_subtypes`, `supertypes`, name resolution, `infer::member_accesses_named`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity { Error, Warning }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub start: u32, // the name
    pub end: u32,
    pub severity: Severity,
    pub message: String,
}

impl Index {
    /// Broken references in `source`, the content of the file at `path`, sorted by
    /// offset. Every rule must prove the target missing; uncertainty stays silent.
    pub fn diagnostics(&self, path: &Path, source: &[u8]) -> Vec<Diagnostic>;
}
```

Rules, each silent when the name is in the file's `guarded` list:

1. **Class names** (`new`, `X::member` but never `X::class`, `instanceof`, `catch`, `extends`, `implements`, trait `use`, parameter, return and property types): flagged when the resolved name is neither indexed nor built in. `self`, `static`, `parent`, attributes and PHPDoc types are never checked.
2. **Functions:** flagged when neither the resolved name nor, for an unqualified name in a namespace, the global name is indexed or built in.
3. **Constants:** the same rule as functions, with constant case rules.
4. **Methods and class constants through a named class, `self::` or `parent::`:** flagged when the class and all its ancestors are indexed, none is magic (methods only), and none declares the member.
5. **Instance methods (`->`, `?->`) and `static::` members:** flagged when the receiver resolves, every receiver class is indexed and is not a trait, and for each receiver class neither it, its ancestors nor any of its subtypes declares the member, none of them is magic, and none has an ancestor missing from the index. Unknown and anonymous receivers stay silent.
6. **Enums** always have `cases()`, and backed enums `from()` and `tryFrom()`.

The fixture runner gets `run_diagnostics_fixture(name)`: a `/*@broken*/` marker before a name means that name must be flagged; the set of flagged names must match the markers exactly.

- [ ] **Step 1: Write the failing fixture** `diagnostics` with files for each rule above and its silent counterpart: missing class in each position and through an import, `X::class` on a missing class, attributes and PHPDoc with missing classes, guarded uses of each kind, built-in classes, functions and constants (`\DateTime`, `strlen`, `PHP_EOL`), namespaced and global functions and constants, an invented method on a typed parameter, a method only a subtype declares, `__call`, `__callStatic`, `@method`, `@mixin`, a class extending `\Exception` calling `getMessage()`, `parent::__construct()` without a parent constructor, a missing static method, a missing class constant and enum case, enum `cases()` and `from()`, `$this` in an abstract class and in a trait, a union receiver where one member declares the method, an unknown receiver.

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-index --test fixtures diagnostics`. Expected: FAIL to compile, `diagnostics` not found.

- [ ] **Step 3: Implement `diagnostics`.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): find broken references`.

### Task 4: Server, extension and refresh

**Files:**

- Modify: `crates/phplumb-server/src/lsp.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/package.json`, `editors/vscode/src/test/definition.test.ts`
- Create: `crates/phplumb-server/tests/lsp_diagnostics.rs`, `editors/vscode/test-fixtures/basic/src/Broken.php`

**Interfaces:**

- Consumes: `Index::diagnostics` (Task 3).
- Produces: capability `diagnosticProvider` `{ interFileDependencies: true, workspaceDiagnostics: false }`. `textDocument/diagnostic` answers a full report: the file's diagnostics when it is project code, diagnostics are enabled and the index is ready; an empty list otherwise. After the initial index and after each applied batch of file changes, the server sends `workspace/diagnostic/refresh` when the client supports it. `initializationOptions.diagnostics` carries `phplumb.diagnostics.enabled`; changing it restarts the server, like `phplumb.exclude`. The extension returns an empty report for documents with unsaved edits and pulls on open, change and save.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn diagnostics_over_stdio()            // src/Use.php "new Missing();" -> one diagnostic, message "Undefined class 'App\\Missing'", severity 1, range on "Missing"
#[test] fn diagnostics_skip_vendor_and_core()  // the same code under vendor/acme/lib/ (autoloaded) and web/core/ -> empty
#[test] fn diagnostics_refresh_after_changes() // client with refreshSupport; create src/Missing.php declaring the class, send didChangeWatchedFiles -> the server sends workspace/diagnostic/refresh, and a new pull returns no diagnostics
#[test] fn diagnostics_can_be_disabled()       // initializationOptions {"diagnostics": false} -> empty
```

```ts
test("broken references are reported", ...) // open src/Broken.php: vscode.languages.getDiagnostics(uri) eventually has "Undefined class 'App\\Ghost'"
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-server --test lsp_diagnostics`. Expected: FAIL.

- [ ] **Step 3: Implement the handler, the refresh, the setting and the middleware.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: report broken references`.

### Task 5: False alarm audit on the reference project

**Files:**

- Modify: `crates/phplumb-server/src/bench.rs`, `crates/phplumb-server/tests/bench.rs`, `docs/benchmarks.md`

**Interfaces:**

- Produces: bench option `--diagnostics <dir>`: runs `diagnostics` on every indexed file below `<dir>` (ignoring the project-code scope) and adds `diagnostics: { files, count, slowest_file_ms, entries }` to the report, where `entries` lists `path:line:column message`.

- [ ] **Step 1: Write the failing test** `bench_reports_diagnostics` on the `diagnostics` fixture: `report["diagnostics"]["count"]` equals the number of `/*@broken*/` markers and `entries` is non-empty.

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-server --test bench`. Expected: FAIL.

- [ ] **Step 3: Implement the option.**

- [ ] **Step 4: Audit.** Run the release bench with `--diagnostics web/modules/custom`, then with `--diagnostics web/core` and `--diagnostics web/modules/contrib`. Check every custom diagnostic by hand against the project: each one must be a real missing target. A false alarm is a bug: find its class, add a fixture case for it to Task 3's fixture, fix the rule and rerun. For core and contrib, classify every diagnostic (optional dependency, test-only code, real bug, false alarm) and fix every false alarm class the same way. Expected: zero false alarms in custom code; `slowest_file_ms` under 500; cold index under 10000; `peak_rss_mb` under 1024; `cases_failed` 0.

If a target is missed, stop and report the numbers to the user.

- [ ] **Step 5: Add a dated section to `docs/benchmarks.md`** with the counts and the classification, no client names, and commit with message `feat(server): audit diagnostics on a project`.
