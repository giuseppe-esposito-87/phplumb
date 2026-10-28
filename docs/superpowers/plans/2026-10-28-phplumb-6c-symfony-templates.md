# PHPlumb 6c: Symfony templates implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a template name in PHP (`render()`, `renderView()`, `#[Template]`) or Twig (`extends`, `include`, `embed`, `import`, `from`, `use`, `include()`, `source()`) opens the Twig file, with names resolved as Symfony's TwigBundle configures Twig's loader.

**Architecture:** The Symfony module reads the template paths from `config/packages/twig.yaml` (and its `when@<env>` blocks): each namespace's directories and the main namespace's, then `default_path`. A template name resolves to the first existing file in that order. PHP string arguments reach the module through `string_targets`, `#[Template]` through `declaration_targets`, and Twig names through the Twig scanner of step 6b.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goal 2 "Templates", delivery step 6c. The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Template paths** (spec): "Template namespaces such as `@Identity` come from `twig.paths` in `config/packages/twig.yaml`; names without a namespace resolve against the default template directory and the paths without a namespace." As `vendor/symfony/twig-bundle/DependencyInjection/TwigExtension.php` adds them: `twig.paths` entries in order (a path with an empty or null namespace, or a list entry, joins the main namespace), then `default_path` (default `%kernel.project_dir%/templates`) when it exists. `%kernel.project_dir%` is the Symfony project's directory; a path with any other parameter is skipped. `twig.paths` and `default_path` inside `when@<env>` blocks count too.
- **Resolution:** `@Ns/name` looks in `Ns`'s directories, other names in the main namespace's, in order; the first existing file is the target, at offset 0 with an empty text. A namespace the configuration does not declare, such as a bundle's, gives nothing.
- **Usage sites:**
  - PHP: argument 0 of `render()`, `renderView()`, `renderBlock()`, `renderBlockView()` and `stream()` declared in `Symfony\Bundle\FrameworkBundle\Controller\AbstractController`, and of `render()`, `display()` and `load()` declared in `Twig\Environment`; argument 0 of the `#[\Symfony\Bridge\Twig\Attribute\Template]` attribute on a method.
  - Twig: the string literal after `extends`, `include`, `embed`, `import`, `from` or `use` at the start of a `{% %}` tag, also as an element of an array right after `extends` or `include`; the first argument of `include()` and `source()`.
- **Branch** `feat/symfony-templates` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **The same name in two directories of one namespace** opens the first, as Twig's loader does. Test in Task 1.
- **A name in an undeclared namespace,** such as `@Twig/Exception/error.html.twig`, gives nothing. Test in Task 1.
- **`twig.yaml` with no `paths`** still resolves plain names against `templates/`. Test in Task 1.
- **A Twig tag whose name is a variable,** such as `{% include template %}`, gives nothing. Test in Task 3.
- **A route name in `path()` and a template name in `include()` on one line** each open their own target. Test in Task 3.

---

### Task 1: Read template paths

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/templates.rs`
- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`, `crates/phplumb-index/tests/projects.rs`

**Interfaces:**

- Produces: `Symfony::template_file(&self, name: &str) -> Option<PathBuf>`; the module reads the template paths in `rebuild`, and again in `update` when a changed or removed path is a YAML file under `config/packages` or a directory holding one.

- [ ] **Step 1: Extend the fixture:** `config/packages/twig.yaml` with `paths: { '%kernel.project_dir%/src/Blog/templates': Blog, '%kernel.project_dir%/src/Shared/templates': Blog, '%kernel.project_dir%/extra': ~ }` and `when@dev: { twig: { paths: { '%kernel.project_dir%/dev/templates': Dev } } }`; templates `templates/base.html.twig`, `extra/only_extra.html.twig`, `extra/base.html.twig`, `src/Blog/templates/list.html.twig`, `src/Shared/templates/list.html.twig`, `src/Shared/templates/card.html.twig`, `dev/templates/tools.html.twig`.

