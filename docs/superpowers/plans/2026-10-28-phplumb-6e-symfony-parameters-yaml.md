# PHPlumb 6e: Symfony parameters and YAML navigation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a parameter name (`#[Autowire(param: 'x')]`, `#[Autowire('%x%')]`, `getParameter('x')`, `%x%` in YAML) opens its definition under `parameters:`, and in Symfony YAML configuration Cmd+click on a class name opens the class and on `@id` opens the service definition, as does `#[Autowire(service: 'id')]`.

**Architecture:** The Symfony module reads parameters and service definitions from `config/services*.yaml` and `config/packages/*.yaml`. A new index hook gives frameworks a string literal under the cursor inside any PHP attribute, constructor parameter attributes included, with the attribute's class and the argument's name or position. YAML files reach the module through `file_targets`, as for routes.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goals 4 "Parameters" and 5 "YAML navigation", delivery step 6e. The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Configuration files:** `config/services.yaml`, `config/services_<env>.yaml` and `config/packages/*.yaml` of the Symfony project, also inside `when@<env>` blocks.
- **Parameters** (spec): "Navigate from a parameter name to its definition under `parameters:` in the project's YAML configuration. Names appear in `#[Autowire(param: 'name')]`, `#[Autowire('%name%')]`, `getParameter('name')` and `%name%` inside YAML values. Parameters the kernel sets itself, such as `kernel.project_dir`, have no target." A key `env(NAME)` defines an environment default, not a parameter here, and `%env(...)%` is no parameter name. `getParameter` is the one declared in `Symfony\Bundle\FrameworkBundle\Controller\AbstractController`.
- **Services:** each key under `services:` is a service id, apart from `_defaults`, `_instanceof` and keys whose value has `resource` (prototypes). Its class is its `class:` value, else the id itself when the id names a class. A value `'@other'` or `alias: other` makes the id an alias of `other`.
- **YAML navigation** (spec): "In `config/services*.yaml` and `config/packages/*.yaml`, Cmd+click on a class name opens the class, and on `@id` opens the service definition, or the class when the id is a class name. This covers service keys, `class:`, `!service` blocks, aliases, and the message classes under `framework.messenger.routing`." `#[Autowire(service: 'id')]` leads to the same targets as `@id`.
- **Attribute strings:** `#[Autowire]` is `Symfony\Component\DependencyInjection\Attribute\Autowire`; its first positional argument is `value`. Inside a value, the `%name%` under the cursor is the parameter.
- **Branch** `feat/symfony-parameters-yaml` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **`%%` escapes and `%env(X)%`** in a YAML value or `#[Autowire]` give no parameter. Tests in Tasks 1 and 2.
- **A parameter defined twice,** in `services.yaml` and in a `when@dev` block, lists both definitions. Test in Task 1.
- **`@?id` and `@=expression`** in YAML: `@?id` (optional) leads to `id`; an expression gives nothing. Test in Task 3.
- **A Drupal `*.services.yml`** inside a nested Drupal project gets no Symfony answers, and a Symfony `services.yaml` no Drupal ones. Test in Task 3.
- **A class name in a YAML comment** gives nothing. Test in Task 3.

---

### Task 1: Parameters

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/config.rs`
- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Produces: `Symfony::parameter_targets(&self, name: &str) -> &[RouteTarget]` (the key of every definition, `RouteTarget` reused as a location with its text); `config::parameter_at(text: &str, offset: u32) -> Option<String>`, the `%name%` under `offset` in a string, `None` for `%%`, `env(...)` and `kernel.*`.

- [ ] **Step 1: Extend the fixture:** `config/services.yaml` with `parameters: { app.name: 'Demo', app.limit: 10, env(APP_X): 'x' }` and `services:` entries for Task 3; `config/packages/framework.yaml` with `when@dev: { parameters: { app.name: 'Dev demo' } }` and `framework: { secret: '%app.name%', cache: '%env(APP_X)%', escaped: '100%%' }`; `getParameter()` on the fixture's `AbstractController`; `src/Controller/ParamsController.php` calling `$this->getParameter('app.limit')` and `$this->getParameter('kernel.project_dir')`.

- [ ] **Step 2: Write the failing tests:**
  - `parameters_lead_to_their_definitions`: `app.name` gives its keys in `config/services.yaml` and `config/packages/framework.yaml`; `app.limit` its key in `config/services.yaml`; `env(APP_X)` and `kernel.project_dir` nothing.
  - `parameters_in_yaml_values`: definition on `app.name` inside `'%app.name%'` in `framework.yaml` gives both definitions; inside `%env(APP_X)%` and `100%%`, nothing.
  - `get_parameter_leads_to_the_definition`: definition on `'app.limit'` gives its key; on `'kernel.project_dir'`, nothing.
  - `parameter_at` unit tests in `config.rs` for `'%a%'`, `'x/%a%/y'` on and off the name, `'%%'`, `'%env(A)%'` and `'%kernel.secret%'`.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony && cargo test -p phplumb-index --lib config`. Expected: FAIL to compile.

