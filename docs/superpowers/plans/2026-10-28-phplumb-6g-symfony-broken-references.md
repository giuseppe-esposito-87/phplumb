# PHPlumb 6g: broken Symfony references implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flag route names, template names and parameter names that are defined nowhere, in PHP, Twig and YAML, staying silent whenever a source that could define the name cannot be read.

**Architecture:** The Symfony module gains a certainty model for each kind of name, built when it reads routes, templates and parameters. `Symfony::diagnostics` checks the literal names at every usage site the navigation steps answer. The index lets frameworks diagnose YAML and Twig files too, which it skipped until now.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goal 7 "Broken Symfony references", anti-goal "No false Symfony alarms", success criterion "Diagnostics sweep", delivery step 6g. The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **No false alarms** (spec): "A broken route, template or parameter is flagged only when every source that could define it was read. A route imported from a source the module cannot read, a template in a namespace the module does not know, or a parameter set by PHP code with a computed name keeps the diagnostic silent."
- **Routes are certain** when every import in the route files is one the module reads (`routing.controllers`, attribute directories or files of the project, YAML files of the project) or one of the Symfony imports whose names all start with `_`: resources under `@FrameworkBundle`, `@WebProfilerBundle`, `@SecurityBundle` and `@TwigBundle`, and the `security.route_loader.logout` service. With such imports, names starting with `_` stay silent. Any other import makes every route name uncertain. A route named through a class constant (step 6b backlog) also makes route names uncertain.
- **Templates are certain** per namespace: a namespace the configuration does not declare, or one with a path the module cannot read, stays silent. TwigBridge's form theme directory (`vendor/symfony/twig-bridge/Resources/views/Form`) belongs to the main namespace, as FrameworkBundle adds it.
- **Parameters are certain** when no PHP configuration file (`config/*.php`, `config/packages/*.php`) exists and every `setParameter()` call in project code has a literal name, which then counts as a definition. Even then, a missing name is flagged only when its first segment (before the first `.`) is the first segment of a parameter the project defines, since bundles define their own parameters; `kernel.*` and `env(...)` are never flagged.
- **Sites:** the usage sites of steps 6b, 6c and 6e with a literal name: `redirectToRoute()`, `generateUrl()`, `generate()`, Twig `path()` and `url()`; `render()` and the other template methods, `#[Template]`, Twig template tags and functions; `#[Autowire(param:)]`, `#[Autowire('%x%')]`, `getParameter()`, and `%x%` in YAML under `config/`.
- **Messages:** "Undefined route 'x'", "Undefined template 'x'", "Undefined parameter 'x'", with the severity of the other framework diagnostics, on the name's range.
- **Branch** `feat/symfony-broken-references` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **A route only an unknown bundle import could define** stays silent. Test in Task 1.
- **`_wdt` or `_logout_main`** with only the known Symfony imports stays silent, while `missing_route` is flagged. Test in Task 1.
- **`form_div_layout.html.twig`** in `{% use %}` is not flagged. Test in Task 2.
- **A parameter `router.request_context.host`,** whose prefix no project parameter shares, stays silent. Test in Task 1.
- **A Drupal project** gets no Symfony diagnostics, and Drupal diagnostics never reach Symfony Twig or YAML files. Test in Task 3.

---

### Task 1: Certainty and diagnostics in PHP

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/routes.rs`, `templates.rs`, `config.rs`, `mod.rs`, `crates/phplumb-index/tests/symfony.rs`, fixtures

**Interfaces:**

- Produces: `Symfony::route_certain(&self, name: &str) -> bool`, `Symfony::template_status(&self, name: &str) -> TemplateStatus` with `pub enum TemplateStatus { Found(PathBuf), Missing, Unknown }`, and `Symfony::parameter_certain(&self, name: &str) -> bool`, each true or `Missing` only when the name is defined nowhere and every source was read.

- [ ] **Step 1: Write the failing tests.** In the fixture, add `config/routes/framework.yaml` with `when@dev: { _wdt: { resource: '@WebProfilerBundle/Resources/config/routing/wdt.php' } }` and `config/routes/security.yaml` with `_security_logout: { resource: security.route_loader.logout, type: service }`, and `src/Controller/BrokenController.php` extending `AbstractController`, holding on their own lines `redirectToRoute('home')`, `redirectToRoute('missing_route')`, `redirectToRoute('_wdt')`, `render('base.html.twig')`, `render('nowhere.html.twig')`, `render('@Unknown/x.html.twig')`, `getParameter('app.name')`, `getParameter('app.nowhere')`, `getParameter('router.request_context.host')`, and a promoted parameter with `#[Autowire(param: 'app.missing')]`:
  - `broken_symfony_references_are_flagged`: diagnostics on the file are exactly "Undefined route 'missing_route'", "Undefined template 'nowhere.html.twig'", "Undefined parameter 'app.nowhere'" and "Undefined parameter 'app.missing'", each on its name.
  - In `tests/projects.rs`, `unknown_route_imports_silence_route_diagnostics`: a temporary Symfony project with `config/routes.yaml` importing `@AcmeBundle/Resources/config/routes.yaml` gives no diagnostic on `redirectToRoute('missing_route')`.
  - In `tests/projects.rs`, `php_configuration_silences_parameter_diagnostics`: with a `config/services.php`, `getParameter('app.nowhere')` gives no diagnostic.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony --test projects`. Expected: `broken_symfony_references_are_flagged` FAILS.

- [ ] **Step 3: Implement** the certainty model while reading routes, templates and parameters, and `Symfony::diagnostics` for PHP files.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): flag broken route, template and parameter names in php`.