- [ ] **Step 2: Write the failing tests** in `tests/symfony.rs`, through `template_file`, each as a path relative to the fixture:
  - `namespaced_names_use_their_paths`: `@Blog/list.html.twig` gives `src/Blog/templates/list.html.twig`; `@Blog/card.html.twig` gives `src/Shared/templates/card.html.twig`; `@Dev/tools.html.twig` gives `dev/templates/tools.html.twig`.
  - `plain_names_use_the_main_namespace_then_the_default_path`: `base.html.twig` gives `extra/base.html.twig`; `only_extra.html.twig` gives `extra/only_extra.html.twig`.
  - `unknown_names_and_namespaces_give_nothing`: `@Twig/Exception/error.html.twig` and `missing.html.twig` give nothing.
  - In `tests/projects.rs`, `templates_without_paths_use_the_default_directory`: a temporary Symfony project with `config/packages/twig.yaml` holding only `twig: {}` and `templates/page.html.twig` resolves `page.html.twig`.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony --test projects`. Expected: FAIL to compile, `template_file` does not exist.

- [ ] **Step 4: Implement** `templates.rs` and wire it into the module.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(symfony): read twig template paths`.

### Task 2: Template names in PHP

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Consumes: `Symfony::template_file` (Task 1).

- [ ] **Step 1: Write the failing tests.** Add the `Twig\Environment` and `Symfony\Bridge\Twig\Attribute\Template` stubs to the fixture's vendor and `installed.json`, `render()` and `renderView()` to its `AbstractController`, and `src/Controller/PagesController.php` extending `AbstractController`, with a promoted `Environment $twig`, holding on their own lines `$this->render('@Blog/list.html.twig')`, `$this->renderView('base.html.twig')`, `$this->twig->render('@Blog/card.html.twig')`, `$this->render('missing.html.twig')`, and a method with `#[Template('@Dev/tools.html.twig')]`:
  - `template_names_in_php_open_their_file`: definition on each of the first three literals and on the attribute's literal gives the Task 1 file at offset 0.
  - `unknown_template_names_give_nothing`: `missing.html.twig` gives nothing.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: both FAIL on the expected targets.

- [ ] **Step 3: Implement** the PHP usage sites in `string_targets` and the `#[Template]` attribute in `declaration_targets`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from template names in php`.

### Task 3: Template names in Twig

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/twig.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Produces: `twig::template_name_at(source: &[u8], offset: u32) -> Option<(String, u32, u32)>`, for the Twig usage sites in the global constraints.

- [ ] **Step 1: Write the failing tests.** Add `templates/page.html.twig` with, on their own lines, `{% extends 'base.html.twig' %}`, `{% include '@Blog/list.html.twig' with {a: 1} %}`, `{% embed "@Blog/card.html.twig" %}{% endembed %}`, `{% import 'only_extra.html.twig' as ui %}`, `{% from '@Dev/tools.html.twig' import x %}`, `{% use 'base.html.twig' %}`, `{{ include('@Blog/list.html.twig') }}`, `{{ source('base.html.twig') }}`, `{% extends ['missing.html.twig', 'base.html.twig'] %}`, `{% include template %}` and `<a href="{{ path('home') }}">{{ include('@Blog/card.html.twig') }}</a>`:
  - `template_names_in_twig_open_their_file`: definition on each literal of the first eight lines gives the Task 1 file; on `'base.html.twig'` in the array, `extra/base.html.twig`.
  - `twig_template_variables_give_nothing`: definition on `template` gives nothing.
  - `routes_and_templates_on_one_line`: on that last line, `'home'` gives `HomeController::index` and `'@Blog/card.html.twig'` gives its file.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: the tests that expect a file FAIL.

- [ ] **Step 3: Implement** `template_name_at` and answer it in `Symfony::file_targets` beside route names.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from template names in twig`.

### Task 4: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Build the 6b baseline** from `git merge-base main HEAD`.

- [ ] **Step 2: Probe every literal template name** in the Symfony reference project's PHP (`render(`, `renderView(`) and Twig files (the tags and functions above), count those that resolve, and check every one that does not by hand. Add cases: a namespaced `render()` name, a plain `extends` name, an `include()` name, and a `{% import %}` name.

- [ ] **Step 3: Run both benchmarks and both diagnostics sweeps,** for this branch and the baseline. Expected: the Drupal reference project unchanged; Symfony coverage, cases and diagnostics at least the baseline's; cold index under 10000 and `peak_rss_mb` under 1024. If a target is missed, stop and report to the user.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6c Symfony templates` to `docs/benchmarks.md`, a README bullet under Symfony, and to `docs/backlog.md` anything this step defers, such as bundle templates and their overrides in `templates/bundles`, and `form_themes` in `twig.yaml`. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record symfony template results`.
