# PHPlumb 5a: built-in class members implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve PHP's built-in classes, functions, constants and their members from JetBrains phpstorm-stubs, so `$exception->getMessage()` and `$date->modify('+1 day')->format('Y')` lead to a declaration and navigation continues on the next call.

**Architecture:** The stubs are vendored in the repository and shipped next to the engine binary. The engine indexes them once per process into a separate stub `Index`, summarized for PHP 8.5: declarations not available in 8.5 are dropped, duplicates keep the last one, and `LanguageLevelTypeAware` return types are applied. The project index sends every lookup of a built-in name (one listed in `builtins.txt`) to the stub index, so member lookup, inheritance, return types, definition and hover work unchanged. Every other feature leaves stub locations out.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, goal "Core PHP 1", delivery step 5a. The earlier spec `2026-10-26-php-navigator-design.md` still applies.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Stubs source:** JetBrains phpstorm-stubs tag `v2026.2`, commit `709e512210784a7c0a677b3a89d35def844a59b9`, Apache License 2.0, with its `LICENSE` kept beside the files.
- **One target per built-in member or function:** the declaration available in PHP 8.5.
- **Stubs are for definition, hover and types only.** References, implementations, type hierarchy, call hierarchy and workspace symbols never list a stub location. References of a built-in symbol return nothing.
- **A built-in name resolves to its stub alone,** even when project or `vendor` code also declares it, as polyfills do.
- **Diagnostics do not change.** No new broken-reference diagnostic may appear on either reference project.
- **Without stubs** (no stub directory found), every result is the same as before this step.
- **Targets on the Drupal reference project:** member coverage in `web/modules/custom` at least 42,455; cold index under 10 seconds; warm start under 2 seconds; `peak_rss_mb` under 1024; every query case under 1 second; every case passes.
- **Repository hygiene:** no client code and no reference project path in the repository. Code comments describe the current code only, never its history.

## Review focus

- **A vendor polyfill of a built-in class,** such as a global `class Stringable` outside the stubs: go to definition on `\Stringable` gives only the stub. Test in Task 2.
- **A member the stubs declare once per PHP version,** such as `DateTimeImmutable::modify`: exactly one target, the one available from 8.3. Tests in Task 1 and Task 2.
- **A project class extending a built-in class** calls an inherited method (`$this->getMessage()`) or `parent::__construct()`: both resolve to the stub. Test in Task 2.
- **A namespaced project function with a built-in name,** such as `App\Local\count()`: a call inside that namespace leads to the project function, not the stub. Test in Task 2.
- **A stub target checked for staleness** on every definition request: the stub file never enters the project index. Test in Task 2.

---

### Task 1: Stub summaries for PHP 8.5

**Files:**

- Create: `crates/phplumb-index/src/stubs.rs`
- Modify: `crates/phplumb-index/src/lib.rs` (add `pub mod stubs;`)
- Test: `crates/phplumb-index/tests/stubs.rs`

**Interfaces:**

- Produces: `pub const PHP_VERSION: (u32, u32) = (8, 5);` in `stubs.rs`.
- Produces: `pub fn summarize_stub(source: &[u8]) -> FileSummary` in `stubs.rs`: `summarize`, then:
  - drops every function, class-like and method whose `JetBrains\PhpStorm\Internal\PhpStormStubsElementAvailable` attribute excludes `PHP_VERSION`. The bounds are `from` and `to`, named or positional at 0 and 1, inclusive, compared as `(major, minor)`; a missing bound is open;
  - keeps, among the remaining declarations, one function per fully qualified name and one member per kind and lowercased name within a class: the last one in the file;
  - when a function or method has `JetBrains\PhpStorm\Internal\LanguageLevelTypeAware`, uses as its declared return type the entry of the first argument's version map with the highest version not above `PHP_VERSION`, else the `default` argument when it is not empty. The PHPDoc type still wins when it names a class, as `summary::pick_type` decides for native types.