### Task 2: Diagnostics in Twig and YAML

**Files:**

- Modify: `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/src/frameworks/symfony/twig.rs`, `crates/phplumb-index/tests/symfony.rs`, fixtures

**Interfaces:**

- Produces: `Index::diagnostics` asks `frameworks_for(path)` for framework files (YAML, Twig, Doctrine XML) and gives no PHP diagnostics there; `twig::names(source) -> Vec<(TwigName, String, u32, u32)>`, every literal route and template name of a template with its range.

- [ ] **Step 1: Write the failing tests.** Add `templates/broken.html.twig` with `{{ path('home') }}`, `{{ path('missing_route') }}`, `{% extends 'base.html.twig' %}`, `{% include 'nowhere.html.twig' %}` and `{% use 'form_div_layout.html.twig' %}` (with a `vendor/symfony/twig-bridge/Resources/views/Form/form_div_layout.html.twig` stub), and to `config/packages/framework.yaml` a value `'%app.nowhere%'`:
  - `broken_names_in_twig_are_flagged`: exactly "Undefined route 'missing_route'" and "Undefined template 'nowhere.html.twig'".
  - `broken_parameters_in_yaml_are_flagged`: exactly "Undefined parameter 'app.nowhere'" in `framework.yaml`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony broken`. Expected: both new tests FAIL.

- [ ] **Step 3: Implement** the index change, `twig::names` and the Symfony diagnostics for Twig and YAML.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): flag broken names in twig and yaml`.

### Task 3: Separation and the sweep

**Files:**

- Modify: `crates/phplumb-index/tests/projects.rs`, `crates/phplumb-server/src/bench.rs`, fixtures

**Interfaces:**

- Produces: `bench --diagnostics <dir>` also checks the YAML and Twig files below `dir`.

- [ ] **Step 1: Write the failing tests:**
  - In `tests/projects.rs`, `diagnostics_stay_in_their_module`: in the `projects` fixture, a Symfony Twig file with `{{ path('demo.page') }}` gets "Undefined route" only if Symfony's routes are certain, never a Drupal one, and the Drupal `demo.services.yml` gets no Symfony diagnostic.
  - In `crates/phplumb-server/tests/bench.rs`, `bench_sweeps_twig_and_yaml`: on a temporary Symfony project with one broken Twig route, the diagnostics count is 1.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test --workspace`. Expected: `bench_sweeps_twig_and_yaml` FAILS.

- [ ] **Step 3: Implement** the bench sweep of framework files.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(bench): sweep twig and yaml diagnostics`.

### Task 4: Reference sweep, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`

- [ ] **Step 1: Build the 6f baseline** from `git merge-base main HEAD`.

- [ ] **Step 2: Run the diagnostics sweep** on the Symfony reference project's `src`, `templates` and `config`, and on the Drupal reference project's `web/modules/custom`, for this branch and the baseline. Check every new diagnostic by hand. Expected: none on Drupal; on Symfony, only real broken references. If any is a false alarm, fix the certainty rule with a test before going on.

- [ ] **Step 3: Run both benchmarks,** base first. Expected: coverage and cases unchanged; cold index under 10000 and `peak_rss_mb` under 1024.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6g broken Symfony references` to `docs/benchmarks.md` with the sweep results, a README bullet under Symfony, and to `docs/backlog.md` anything this step defers, such as reading bundle route files and parameters set by bundle extensions. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record broken symfony reference results`.
