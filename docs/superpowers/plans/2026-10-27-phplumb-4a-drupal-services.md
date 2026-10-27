# PHPlumb 4a: Drupal services implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Type `\Drupal::service('id')` and `$container->get('id')` from every `*.services.yml` on disk, so navigation continues on the next call, and open a service's YAML definition with Cmd+click on its id.

**Architecture:** A framework registration point in the index: each framework implements one trait that reads its own non-PHP files, types calls the declarations cannot, and resolves string arguments to targets. Inference asks the active frameworks before falling back to a call's declared return type; go to definition asks them when the cursor is in a string argument. The Drupal framework is the first implementation. It is active when `composer.lock` requires `drupal/core`, or when the user forces it. Its services registry parses every `*.services.yml` outside `vendor` and test modules with a YAML parser that keeps positions, and resolves a service id to its classes through aliases, parents, class-named ids and decorators.

**Tech Stack:** existing workspace; `yaml-rust2` 0.13 (event parser with positions).

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, item 1; constraints on framework modules; framework activation).

**Scope:** First part of delivery step 4. Entities (4b), hooks (4c), plugins (4d), routes and YAML navigation (4e) and broken Drupal references (4f) have their own plans.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, no client names or paths in committed files, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Registration point:** core navigation code calls frameworks only through `frameworks::Framework` and the list `frameworks::registered()`. Adding a framework means a new module and one line in that list.
- **Activation:** setting `phplumb.frameworks.drupal`, `auto` (default), `on` or `off`. `auto` turns the module on when `<root>/composer.lock` contains `"name": "drupal/core"`. Changing it restarts the server.
- **Registry scope:** every `*.services.yml` under the root except those with a `vendor` or `tests` path component, those `phplumb.exclude` matches, and hidden or `node_modules` directories.
- **Wrong target rule:** a service id resolves to every class any definition on disk can give it (several modules defining the same id, decorators): a union, never a guess. An unknown id leaves the call's declared type in place.
- **Targets on the reference project:** member coverage in `web/modules/custom` rises above the 90.7% baseline; cold index under 10 seconds; memory under 1 GB.

## Review focus

- **Alias chains and cycles:** `a: '@b'`, `b: { alias: c }`, `c: { class: X }` gives `X`; a cycle gives nothing and terminates. Test in Task 2.
- **Parents and class-named ids:** a child without `class` takes its parent's; `Drupal\x\Foo: ~` and `Drupal\x\Foo: { autowire: true }` give `Drupal\x\Foo`. Test in Task 2.
- **Decorators:** a service decorated by another resolves to both classes. Test in Task 2.
- **A container typed through an implementation:** `$container->get('x')` works when `$container` is a Drupal container class implementing Symfony's `ContainerInterface`, and through `\Drupal::getContainer()`. Test in Task 3.
- **A services file that changes, appears or disappears** updates the registry without a restart. Test in Task 4.

---

### Task 1: Registration point and activation

**Files:**

