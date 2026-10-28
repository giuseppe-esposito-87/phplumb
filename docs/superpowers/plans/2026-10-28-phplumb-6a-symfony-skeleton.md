# PHPlumb 6a: Symfony module skeleton implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Symfony module that activates on `symfony/framework-bundle`, with its setting, and project scoping for both modules: each reads and answers only for its own project, and code both need moves into shared modules with no change in Drupal behavior.

**Architecture:** Framework detection moves to one shared function that finds the nearest `composer.lock` listing a module's package. Each active module gets a `Project`: that lock's directory, minus the project directories of other active modules inside it. The Drupal module reads files and declarations only inside its project, and the index asks a module only about files inside its project. The Symfony module implements `Framework` and does nothing else yet.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, delivery step 6a, constraints "Each module reads its own project", "Symfony activation" and "The modules share code, not each other", anti-goal "No cross-talk between modules".

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Each module reads its own project** (spec): "A module's project is the directory of the matching `composer.lock` nearest to the root, minus the directory of any other active module's project inside it. Matching locks deeper than the nearest one are ignored; when several share the nearest depth, the first in path order wins. A module forced on without a matching lock reads from the root."
- **Detection depth** stays as today: locks up to depth 4 below the root, outside hidden directories, `vendor` and the skipped directory names.
- **Symfony activation** (spec): the setting `phplumb.frameworks.symfony` takes `auto`, `on` or `off`; in `auto` the module turns on when a lock lists `symfony/framework-bundle` in `packages` or `packages-dev`. Drupal keeps `drupal/core`.
- **No cross-talk:** a module reads files and declarations only inside its project, and is asked about a file only when the file is inside its project.
- **No change in Drupal behavior** on a project with no other module: every existing test passes unchanged apart from struct literals that gain a field, and the Drupal reference project gives the same benchmark.
- **Synthetic fixtures only.**
- **Branch** `feat/symfony-skeleton` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **One lock listing both packages** makes both modules active on the same directory, and neither excludes the other. Test in Task 1.
- **Sibling directories with a shared prefix,** such as `drupal` and `drupal2`, are different projects: `Project::contains` compares whole path components. Test in Task 1.
- **A forced module with a matching lock deeper down** uses that lock's directory, not the root. Test in Task 1.
- **A file changed outside the Drupal project after the build,** such as a new Symfony controller with `#[Route]`, never becomes a Drupal route. Test in Task 2.
- **The Drupal reference project,** where Drupal is the only module, keeps every case, coverage figure and diagnostic. Checked in Task 6.

---

### Task 1: Project detection and the Symfony module

**Files:**

- Create: `crates/phplumb-index/src/frameworks/project.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/projects.rs`
- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:
  - `pub struct Project { pub dir: PathBuf, pub excluded: Vec<PathBuf> }`, canonical paths, with `pub fn contains(&self, path: &Path) -> bool`: inside `dir` and inside no `excluded` directory, by whole path components.
  - `pub(crate) fn nearest_lock(root: &Path, package: &str) -> Option<PathBuf>` in `project.rs`: the directory of the matching lock nearest to the root under the detection rules; the depth constant and the lock reading move here from the Drupal module.
  - `Framework::package(&self) -> &'static str` replaces `Framework::detect`; `Framework::project(&self) -> &Project` and `Framework::set_project(&mut self, project: Project)`.
  - `FrameworkSettings::symfony: Activation`; `registered` lists Drupal, then Symfony.
  - `frameworks::active` sets each active module's project: its nearest lock's directory (the root when forced on without one), then every other active project strictly inside it as `excluded`.
  - `pub struct Symfony` in `frameworks/symfony/mod.rs`, named `symfony`, with package `symfony/framework-bundle`, no dependency directories, no extra PHP extensions, and empty answers everywhere.
  - `Index::framework_project(&self, name: &str) -> Option<&Project>`.

