# PHPlumb 4e: Drupal routes and YAML navigation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a route name in PHP (`Url::fromRoute('name')` and similar calls) opens the route in `*.routing.yml`. Inside services, routing and links YAML files, Cmd+click on a class, a method, a service or a route opens its definition, so a route leads on to its controller or form.

**Architecture:** The Drupal module gets a small YAML reader that keeps the byte range of every scalar and its key path, shared by the services registry and a new route registry read from `*.routing.yml`. Route names in PHP reach the route registry through string targets with a built-in table of calls. Core gains one framework-agnostic entry point for definitions in files the PHP engine does not read; the server answers only definitions there, and the extension sends those YAML files to the server.

**Tech Stack:** existing workspace (`yaml-rust2`).

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, items 5 and 6; registries exclude test modules; anti-goal: a wrong target is worse than no target).

**Scope:** Fifth part of delivery step 4. Out of scope: routes defined in PHP (`route_callbacks`, route subscribers), route parameters and options, menu link and local task ids (`parent`, `parent_id`), `new Url('name')`, route names built at run time, and YAML files other than `*.services.yml`, `*.routing.yml` and `*.links.{menu,task,action,contextual}.yml`.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, no client names or paths in committed files, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Registration point:** core changes stay framework-agnostic. Drupal knowledge lives in `frameworks/drupal`.
- **Routes:** every top-level key of every `*.routing.yml` outside test modules, hidden directories and `vendor`, except `route_callbacks`. A route name defined twice gives both keys.
- **Route names in PHP:** argument 0 of `Drupal\Core\Url::fromRoute`, `Drupal\Core\Form\FormStateInterface::setRedirect`, `Drupal\Core\Controller\ControllerBase::redirect`, `Drupal\Core\Routing\UrlGeneratorInterface::generateFromRoute` and `Drupal\Core\Access\AccessManagerInterface::checkNamedRoute`; argument 1 of `Drupal\Core\Link::createFromRoute`. Calls on classes that are or extend these. Each target is the route key, with the route name as the expected text.
- **References in YAML**, by key path (a `*` is any key, `[]` any sequence element):

  | File             | Where                                                                                    | Value                                       | Target                                                |
  | ---------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------- | ----------------------------------------------------- |
  | `*.services.yml` | `services.*.class`, a service key that names a class                                     | `Drupal\x\Foo`                              | the class                                             |
  | `*.services.yml` | `services.*.factory`, `services.*.factory[]`, `services.*.calls[][]`                     | `Foo::create`, `['@id', 'm']`, `[Foo, m]`   | the method, on the named class or the service's class |
  | `*.services.yml` | any value starting with `@` (`@id`, `@?id`), `services.*.parent`, `.decorates`, `.alias` | a service id                                | every definition of the service                       |
  | `*.routing.yml`  | `*.defaults._controller`, `._title_callback`, `*.requirements._custom_access`            | `\Drupal\x\Foo::bar` or `service.id:method` | the method                                            |
  | `*.routing.yml`  | `*.defaults._form`                                                                       | `\Drupal\x\Form\FooForm`                    | the class                                             |
  | `*.links.*.yml`  | `*.route_name`, `*.base_route`, `*.appears_on[]`                                         | a route name                                | every definition of the route                         |
  | `*.links.*.yml`  | `*.class`                                                                                | `Drupal\x\Foo`                              | the class                                             |

  Class names lose a leading `\`; a method is found on the class and its parents. A value naming nothing indexed gives nothing.

- **YAML documents in the server:** definitions only. Hover, references, symbols, implementations, call and type hierarchies and diagnostics give nothing for them.
- **Targets on the reference project:** member coverage in `web/modules/custom` stays at or above 42,455 accesses; cold index under 10 seconds; memory under 1 GB; `definition_ms` under 1000.

## Review focus

- **A controller served by a service** (`_controller: 'mymodule.controller:page'`) opens the service class's method. Test in Task 3.
- **Quoted and escaped class names** (`'\Drupal\x\Foo::bar'`, `"Drupal\\x\\Foo"`) resolve, and the cursor anywhere inside the scalar works. Test in Task 3.
- **A YAML document gets no PHP diagnostics, hover or symbols.** Test in Task 3.
- **A route in a test module** is not a route. Test in Task 1.
- **The services registry keeps its key positions** after moving to the shared YAML reader, including keys after multi-byte characters. Test in Task 1.

---

### Task 1: YAML positions and the route registry

**Files:**

- Create: `crates/phplumb-index/src/frameworks/drupal/yaml.rs`, `crates/phplumb-index/src/frameworks/drupal/routes.rs`
- Modify: `crates/phplumb-index/src/frameworks/drupal/services.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
// frameworks::drupal::yaml (crate-internal)
pub(crate) enum Yaml {
    /// A scalar and the byte range it is written at, quotes included.
    Scalar { value: String, start: u32, end: u32 },
    Map(Vec<(Yaml, Yaml)>),
    Sequence(Vec<Yaml>),
    Other,
}
pub(crate) fn parse(text: &str) -> Option<Yaml>;
/// The scalar covering `offset` and the keys and sequence positions leading to it.
pub(crate) fn scalar_at(root: &Yaml, offset: u32) -> Option<(Vec<Step<'_>>, &Yaml)>;
pub(crate) enum Step<'a> { Key(&'a str), Index(usize), /** the scalar is this map's key */ KeyItself }