- [ ] **Step 1: Write the failing tests** in `crates/phplumb-index/tests/stubs.rs`. Each summarizes an inline stub source that imports both attributes from `JetBrains\PhpStorm\Internal`:
  - `stub_members_keep_the_php_85_declaration`: a class `D` with `modify()` marked `from: '5.5', to: '8.2'`, a second `modify()` marked `from: '8.3'`, and `gone()` marked `to: '7.4'`. The summary has one `modify`, whose `name_start` is the second declaration's, and no `gone`.
  - `stub_functions_keep_the_php_85_declaration`: `f()` marked `from: '8.0'`, then `f()` marked `from: '5.3', to: '7.4'`. The summary has one `f`, the first.
  - `positional_availability_bounds_count`: `#[PhpStormStubsElementAvailable('8.6')]` on `h()` drops it.
  - `language_level_types_pick_the_newest_version_up_to_85`: `#[LanguageLevelTypeAware(['8.4' => 'D', '8.6' => 'E'], default: 'static|false')]` on a method without native return type. Its `ty` equals the `ty` that `summarize` gives a method declared `: D`.
  - `language_level_types_fall_back_to_default`: the map `['8.6' => 'E']` with `default: 'D'` gives the same `ty` as `: D`.
  - `unmarked_stub_declarations_stay`: a class and method without attributes are kept as `summarize` gives them.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test stubs`. Expected: FAIL, `summarize_stub` not found.

- [ ] **Step 3: Implement `summarize_stub`** as specified above. Versions parse from the first two dot-separated numbers of the string.

- [ ] **Step 4: Run the tests.** Run: `cargo test -p phplumb-index --test stubs`. Expected: PASS.

- [ ] **Step 5: Commit** with message `feat(index): summarize phpstorm stubs for PHP 8.5`.

### Task 2: Stub index and built-in lookups

**Files:**

- Create: `crates/phplumb-index/data/stubs/**` (vendored stubs and `LICENSE`)
- Modify: `crates/phplumb-index/src/stubs.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/discover.rs`
- Test: `crates/phplumb-index/tests/stubs.rs`, `crates/phplumb-index/tests/fixtures/stubs/**`

**Interfaces:**

- Consumes: `summarize_stub` (Task 1); `builtins::is_class`, `builtins::is_function`, `builtins::is_constant`.
- Produces: `pub fn default_dir() -> Option<PathBuf>` in `stubs.rs`: the first existing directory among `$PHPLUMB_STUBS`, `<directory of std::env::current_exe()>/stubs`, and `concat!(env!("CARGO_MANIFEST_DIR"), "/data/stubs")`.
- Produces: `pub(crate) fn load(dir: &Path) -> Option<Arc<Index>>` in `stubs.rs`: the stub index of `dir`, built once per process and directory, and `None` when the directory holds no PHP file.
- Produces: `DiscoverOptions::stubs: Option<PathBuf>`, defaulting to `stubs::default_dir()`.
- Produces: `pub fn is_stub(&self, path: &Path) -> bool` and `pub fn stubs_dir(&self) -> Option<&Path>` on `Index`.
- Produces: a private `fn holder(&self, kind: SymbolKind, fqn: &str) -> &Index` on `Index`: the stub index for a built-in name when stubs are loaded, `self` otherwise.

- [ ] **Step 1: Vendor the stubs.** Clone tag `v2026.2` into a scratch directory and check that `git rev-parse HEAD` prints `709e512210784a7c0a677b3a89d35def844a59b9`. Then copy every `*.php` file outside its top-level `tests` directory, keeping relative paths, plus `LICENSE`, into `crates/phplumb-index/data/stubs`. Check: `find crates/phplumb-index/data/stubs -name '*.php' | wc -l` prints `563`.

- [ ] **Step 2: Write the failing tests.** Create the fixture `crates/phplumb-index/tests/fixtures/stubs`:
  - `app/Uses.php` in namespace `App`, holding:
    - a `catch (\RuntimeException $e)` that calls `$e->getMessage()`;
    - `(new \DateTimeImmutable())->modify('+1 day')->format('Y')`;
    - an unqualified `count([])`;
    - `\PHP_EOL`;
    - a reference to `\Stringable`;
    - a class `AppException extends \RuntimeException` whose method calls `$this->getMessage()` and whose constructor calls `parent::__construct()`.
  - `app/Local.php` in namespace `App\Local`, declaring `function count()` and calling `count()`.
  - `lib/Stringable.php`, declaring a global `class Stringable {}`.

  The tests in `tests/stubs.rs` find cursors by searching the source text. A stub location is described by its path relative to `index.stubs_dir()` and the text at its range:
  - `built_in_method_opens_its_stub`: `getMessage` on `$e` gives exactly one location, `Core/Core_c.php` with text `getMessage`.
  - `inherited_built_in_members_resolve`: `$this->getMessage()` gives the same location; `__construct` in `parent::__construct()` gives one location in `Core/Core_c.php`.
  - `built_in_return_types_continue_the_chain`: `format` gives exactly one location, in `date/date_c.php`.
  - `version_duplicates_give_one_target`: `modify` gives exactly one location, the declaration whose attributes say `from: '8.3'`.
  - `built_in_function_and_constant_open_stubs`: `count` in `Uses.php` gives one stub location with text `count`; `PHP_EOL` gives one stub location with text `PHP_EOL`.
  - `project_function_wins_in_its_namespace`: `count()` in `Local.php` gives the declaration in `app/Local.php`.
  - `polyfill_of_a_built_in_class_is_not_a_target`: `Stringable` gives one location, under the stubs directory.
  - `hover_shows_the_stub_signature`: hover on `getMessage` contains `getMessage(): string`.
  - `stub_files_never_enter_the_project_index`: after the definition requests above, `index.declarations()` yields no path under the stubs directory.
  - `without_stubs_built_ins_resolve_to_nothing`: built with `DiscoverOptions { stubs: None, ..Default::default() }`, `getMessage` and `count` in `Uses.php` give no location, and `Stringable` gives `lib/Stringable.php`, as before this step.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test stubs`. Expected: the new tests FAIL.

- [ ] **Step 4: Implement the stub index and the lookups.**
  - `Index` gets `stubs: Option<Arc<Index>>`, set in `build_from` from `opts.stubs` through `stubs::load`. The stub index itself has none and no frameworks, and reads every `*.php` below its directory with `summarize_stub`.
  - `holder` decides every lookup of a class-like, function or constant key: `classes_named`, `has_class`, `function_returns`, and the class, function and constant arms of `resolve`, where entries are read from the holder's `files`. Global fallback for unqualified names checks the project's namespaced name first, as today.
  - The staleness check in `definition_with` skips stub locations.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass. An existing fixture cursor on a built-in name that expected nothing now fails: move that expectation into `tests/stubs.rs` as a stub location and remove the cursor.

- [ ] **Step 6: Commit** with message `feat(index): resolve built-in symbols from phpstorm stubs`.

### Task 3: Keep stubs out of the other features

**Files:**

- Modify: `crates/phplumb-index/src/index.rs`, `crates/phplumb-server/src/lsp.rs`
- Test: `crates/phplumb-index/tests/stubs.rs`, `crates/phplumb-server/tests/lsp_hierarchy.rs`, `crates/phplumb-server/tests/lsp_symbols.rs`

**Interfaces:**

- Consumes: `Index::is_stub`, the `stubs` fixture (Task 2).

- [ ] **Step 1: Write the failing tests:**
  - `references_of_a_built_in_are_empty` in `tests/stubs.rs`: references on `getMessage`, with and without `include_declaration`, are empty.
  - `implementations_leave_stubs_out`: implementations on `RuntimeException` in `AppException extends \RuntimeException` list no stub path.
  - In `lsp_hierarchy.rs`, following its existing tests on the `stubs` fixture:
    - the type hierarchy supertypes of `AppException` list no stub path;
    - the outgoing calls of the method calling `$this->getMessage()` list no stub path.
  - In `lsp_symbols.rs`: a workspace symbol query `RuntimeException` on the `stubs` fixture lists no stub path.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test --workspace --test stubs --test lsp_hierarchy --test lsp_symbols`. Expected: the new tests FAIL where a stub location leaks.

- [ ] **Step 3: Filter stub locations** out of the results of `Index::references`, `Index::implementations`, `Index::incoming_calls` and `Index::outgoing_calls`, and out of the supertypes the type hierarchy handler in `lsp.rs` returns. Skip the reference scan when every target is a stub location.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `fix(index): keep stub declarations out of references and hierarchies`.

### Task 4: Packaging, benchmark output and docs

**Files:**

- Modify: `editors/vscode/scripts/package.mjs`, `crates/phplumb-server/src/bench.rs`, `crates/phplumb-index/data/README.md`, `docs/development.md`
- Test: `crates/phplumb-server/tests/bench.rs`

**Interfaces:**

- Consumes: `Index::stubs_dir`, `stubs::default_dir` (Task 2).

- [ ] **Step 1: Write the failing test** in `tests/bench.rs`: a definition case on the `stubs` fixture at the `getMessage` call, expecting `["stubs/Core/Core_c.php:<line>:<col>"]` with the position of `getMessage` in the vendored file, passes.

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-server --test bench`. Expected: FAIL, the target renders as an absolute path.

- [ ] **Step 3: Render stub locations** in `query_case` as `stubs/` followed by the path relative to the stubs directory.

- [ ] **Step 4: Ship the stubs.** `package.mjs` replaces `editors/vscode/bin/stubs` with a copy of `crates/phplumb-index/data/stubs` for every target, beside `bin/phplumb`.

- [ ] **Step 5: Update the docs.**
  - `data/README.md` says that `stubs/` holds the stub files of the same tag and that the engine reads them for built-in declarations.
  - The "Built-in PHP names" section of `docs/development.md` explains how to refresh `data/stubs` together with `builtins.txt` from a newer tag, with the copy rule from Task 2 Step 1.
  - Run the Markdown formatter on both files.

- [ ] **Step 6: Run the checks.** Run: `cargo test --workspace && npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 7: Check the package.** Run: `npm --prefix editors/vscode run package -- darwin-arm64 && unzip -l editors/vscode/phplumb-darwin-arm64-0.3.0.vsix | grep -c 'extension/bin/stubs/.*\.php'`. Expected: `563`.

- [ ] **Step 8: Commit** with message `build(vscode): ship phpstorm stubs with the engine`.

### Task 5: Reference cases and benchmarks

**Files:**

- Modify: `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`
- Create outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Update the Drupal cases.** Back up `reference.toml` as `reference.toml.bak-5a`. Then:
  - change `nothing: PHP built-in function count()` to expect its stub location, checked by hand;
  - add cases, each checked by hand, for `getMessage()` on a caught exception and for a method after a `DateTimeImmutable` call in custom code.

- [ ] **Step 2: Write the Symfony cases.** Write at least 4 cases in `symfony.toml`, each checked by hand in `<symfony-reference-project>/src`:
  - `getMessage()` on a caught exception;
  - `format()` on a `DateTimeImmutable`;
  - an unqualified `count()` in a namespace;
  - a project method reached through an interface, which must keep its current target.

- [ ] **Step 3: Run the Drupal benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom --cache "$(mktemp -d)/index.bin"`. Expected:
  - `cases_failed` 0;
  - resolved accesses at least 42,455;
  - `cold_index_ms` under 10000 and `warm_start_ms` under 2000;
  - `peak_rss_mb` under 1024;
  - every query timing under 1000.

  If a target is missed, stop and report the numbers to the user.

- [ ] **Step 4: Run the Symfony benchmark.** Run: `./target/release/phplumb bench --root <symfony-reference-project> --cases "$HOME/.local/share/phplumb/cases/symfony.toml" --coverage src`. Expected: `cases_failed` 0, and resolved accesses above 12,568 of 13,211.

- [ ] **Step 5: Run the diagnostics sweep** on the Symfony project's `src` and the Drupal project's `web/modules/custom` with `bench --diagnostics`. Expected: the same diagnostics as before this step. Compare against a run of the previous release build.

- [ ] **Step 6: Record the results.** Add a dated section `2026-10-28, step 5a built-in class members` to `docs/benchmarks.md` with one table per reference project. Change its opening sentence to name both reference projects. Run the Markdown formatter.

- [ ] **Step 7: Commit** with message `docs(benchmarks): record built-in stub results`.

- [ ] **Step 8: Ask the user to check in VS Code:** install the darwin-arm64 `.vsix` in the `PHPlumb` profile, open the Symfony reference project, and Cmd+click `getMessage()`. It opens `Core_c.php` from the extension's `bin/stubs`.