- [ ] **Step 4: Implement** `config.rs` (reading parameters, `parameter_at`), the YAML answer in `Symfony::file_targets` and `getParameter` in `string_targets`.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(symfony): navigate from parameter names`.

### Task 2: Strings inside attributes

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Consumes: `Symfony::parameter_targets`, `config::parameter_at` (Task 1).
- Produces:
  - `infer::attribute_string_at(path, summary, source, offset) -> Option<AttributeString>`, with `pub(crate) struct AttributeString { pub attribute: String, pub key: Option<String>, pub position: usize, pub value: String, pub start: u32 }`: the string literal under `offset` that is an argument of an attribute anywhere in the file (class, method, property, function, parameter), the attribute's fully qualified class name, the argument's name or position, and where the value starts.
  - `Framework::attribute_targets(&self, index: &Index, found: &AttributeString, offset: u32) -> Vec<(Location, String)>`; the Drupal module answers nothing.
  - `Index::resolve` asks `frameworks_for(path)` before PHP name resolution when the cursor is in such a literal.

- [ ] **Step 1: Write the failing tests.** Add the `Autowire` attribute stub and `src/Service/Configured.php`, whose constructor has promoted parameters with `#[Autowire(param: 'app.name')]`, `#[Autowire('%app.limit%')]`, `#[Autowire('%kernel.project_dir%/x')]` and `#[Autowire(env: 'APP_X')]`:
  - `autowire_parameters_lead_to_their_definitions`: definition on `app.name` and on `app.limit` gives their keys; on `kernel.project_dir` and `APP_X`, nothing.

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-index --test symfony autowire`. Expected: FAIL.

- [ ] **Step 3: Implement** the hook and the Symfony `#[Autowire]` answer for parameters; move the `#[Template]` answer onto the hook when that removes the cursor gating of step 6c, keeping its tests green.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(frameworks): answer strings inside attributes`.

### Task 3: Services and YAML navigation

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/config.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/projects.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`, `crates/phplumb-index/tests/fixtures/projects/**`

**Interfaces:**

- Produces: `Symfony::service_targets(&self, index: &Index, id: &str) -> Vec<(Location, String)>`: the service's key, or the class when the id names one and no key defines it, following aliases to their target's key.

- [ ] **Step 1: Write the failing tests.** In the fixture's `services.yaml`: `App\Service\Mailer: ~`, `app.mailer: '@App\Service\Mailer'`, `app.optional: '@?app.mailer'`, `app.computed: '@=service("app.mailer")'`, `App\Service\Notifier: { class: App\Service\Mailer, arguments: ['@app.mailer'] }`, `app.tagged: !service { class: App\Service\Mailer }`, a comment naming `App\Service\Mailer`, and the prototype `App\: { resource: '../src/' }`; in `messenger.yaml`, `framework: { messenger: { routing: { App\Message\SendEmail: async } } }`; in `Configured.php`, `#[Autowire(service: 'app.mailer')]`:
  - `yaml_class_names_open_their_class`: the service key `App\Service\Mailer`, the `class:` value, the class in `!service`, and the routing key `App\Message\SendEmail` open their class.
  - `yaml_service_references_open_their_definition`: `@app.mailer` in `arguments` opens the key `app.mailer`; `@App\Service\Mailer` the key `App\Service\Mailer`; `@?app.mailer` the key `app.mailer`; `@=service(...)` nothing.
  - `autowire_service_opens_its_definition`: definition on `'app.mailer'` in `#[Autowire(service: ...)]` gives the key `app.mailer`.
  - `yaml_comments_give_nothing`: the class in the comment gives nothing.
  - In `tests/projects.rs`, `drupal_and_symfony_yaml_stay_apart`: in the `projects` fixture, `@demo.thing` written in a Symfony `config/services.yaml` gives nothing, and `demo.services.yml` of the Drupal project keeps its Drupal answers.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony --test projects`. Expected: the tests that expect a target FAIL.

- [ ] **Step 3: Implement** reading service definitions, the YAML class and `@id` answers in `file_targets`, and `#[Autowire(service:)]` in `attribute_targets`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate symfony yaml configuration`.

### Task 4: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Build the 6d baseline** from `git merge-base main HEAD`.

- [ ] **Step 2: Probe** every `#[Autowire(param:)]` and `#[Autowire('%x%')]` name and every `@id` and class name in the Symfony reference project's `config/services.yaml`, count those that resolve, and check those that do not by hand. Add cases: an `#[Autowire(param:)]` name, a `%x%` in YAML, a class key in `services.yaml`, an `@id` alias, and a `kernel.project_dir` name expecting nothing.

- [ ] **Step 3: Run both benchmarks and both diagnostics sweeps,** for this branch and the baseline, alternating their order. Expected: the Drupal reference project unchanged; Symfony coverage, cases and diagnostics at least the baseline's; cold index under 10000 and `peak_rss_mb` under 1024. If a target is missed, stop and report to the user.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6e Symfony parameters and YAML` to `docs/benchmarks.md`, README bullets under Symfony, and to `docs/backlog.md` anything this step defers, such as services from bundles, XML and PHP configuration, `_instanceof`, and parameters set in PHP. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record symfony parameter and yaml results`.
