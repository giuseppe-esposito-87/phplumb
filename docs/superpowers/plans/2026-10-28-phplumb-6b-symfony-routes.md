# PHPlumb 6b: Symfony routes implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a route name in PHP (`redirectToRoute()`, `generateUrl()`, `UrlGeneratorInterface::generate()`) or Twig (`path()`, `url()`) opens the route's definition and its controller method, with route names as the installed Symfony version builds them.

**Architecture:** The Symfony module reads routes from `#[Route]` attributes on project classes and from YAML route files under `config/`, naming attribute routes as Symfony 8.1's `AttributeClassLoader` and FrameworkBundle's `AttributeRouteControllerLoader` do. PHP string arguments reach it through `string_targets`; Twig files reach it through `file_targets`, as YAML files do, and the extension sends them to the engine.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goal 1 "Routes", delivery step 6b. The 6a plan's constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds, including "each module reads its own project" and "no cross-talk".
- **Route names** (spec): "Route names follow the naming rules of the Symfony version installed in the project, including class-level prefixes and generated default names." For attribute routes, as the reference project's `vendor/symfony/routing/Loader/AttributeClassLoader.php` and `vendor/symfony/framework-bundle/Routing/AttributeRouteControllerLoader.php` do:
  - a method route's name is its `name` argument (named or at position 1), or the default name, prefixed by the `name` of the class's first `#[Route]`;
  - the default name lowercases `FQCN_method` with `\` as `_`, appends `_N` for the N-th unnamed route of the same method from N = 1, then replaces `bundle_` and `controller_` with `_`, drops a trailing `action` (keeping `_N`) when the method name ends in `Action` or `_action`, and replaces `__` with `_`;
  - a method with exactly one route also answers to `FQCN::method`;
  - a class with `__invoke` and no method routes turns each class-level `#[Route]` into a route of `__invoke`, without the class prefix; when the class then has exactly one route, it also answers to `FQCN` and `FQCN::__invoke`;
  - a name that is not a literal string makes no route.
- **Attribute routes** come from classes in project code, outside `vendor`, inside the Symfony project.
- **YAML routes** come from `config/routes.yaml` and every `*.yaml` or `*.yml` below `config/routes/`, inside the Symfony project: each top-level key with a `path`, and each such key inside a `when@<env>` block. Keys with `resource` are imports and define no route here.
- **Targets of a route name:** an attribute route gives its controller method's name; a YAML route gives its key and, when its `controller` (or `defaults._controller`) names `Class::method` or an invokable `Class`, that method. Unknown names give nothing.
- **Usage sites:** argument 0 of `redirectToRoute()` and `generateUrl()` declared in `Symfony\Bundle\FrameworkBundle\Controller\AbstractController`, and of `generate()` declared in `Symfony\Component\Routing\Generator\UrlGeneratorInterface`; in Twig, the first argument of `path()` and `url()` when it is a string literal.
- **Twig files** (`*.twig`) are framework files like YAML: PHP inference, hover and diagnostics skip them, and the extension sends them to the engine without a Twig language extension.
- **Branch** `feat/symfony-routes` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **Two unnamed routes on one method** get `…_list` and `…_list_1`, and the method has no `FQCN::method` alias. Test in Task 1.
- **An attribute route in `vendor`** gives no target, so a bundle's controller never shadows a project route. Test in Task 1.
- **A Drupal route name in Symfony code,** with both modules active, gives nothing. Test in Task 2.
- **A dynamic route name,** such as `redirectToRoute($name)` or `path(name)` in Twig, gives nothing. Test in Tasks 2 and 3.
- **A Twig file in a Drupal project** gets no YAML answers from the Drupal module. Test in Task 3.

---

### Task 1: Read Symfony routes

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/routes.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`
- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`

**Interfaces:**

