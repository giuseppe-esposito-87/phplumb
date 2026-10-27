# PHPlumb 4f: broken Drupal references implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flag service ids, route names and plugin ids written in PHP that are defined nowhere on disk, with no false alarm on the reference project.

**Architecture:** The Drupal registries also keep what test modules define, apart from what navigation uses. Ids defined in PHP (services registered by service providers and compiler passes, routes built in route subscribers and route callbacks, plugins added with `setDefinition()`) are collected conservatively: every string literal of the files that can define them counts as defined. A new framework entry point adds diagnostics to a PHP file; the Drupal module checks the literal ids of the contexts steps 4a, 4d and 4e already navigate.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, item 7: "A broken Drupal reference is flagged only when the id is defined nowhere on disk, test modules included"; anti-goal: false alarms).

**Scope:** Last part of delivery step 4. Out of scope: ids in YAML files, entity type ids, hook names, ids built at run time, and ids passed through variables.

## Global constraints

- Everything in the earlier plans' global constraints still holds, and step 3's rule: a diagnostic appears only when the engine is certain; any doubt stays silent. Diagnostics cover project code only, as in step 3.
- **Registration point:** core gains one framework-agnostic entry point; Drupal knowledge lives in `frameworks/drupal`.
- **Defined anywhere:** what navigation reads, plus test modules, plus:
  - services: every string literal of a PHP file that mentions `Symfony\Component\DependencyInjection\ContainerBuilder`, `Drupal\Core\DependencyInjection\ServiceProviderInterface`, `ServiceModifierInterface` or `Symfony\Component\DependencyInjection\Compiler\CompilerPassInterface`; and the synthetic `service_container`, `kernel` and `class_loader`;
  - routes: every string literal of a PHP file that mentions `Symfony\Component\Routing\Route` or `RouteCollection`;
  - plugins: every string literal of a PHP file that calls a method named `setDefinition`.
- **Contexts checked** (literal ids only; the diagnostic covers the string):

  | Kind    | Contexts                                                                                                                                                     | Not flagged                                                                                           |
  | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
  | Service | `\Drupal::service(id)`; `get(id)` on `Symfony\Component\DependencyInjection\ContainerInterface` or `Drupal\Component\DependencyInjection\ContainerInterface` | the file calls `has(id)` or `\Drupal::hasService(id)` with the same id                                |
  | Route   | the route call table of step 4e                                                                                                                              | `<front>`, `<none>`, `<nolink>`, `<current>`, `<button>`; names starting with `entity.` or `view.`    |
  | Plugin  | the plugin id contexts of step 4d (call arguments, `#type`, display option types)                                                                            | the file calls `hasDefinition(id)` with the same id; a manager that declares its own `getDiscovery()` |

- **Messages:** `Undefined service 'x'`, `Undefined route 'x'`, `Undefined plugin 'x' of FieldTypePluginManager` (the manager's short name), with step 3's severity.
- **Targets on the reference project:** no Drupal diagnostic in `web/modules/custom` except ones checked by hand as real; every Drupal diagnostic in `web/core` and `web/modules/contrib` checked by hand, and any false alarm fixed by a rule recorded in the ledger; diagnostics of one file under 50 ms; cold index under 10 seconds; memory under 1 GB.

## Review focus

- **A service registered by a service provider** (`$container->register('mymodule.dynamic', ...)`) is not flagged. Test in Task 2.
- **A route built in a route subscriber or a route callback** is not flagged; `entity.node.canonical` is not flagged. Test in Task 2.
- **A constraint registered with `setDefinition()`** (`NotBlank`) is not flagged. Test in Task 2.
- **An id defined only in a test module** is not flagged. Test in Task 2.
- **A derivative plugin id** (`menu_block:main`) is not flagged. Test in Task 2.

---

### Task 1: Definitions anywhere

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/{mod,services,routes,plugins}.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
impl Drupal {
    /// Whether the service `id` is defined anywhere on disk, by the rules of the
    /// global constraints.
    pub fn defines_service(&self, index: &Index, id: &str) -> bool;
    pub fn defines_route(&self, index: &Index, name: &str) -> bool;
    /// Whether `manager` has a plugin `id` (or its base, for `base:derivative`).
    pub fn defines_plugin(&self, index: &Index, manager: &str, id: &str) -> bool;
}
```

The literal sets from PHP files are computed on first use and emptied on every update.

- [ ] **Step 1: Write the failing tests** on the fixture, extended with a services file, a routing file and a block plugin in the test module, a service provider registering `mymodule.dynamic`, a route subscriber adding `mymodule.dynamic_route`, and the `ConstraintManager` stub registering `NotBlank` with `setDefinition()`:
  - Each of these is defined; `nowhere.service`, `nowhere.route` and the block `nowhere_block` are not.
  - Navigation still ignores the test module's definitions (the 4a, 4d and 4e tests pass unchanged).

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the sets.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): know every drupal id defined on disk`.

### Task 2: Drupal diagnostics

**Files:**

- Modify: `crates/phplumb-index/src/diagnostics.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/*.rs`, `crates/phplumb-index/src/infer.rs` (argument ranges), `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/**`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `Drupal::defines_*` (Task 1).
- Produces:

```rust
pub trait Framework {
    // existing methods
    /// Broken references the framework finds in a PHP file.
    fn diagnostics(&self, index: &Index, path: &Path, source: &[u8]) -> Vec<Diagnostic>;
}
```

`Index::diagnostics` adds them to step 3's, sorted by offset. `FoundCall` gains the byte ranges of its string literal arguments, and `infer` lists a file's keyed strings with their ranges, for the contexts of the global constraints.

- [ ] **Step 1: Write the failing tests** on a fixture file in `mymodule` using every context:
  - Flagged, each with its message and the string's range: an unknown service in `\Drupal::service()` and in `$container->get()`, an unknown route in `Url::fromRoute()`, an unknown `#type`, an unknown field type in `BaseFieldDefinition::create()`, an unknown widget in display options.
  - Not flagged: every Review focus case, `entity.node.canonical`, `<front>`, a service guarded by `$container->has('x')`, an id in a variable, a `get()` on a non-container receiver.
  - A VS Code test: an unknown service id in the `basic` fixture is reported.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement it.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: flag broken drupal references`.

### Task 3: False-alarm sweep, benchmarks and README

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/*.rs` (only for rules the sweep requires, each with a test), `docs/benchmarks.md`, `README.md`

- [ ] **Step 1: Run the diagnostics sweep** on `web/modules/custom`, `web/modules/contrib` and `web/core`: `./target/release/phplumb bench --root <reference-project> --diagnostics <dir>`. List every Drupal diagnostic.

- [ ] **Step 2: Check each one by hand.** A real one stays. A false alarm gets a rule (with a failing test first), recorded in the ledger as a ruling. Repeat Steps 1 and 2 until no false alarm is left. If a false alarm needs a rule that would hide real errors broadly, stop and report it to the user.

- [ ] **Step 3: Run the reference benchmark.** Expected: `cases_failed` 0; resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024; slowest diagnostics file under 50 ms.

- [ ] **Step 4: Add a dated section to `docs/benchmarks.md`** (counts per area, real findings by kind), move Broken Drupal references to the available Drupal features in `README.md`, and commit with message `docs: record drupal broken reference results`.