- Create: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Modify: `crates/phplumb-index/src/lib.rs`, `crates/phplumb-index/src/discover.rs`, `crates/phplumb-index/src/index.rs`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
// phplumb_index::frameworks
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activation { #[default] Auto, On, Off }

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FrameworkSettings { pub drupal: Activation }

/// What a framework sees of a call: what it targets and its literal arguments.
pub struct FrameworkCall<'a> {
    /// `Class::method` for every declaration the call resolves to, or the function.
    pub targets: &'a [CallTarget],
    /// Positional arguments that are string literals or `X::class`, by position.
    pub strings: &'a [Option<String>],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallTarget { Method { class: String, name: String }, Function(String) }

pub trait Framework: Send + Sync {
    /// Whether the framework applies to the project at `root`.
    fn detect(&self, root: &Path) -> bool;
    /// Reads the framework's own files below the filter's root.
    fn rebuild(&mut self, filter: &PathFilter);
    /// Applies changed and removed paths (files or directories, any extension).
    fn update(&mut self, filter: &PathFilter, changed: &[PathBuf], removed: &[PathBuf]);
    /// The type `call` returns when the framework knows better than its declaration.
    fn call_type(&self, index: &Index, call: &FrameworkCall<'_>) -> Option<TypeExpr>;
    /// Targets of the string literal at position `argument` of `call`.
    fn string_targets(&self, index: &Index, call: &FrameworkCall<'_>, argument: usize, value: &str) -> Vec<Location>;
}

/// Every framework the engine knows, each with its setting.
pub fn registered(settings: &FrameworkSettings) -> Vec<(Activation, Box<dyn Framework>)>;
```

`DiscoverOptions` gains `pub frameworks: FrameworkSettings`. `Index` keeps the active frameworks (`On`, or `Auto` and `detect`), rebuilds them after the PHP index, passes every changed and removed path of `update` to them, and exposes `pub fn active_frameworks(&self) -> Vec<&'static str>` (names, for the log).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn drupal_is_detected_from_composer_lock() // composer.lock with "name": "drupal/core" -> active_frameworks() == ["drupal"]
#[test] fn drupal_stays_off_without_drupal_core()  // composer.lock without it -> []; no composer.lock -> []
#[test] fn drupal_can_be_forced()                  // no composer.lock, drupal: On -> ["drupal"]; with drupal/core, drupal: Off -> []
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the trait, the list, the settings and the Drupal module's `detect`** (the other methods do nothing yet).

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): add a framework registration point`.

### Task 2: Services registry

**Files:**

- Create: `crates/phplumb-index/src/frameworks/drupal/services.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Modify: `Cargo.toml`, `crates/phplumb-index/Cargo.toml`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
// phplumb_index::frameworks::drupal
pub struct ServiceDefinition {
    pub path: PathBuf,
    pub start: u32, pub end: u32, // the id key in the YAML file
    pub class: Option<String>, pub alias: Option<String>,
    pub parent: Option<String>, pub decorates: Option<String>,
}
impl Drupal {
    /// Every definition of `id` on disk.
    pub fn services(&self, id: &str) -> &[ServiceDefinition];
    /// The classes a service id can give: through aliases, parents, an id that names
    /// an indexed class, and every service decorating it. Empty when unknown.
    pub fn service_classes(&self, index: &Index, id: &str) -> Vec<String>;
}
```

`Index` exposes `pub fn drupal(&self) -> Option<&Drupal>` for tests and the bench. The fixture `drupal` is a small Drupal tree: `composer.lock`, `core/lib/Drupal.php` (`class Drupal` with `service()` and `getContainer()`), a Symfony `ContainerInterface` and a Drupal `Container` implementing it, `core/core.services.yml`, and two custom modules with services files, one inside a `tests` directory.

- [ ] **Step 1: Write the failing tests** on the fixture: definitions with their key positions; short alias `'@b'`, `alias:`, alias chains and a cycle; `parent`; class-named ids (`~` and a mapping without `class`); `decorates`; the same id in two modules; services files in `tests` and `vendor` ignored; `_defaults` and `parameters` ignored.

- [ ] **Step 2: Run them and watch them fail.** Expected: FAIL to compile.

- [ ] **Step 3: Implement the registry and `rebuild`/`update`** (re-parse changed services files; drop removed files and directories).

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): read drupal services`.

### Task 3: Typing service calls

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Test: `crates/phplumb-index/tests/fixtures/drupal/**`, `crates/phplumb-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `Framework::call_type`, `service_classes` (Tasks 1 and 2).
- Produces: inference builds a `FrameworkCall` for method, null-safe, static and function calls and uses the first framework answer before the declared return type. The Drupal module answers `\Drupal::service(id)` and `get(id)` on any class that is or implements `Symfony\Component\DependencyInjection\ContainerInterface` or `Psr\Container\ContainerInterface`, with the union of `service_classes`; an id given as `X::class` counts as the class name.

- [ ] **Step 1: Write the failing fixture cases** (cursor markers, `run_fixture("drupal")`): a method call after `\Drupal::service('id')`, after `$container->get('id')` with `$container` typed by Symfony's interface, by the Drupal container class and through `\Drupal::getContainer()`, after an alias, after a decorated service (both targets), after `get(Foo::class)`, and an unknown id (nothing).

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-index --test fixtures drupal`. Expected: FAIL.

- [ ] **Step 3: Implement the inference hook and the Drupal `call_type`.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): type drupal service calls`.

### Task 4: Service ids to YAML, server and extension

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-server/src/lsp.rs`, `editors/vscode/package.json`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`
- Create: `crates/phplumb-server/tests/lsp_drupal.rs`, `editors/vscode/test-fixtures/basic/...` files the extension test needs

**Interfaces:**

- Consumes: `Framework::string_targets`.
- Produces: `definition` on a string argument asks the active frameworks; the Drupal module answers the service id of `\Drupal::service()` and container `get()` with every definition's key. The server reads `frameworks` from `initializationOptions` (`{"drupal": "auto"}`), logs the active frameworks after indexing, watches `**/*.yml`, and passes those events to the index. The extension adds the `phplumb.frameworks.drupal` setting and restarts the server when it changes.

- [ ] **Step 1: Write the failing tests**

```rust
// fixture: cursor inside 'mymodule.greeter' in \Drupal::service('mymodule.greeter') -> the key in mymodule.services.yml (and both keys for an id defined twice)
#[test] fn service_definition_over_stdio()   // definition on the id string -> uri ends with mymodule.services.yml, range on the key
#[test] fn services_follow_yaml_changes()    // add a service to a services file, send didChangeWatchedFiles, the call after it now resolves
#[test] fn registers_a_yaml_watcher()        // the registration has a **/*.yml watcher
```

```ts
test("Cmd+click on a service id opens its YAML", ...) // executeDefinitionProvider on the id string in a fixture file
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-server --test lsp_drupal`. Expected: FAIL.

- [ ] **Step 3: Implement string definitions, the settings, the watcher and the update path.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate drupal services`.

### Task 5: Reference cases and benchmarks

**Files:**

- Modify: `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Add at least 5 cases**, each checked by hand: a method call after `\Drupal::service()` in a `.module` file, after `$container->get()` in a `create()` method, through an alias, a service id string to its YAML key, and an unknown id that expects nothing.

- [ ] **Step 2: Run the reference benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0; coverage above 90.7%; cold index under 10000; `peak_rss_mb` under 1024. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 3: Add a dated section to `docs/benchmarks.md`** and commit with message `docs(benchmarks): record drupal services results`.