- Consumes: `frameworks::attributes::route_names`, `frameworks::yaml`, `PathFilter::scoped` (step 6a).
- Produces: `Symfony::route_targets(&self, name: &str) -> &[RouteTarget]`, with `pub struct RouteTarget { pub location: Location, pub text: String }` (the text expected at the range, as `string_targets` returns it). `rebuild` reads every route; `update` reads them again when a changed or removed path is a PHP file of the project, a route YAML file, or a directory holding one.

- [ ] **Step 1: Create the fixture** `crates/phplumb-index/tests/fixtures/symfony`, a Symfony project:
  - `composer.lock` listing `symfony/framework-bundle`;
  - `vendor/composer/installed.json` with `symfony/framework-bundle` and `symfony/routing` autoloaded from `vendor/symfony/...`, holding `AbstractController` (`redirectToRoute`, `generateUrl`), `UrlGeneratorInterface` (`generate`), the `Route` attribute class, and a bundle controller with `#[Route('/vendor', name: 'vendor_page')]`;
  - `config/routes.yaml` with `controllers: { resource: routing.controllers }` and `legacy_page: { path: /legacy, controller: App\Controller\LegacyController::show }`; `config/routes/dev.yaml` with `when@dev: { dev_tools: { path: /dev, controller: App\Controller\LegacyController } }`;
  - `src/Controller/HomeController.php`: `index()` with `#[Route('/', name: 'home')]`;
  - `src/Controller/AdminController.php`: class `#[Route('/admin', name: 'admin_')]`, `users()` with `#[Route('/users', name: 'users')]`;
  - `src/Controller/BlogController.php`: `listAction()` with two unnamed `#[Route]`s, and `show()` with one unnamed `#[Route]`;
  - `src/Controller/AboutController.php`: class `#[Route('/about', name: 'about')]` with `__invoke()`;
  - `src/Controller/LegacyController.php`: `show()` and `__invoke()`, no attributes.

- [ ] **Step 2: Write the failing tests** in `tests/symfony.rs`, through `Symfony::route_targets`, each target as (path relative to the fixture, text):
  - `named_routes_lead_to_their_method`: `home` gives `[("src/Controller/HomeController.php", "index")]`.
  - `class_names_prefix_method_routes`: `admin_users` gives `AdminController::users`.
  - `default_names_follow_symfony`: `app_blog_list` and `app_blog_list_1` give `listAction`; `app_blog_show` gives `show`.
  - `single_route_methods_answer_to_their_fqcn`: `App\Controller\BlogController::show` gives `show`; `App\Controller\BlogController::listAction` gives nothing.
  - `invokable_classes_route_their_invoke`: `about`, `App\Controller\AboutController` and `App\Controller\AboutController::__invoke` give `__invoke`.
  - `yaml_routes_lead_to_their_key_and_controller`: `legacy_page` gives the key `legacy_page` in `config/routes.yaml` and `LegacyController::show`; `dev_tools` gives its key in `config/routes/dev.yaml` and `LegacyController::__invoke`; `controllers` gives nothing.
  - `vendor_attribute_routes_give_nothing`: `vendor_page` gives nothing.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: FAIL to compile, `route_targets` does not exist.

- [ ] **Step 4: Implement** `symfony/routes.rs` and wire it into the Symfony module's `rebuild` and `update`.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(symfony): read routes from attributes and yaml`.