// phplumb_index::frameworks::drupal
pub struct RouteDefinition { pub name: String, pub path: PathBuf, pub start: u32, pub end: u32 }
impl Drupal {
    /// Every definition of the route `name`, sorted by path.
    pub fn routes(&self, name: &str) -> &[RouteDefinition];
}
```

The services registry reads its files with `yaml::parse`; the route registry reads every routing file the same way and updates like services.

- [ ] **Step 1: Write the failing tests** on the fixture, extended with `mymodule.routing.yml` (a controller route, a form route, a service-controller route, `route_callbacks`), a route also defined in `gadget.routing.yml`, and a route in the test module:
  - `routes("mymodule.page")` gives its key in `mymodule.routing.yml`; the route defined twice gives both keys; the test module's route and `route_callbacks` give nothing.
  - Existing services tests still pass, and a services file with `é` in a comment before a key keeps the key's range.
  - Editing a routing file and updating the index changes `routes()`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the YAML reader, the route registry and the services move.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): read drupal routes`.

### Task 2: Route names in PHP

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/routes.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `Drupal::routes` (Task 1), `Framework::string_targets`.

- [ ] **Step 1: Write the failing tests** (needle offsets), with stubs for `Url`, `Link`, `FormStateInterface`, `ControllerBase` and a controller extending it:
  - `Url::fromRoute('mymodule.page')`, `Link::createFromRoute('Text', 'mymodule.page')`, `$form_state->setRedirect('mymodule.page')` and `$this->redirect('mymodule.page')` in a controller: the route key.
  - The text argument of `createFromRoute` and an unknown route name: nothing.

- [ ] **Step 2: Run them and watch them fail.** Expected: FAIL.

- [ ] **Step 3: Implement the call table.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate drupal route names`.

### Task 3: Navigation inside YAML files

**Files:**

- Modify: `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/routes.rs`, `crates/phplumb-server/src/lsp.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/**`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`, `crates/phplumb-server/tests/lsp_drupal.rs`

**Interfaces:**

- Produces:

```rust
pub trait Framework {
    // existing methods
    /// Targets at `offset` in a file the PHP engine does not read, each with the text
    /// expected at its range.
    fn file_targets(&self, index: &Index, path: &Path, source: &[u8], offset: u32) -> Vec<(Location, String)>;
}
```

`Index::definition` on a path whose extension the PHP filter does not accept asks the frameworks instead of parsing PHP; every other query gives nothing for such a path. The extension's document selector adds `{ scheme: "file", language: "yaml", pattern: "**/*.{services,routing,links.menu,links.task,links.action,links.contextual}.yml" }`.

- [ ] **Step 1: Write the failing tests**, with `mymodule.links.menu.yml`, `mymodule.links.task.yml` and services entries using a factory, `calls` and `@` arguments:
  - Every row of the global constraints' YAML table, from the fixture, including a service-controller route and `"Drupal\\mymodule\\Greeter"` double-quoted.
  - A value naming no class, method, service or route: nothing.
  - An LSP test (`lsp_drupal.rs`): definition on a `_form` value in a routing file gives the form class; diagnostics and hover on that file give nothing.
  - A VS Code test: Cmd+click on a `_controller` value in a `basic` fixture routing file opens the controller method.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks && cargo test -p phplumb-server --test lsp_drupal`. Expected: FAIL.

- [ ] **Step 3: Implement the entry point, the Drupal answers, the server guard and the selector.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate drupal yaml files`.

### Task 4: Reference cases, benchmarks and README

**Files:**

- Modify: `crates/phplumb-server/src/bench.rs` (only if a case cannot be expressed), `docs/benchmarks.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Add at least 6 cases**, each checked by hand: `Url::fromRoute()` in custom code, a `_controller` and a `_form` in a custom routing file, a `route_name` in a custom links file, a service `class` and an `@` argument in a custom services file.

- [ ] **Step 2: Run the reference benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0; resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024; `definition_ms` under 1000. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 3: Add a dated section to `docs/benchmarks.md`**, move Routes and YAML to the available Drupal features in `README.md`, and commit with message `docs: record drupal route and yaml results`.