- [ ] **Step 1: Write the failing tests** in `tests/projects.rs`, building temporary projects as `tests/frameworks.rs` does, with a Drupal lock (`drupal/core`) and a Symfony lock (`symfony/framework-bundle`):
  - `symfony_is_detected_from_framework_bundle`: a root Symfony lock gives `active_frameworks() == ["symfony"]`.
  - `drupal_projects_never_activate_symfony`: a root lock listing `drupal/core` and `symfony/http-kernel` gives `["drupal"]`.
  - `the_nearest_lock_wins`: Drupal locks in `a/` and `b/c/` give the Drupal project `root/a`.
  - `locks_at_the_same_depth_take_the_first_in_path_order`: Drupal locks in `b/` and `a/` give `root/a`.
  - `nested_projects_are_left_out_of_the_outer_one`: a root Symfony lock and a Drupal lock in `codebase/drupal/` give Symfony `{ dir: root, excluded: [root/codebase/drupal] }` and Drupal `{ dir: root/codebase/drupal, excluded: [] }`.
  - `one_lock_with_both_packages_activates_both_on_the_root`: both projects are the root, neither excluded.
  - `a_forced_module_without_a_lock_reads_the_root` and `a_forced_module_uses_a_deeper_lock`: Drupal `on` with no lock gives `root`; with a lock in `site/`, `root/site`.
  - `project_contains_compares_whole_components`: `Project { dir: /p/drupal }` contains `/p/drupal/x.php` and not `/p/drupal2/x.php`.
  - In `tests/frameworks.rs`, struct literals of `FrameworkSettings` add `..FrameworkSettings::default()`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test projects`. Expected: FAIL to compile, `Project`, `Symfony` and `symfony` do not exist.

- [ ] **Step 3: Implement** `project.rs`, the trait changes, the Symfony module and `active`, and move the Drupal module's detection onto `nearest_lock`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(frameworks): detect each module's project and add the symfony module`.

### Task 2: Drupal reads only its project

**Files:**

- Modify: `crates/phplumb-index/src/filter.rs`, `crates/phplumb-index/src/frameworks/drupal/*.rs`, `crates/phplumb-index/tests/projects.rs`
- Create: `crates/phplumb-index/tests/fixtures/projects/**`

**Interfaces:**

- Consumes: `Project`, `Framework::project` (Task 1).
- Produces: `PathFilter::scoped(&self, project: &Project) -> PathFilter`: the same filter with `project.dir` as its root, refusing the `excluded` directories in `accepts_dir` and `accepts_path`.

- [ ] **Step 1: Create the fixture** `crates/phplumb-index/tests/fixtures/projects`, a Symfony project holding a Drupal codebase:
  - `composer.lock` listing `symfony/framework-bundle`;
  - `src/Controller/HomeController.php`: a class whose method has `#[\Symfony\Component\Routing\Attribute\Route('/home', name: 'symfony_home')]`;
  - `codebase/drupal/composer.lock` listing `drupal/core`, and under `codebase/drupal/web`: the smallest core and custom module the Drupal fixtures use for a services file (`demo.services.yml` defining `demo.thing` as `Drupal\demo\Thing`), a routing file (`demo.routing.yml` defining `demo.page`), a controller method with the same `Route` attribute named `demo.attr`, `\Drupal` with `service()`, and `Drupal\demo\Thing` with `run()`.