### Task 2: Route names in PHP

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`, `crates/phplumb-index/tests/projects.rs`, `crates/phplumb-index/tests/fixtures/projects/**`

**Interfaces:**

- Consumes: `Symfony::route_targets` (Task 1).

- [ ] **Step 1: Write the failing tests.** Add `src/Service/Links.php` with a constructor-promoted `UrlGeneratorInterface $urls` and a controller extending `AbstractController`, holding on their own lines `$this->redirectToRoute('home')`, `$this->generateUrl('admin_users')`, `$this->urls->generate('app_blog_show')`, `$this->redirectToRoute('missing')` and `$this->redirectToRoute($name)`:
  - `route_names_in_php_lead_to_their_routes`: definition on each literal of the first three gives the Task 1 targets.
  - `unknown_and_dynamic_route_names_give_nothing`: `missing` and `$name` give nothing.
  - In `tests/projects.rs`, `symfony_sees_no_drupal_routes`: in the `projects` fixture, a Symfony controller's `redirectToRoute('demo.attr')` gives nothing and `redirectToRoute('symfony_home')` gives `HomeController::home`. The fixture gains the `AbstractController` stub and an `installed.json` entry for it.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony --test projects`. Expected: the tests that expect a target FAIL.

- [ ] **Step 3: Implement `Symfony::string_targets`** for the usage sites in the global constraints.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from route names in php`.

### Task 3: Route names in Twig

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/twig.rs`
- Modify: `crates/phplumb-index/src/filter.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/manifest.test.ts`

**Interfaces:**

- Produces:
  - `filter::is_twig(path: &Path) -> bool` and `filter::is_framework_file(path: &Path) -> bool` (YAML or Twig); every `is_yaml` guard in `index.rs` becomes `is_framework_file`.
  - `twig::route_name_at(source: &[u8], offset: u32) -> Option<(String, u32, u32)>`: the string literal under `offset` when it is the first argument of `path(` or `url(` inside `{{ }}` or `{% %}`, with its range inside the quotes.
  - `pub const DOCUMENT_SELECTOR` exported from `extension.ts`, holding PHP, YAML and `{ scheme: "file", pattern: "**/*.twig" }`.

- [ ] **Step 1: Write the failing tests.** Add `templates/base.html.twig` with, on their own lines, `{{ path('home') }}`, `<a href="{{ url('admin_users', {id: 1}) }}">`, `{% if path('app_blog_show') %}{% endif %}`, `{{ path(name) }}` and `{{ 'path("home")' }}`:
  - `route_names_in_twig_lead_to_their_routes`: definition on each of the first three literals gives the Task 1 targets.
  - `twig_without_a_literal_route_gives_nothing`: definition on `name` and inside the quoted text `path("home")` gives nothing.
  - `twig_files_get_no_php_answers`: hover and diagnostics on the Twig file give nothing.
  - In `tests/frameworks.rs`, `drupal_answers_nothing_in_twig`: definition on a `'mymodule.page'` literal in a Twig file of the Drupal fixture gives nothing.
  - In `manifest.test.ts`, `sends Twig files to the engine`: `DOCUMENT_SELECTOR` holds the `**/*.twig` pattern.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony --test frameworks && npm --prefix editors/vscode test`. Expected: the new tests that expect a target FAIL, and the manifest test FAILS.

- [ ] **Step 3: Implement** the Twig helpers, `Symfony::file_targets` for Twig, the `is_framework_file` guards, Drupal's `file_targets` answering YAML only, and the exported selector.

- [ ] **Step 4: Run the checks.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from route names in twig`.

### Task 4: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Build the 6a baseline** from `git merge-base main HEAD` with `git archive` and `cargo build --release`.

- [ ] **Step 2: Add Symfony cases**, each checked by hand in `<symfony-reference-project>`: a `redirectToRoute()` name with a class prefix, a `generate()` name, a `path()` name in a template under `src`, and an unknown route name expecting nothing. Check by hand that a sample of 20 `redirectToRoute()` and `path()` names across the project resolves, and list the ones that do not.

- [ ] **Step 3: Run both benchmarks and both diagnostics sweeps,** for this branch and the baseline. Expected: the Drupal reference project unchanged; Symfony coverage, cases and diagnostics at least the baseline's; cold index under 10000 and `peak_rss_mb` under 1024. If a target is missed, stop and report to the user.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6b Symfony routes` to `docs/benchmarks.md`; add a README section on Symfony routes in the Drupal section's style; add to `docs/backlog.md` anything this step defers, such as `name_prefix` on route imports and routes from bundles. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record symfony route results`.