- [ ] **Step 2: Write the failing tests**, reaching the Drupal module as `tests/frameworks.rs` does:
  - `drupal_reads_only_its_project`: `defines_route` is true for `demo.page` and `demo.attr`, false for `symfony_home`.
  - `updates_outside_the_drupal_project_are_ignored`: after copying the fixture to a temporary directory and building, writing `src/Controller/LateController.php` with a `Route` attribute named `symfony_late` and calling `update` with it leaves `defines_route("symfony_late")` false.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test projects`. Expected: both FAIL on the Symfony route names.

- [ ] **Step 4: Scope the Drupal module.** Every reader walks `index.filter().scoped(self.project())` in place of `index.filter()`, every loop over `index.declarations()` skips paths outside the project, and `update` drops changed and removed paths outside it.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass, the Drupal fixtures unchanged.

- [ ] **Step 6: Commit** with message `feat(drupal): read only the drupal project`.

### Task 3: Modules answer only inside their project

**Files:**

- Modify: `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/tests/projects.rs`, `crates/phplumb-index/tests/fixtures/projects/**`

**Interfaces:**

- Consumes: `Framework::project` (Task 1).
- Produces: `Index::frameworks_for(&self, path: &Path) -> impl Iterator<Item = &dyn Framework>`: the active modules whose project contains `path`. Every framework query about a file goes through it: type inference of the file, string and keyed string targets, declaration and file targets, diagnostics, annotation classes and class implementations (the path of the class's declaration). Ranking by dependency directories stays global.

- [ ] **Step 1: Write the failing tests**, adding `src/Uses.php` and `codebase/drupal/web/modules/custom/demo/src/Uses.php`, each with `\Drupal::service('demo.thing')->run();` and `\Drupal::service('missing.service');` on their own lines:
  - `drupal_types_nothing_outside_its_project`: definition on `run` gives `Thing::run` in the Drupal file and nothing in `src/Uses.php`.
  - `drupal_flags_nothing_outside_its_project`: diagnostics give one diagnostic on `missing.service` in the Drupal file and none in `src/Uses.php`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test projects`. Expected: both FAIL on `src/Uses.php`.

- [ ] **Step 3: Add `frameworks_for`** and route every per-file framework query through it; inference receives the file's path for `framework_type`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(frameworks): ask each module only about its own project`.

### Task 4: Shared framework modules

**Files:**

- Create: `crates/phplumb-index/src/frameworks/yaml.rs` (moved), `crates/phplumb-index/src/frameworks/attributes.rs`, `crates/phplumb-index/src/frameworks/ancestry.rs`
- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/*.rs`

**Interfaces:**

- Produces, `pub(crate)`, with the same behavior as today:
  - `frameworks::yaml`: the positioned YAML parser, moved whole from `drupal/yaml.rs`.
  - `frameworks::attributes::route_names(metadata: &[Metadata]) -> Vec<RouteName>`, with `RouteName` and the route attribute names, moved from `drupal/routes.rs`; Drupal's naming rules stay in Drupal.
  - `frameworks::ancestry::is_a(index: &Index, class: &str, ancestors: &[&str]) -> bool`, moved from `drupal/mod.rs`.

- [ ] **Step 1: Move the three pieces** and point the Drupal module at them. No logic changes.

- [ ] **Step 2: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass with the same test count as before the move.

- [ ] **Step 3: Commit** with message `refactor(frameworks): share yaml, route attributes and ancestry`.

### Task 5: The Symfony setting in the extension

**Files:**

- Modify: `editors/vscode/package.json`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/manifest.test.ts`, `crates/phplumb-server/src/lsp.rs` (settings doc comment), `README.md` (settings table)

- [ ] **Step 1: Write the failing test** `contributes the Symfony activation setting` in `manifest.test.ts`, like the Drupal one: enum `["auto", "on", "off"]`, default `"auto"`.

- [ ] **Step 2: Run it and watch it fail.** Run: `npm --prefix editors/vscode test`. Expected: the new test FAILS.

- [ ] **Step 3: Add the setting:**
  - `phplumb.frameworks.symfony` in `package.json`, described as "Symfony support. auto turns it on when a composer.lock in the project folder or up to three directories below it requires symfony/framework-bundle.";
  - the extension restarts the server when it changes and sends it as `frameworks.symfony`;
  - the `lsp.rs` settings example names `"symfony": "auto"`;
  - a README settings row in the Drupal row's style.

- [ ] **Step 4: Run the checks.** Run: `npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test && cargo test --workspace`. Expected: all pass. Format `README.md` with the Markdown formatter.

- [ ] **Step 5: Commit** with message `feat(vscode): add the symfony activation setting`.

### Task 6: Benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`

- [ ] **Step 1: Build the 5c baseline.** Build `git merge-base main HEAD` in a scratch copy with `git archive` and `cargo build --release`.

- [ ] **Step 2: Run both benchmarks** with `--cases`, `--coverage` and, for Drupal, `--cache <file>`, for this branch and the baseline, and both diagnostics sweeps. Expected:
  - Drupal reference: the same coverage, cases and diagnostics as the baseline; cold index under 10000 and `peak_rss_mb` under 1024.
  - Symfony reference: the server log names both modules, the Drupal project is `codebase/drupal`, every case passes, and diagnostics in `src` stay at 0.
  - Symfony coverage in `src`: any access the baseline resolves and this branch does not is a Drupal answer inside Symfony code; check each by hand and list them.

  If a target is missed, or a lost access is anything else, stop and report to the user.

- [ ] **Step 3: Record the results and the backlog.** Add a dated section `2026-10-28, step 6a Symfony module skeleton` to `docs/benchmarks.md`, with one table per reference project, and add to `docs/backlog.md` anything this step defers. Run the Markdown formatter on both files.

- [ ] **Step 4: Commit** with message `docs(benchmarks): record symfony skeleton results`.
