# PHPlumb 4c: Drupal hooks implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a hook implementation (a procedural `mymodule_form_alter()` or a `#[Hook('form_alter')]` method) opens the places that invoke the hook and its `hook_*` documentation; Cmd+click on the hook name in an invocation lists every implementation.

**Architecture:** The summary records the attributes of functions and methods, as it already does for classes. The index gains one framework-agnostic query, the method calls with given names across the index, with their resolved targets, their literal arguments and the function or method they are in; it scans candidate files at query time like references. The Drupal module keeps a registry of extensions (from `*.info.yml`), hook implementations and `*.api.php` documentation, rebuilt from the index. A new framework entry point answers definitions on declarations, and string targets now carry the text expected at each target. Hooks invoked under computed names are matched through a built-in table of Drupal core patterns and sites.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, item 3; registries exclude test modules; anti-goal: a wrong target is worse than no target).

**Scope:** Third part of delivery step 4. Hooks are found the way Drupal 11 discovers them, from every extension on disk; enabled extensions are not read. Out of scope: hook ordering attributes (`ReorderHook`, `RemoveHook`, `order:`), plugin manager alter hooks set through `alterInfo()` (step 4d), listing implementations from an invocation whose hook name is computed, and `hook_update_N` and `hook_post_update_NAME`, which nothing invokes through the module handler (their documentation remains a target).

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, no client names or paths in committed files, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Registration point:** core changes stay framework-agnostic (attributes, calls with arguments, the declaration entry point). Drupal knowledge lives in `frameworks/drupal`.
- **Extensions:** every `*.info.yml` outside test modules (a `tests` path component), hidden directories and `vendor`, as for services files. The name is the file name before `.info.yml`; the kind is its `type:` value, `module`, `profile` or `theme` (other types are skipped). A file belongs to the extension of its nearest ancestor directory holding an `.info.yml`.
- **Procedural implementations** follow Drupal's `HookCollectorPass` and `ThemeHookCollectorPass`:
  - Global functions only, in an extension's files with extension `.module`, `.inc`, `.install` or `.profile` for modules and profiles, `.inc`, `.theme` or `theme-settings.php` for themes, outside the extension's `tests`, `js` and `css` directories (themes also `templates`).
  - A function named `{extension}_{hook}` implements `hook` for that extension. The file's own extension is tried first, then every extension of the same group, longest name first. Modules and profiles form one group, themes the other.
  - Skipped: functions with a `Drupal\Core\Hook\Attribute\LegacyHook`, `LegacyModuleImplementsAlter` or `LegacyRequirementsHook` attribute; the hooks `install`, `install_tasks`, `install_tasks_alter`, `schema`, `uninstall`, `update_last_removed` and `update_dependencies`; names ending in `update_` and digits; every function declared after one with a `ProceduralHookScanStop` attribute in the same file.
- **Attribute implementations:** classes in an extension's `src/Hook/` directory. A `Drupal\Core\Hook\Attribute\Hook` attribute (or one of a class extending it) on a method implements its `hook` argument (named, or position 0, string literal only) with that method, or with its `method` argument when set. On the class, it implements it with `method`, or `__invoke` when `method` is empty. The extension is the `module` argument when set, else the owning extension.
- **Documentation:** global functions named `hook_*` in files ending `.api.php`, outside test modules. A doc name is the function name without `hook_`, placeholders included (`form_FORM_ID_alter`).
- **Invocations:** calls resolving to these methods, with the hook argument at the given position:

  | Interface                                      | Hook name                                                                                   | Alter type (hook is type + `_alter`) | Implementations      |
  | ---------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------ | -------------------- |
  | `Drupal\Core\Extension\ModuleHandlerInterface` | `invoke` 1, `invokeAll` 0, `invokeAllWith` 0, `invokeDeprecated` 2, `invokeAllDeprecated` 1 | `alter` 0, `alterDeprecated` 1       | modules and profiles |
  | `Drupal\Core\Theme\ThemeManagerInterface`      | `invoke` 1, `invokeAllWith` 0                                                               | `alter` 0, `alterForTheme` 1         | themes               |

  An alter type may be a string or an array of strings. `invoke` with a literal first argument reaches that extension's implementations only.

- **Target order:** invocation sites (project code first, then by path and offset), then documentation; from an invocation, implementations, then documentation.
- **Targets on the reference project:** member coverage in `web/modules/custom` stays at or above 42,455 accesses; cold index under 10 seconds; memory under 1 GB; the slowest definition case under 1 second.

## Review focus

- **A helper function in a `.module` file** named like `mymodule_helper()` matches a module prefix but has no invocation and no documentation: Cmd+click on its name still gives the declaration itself, as before. Test in Task 4.
- **Module names that prefix each other** (`mymodule` and `mymodule_extra`): `mymodule_extra_cron()` implements `cron` for `mymodule_extra`, never `extra_cron` for `mymodule`. Test in Task 3.
- **Modules and themes stay apart:** a theme's `mytheme_thing_info_alter()` lists only `ThemeManagerInterface` invocations, and a module's implementation only `ModuleHandlerInterface` ones. Test in Task 4.
- **An entity type placeholder matches only entity type ids:** `#[Hook('report_presave')]`, with no entity type `report`, gets nothing from `ENTITY_TYPE_presave`. Test in Task 5.
- **A `#[Hook]` class outside `src/Hook`**, which Drupal does not discover, is not listed from an invocation. Test in Task 3.

---

### Task 1: Function and method attributes in the summary

**Files:**

- Modify: `crates/phplumb-index/src/summary.rs`
- Test: `crates/phplumb-index/tests/summary.rs`, `crates/phplumb-index/tests/cache.rs`

**Interfaces:**

- Produces: `Declaration` gains `#[serde(default)] pub metadata: Vec<Metadata>` (a function's attributes; classes keep theirs in `ClassShape::metadata`), and `Member` gains `#[serde(default)] pub metadata: Vec<Metadata>` (a method's attributes). Attributes only, resolved like class attributes, with `span` covering each attribute.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn function_and_method_attributes_are_recorded()
// use Drupal\Core\Hook\Attribute\Hook; use Drupal\Core\Hook\Attribute\LegacyHook;
// #[LegacyHook] function mymodule_cron() {}
// class H { #[Hook('form_alter', module: 'other')] public function formAlter() {} }
// -> function metadata [LegacyHook]; method metadata [Hook] with arguments [(None, String("form_alter")), (Some("module"), String("other"))]
#[test] fn function_and_method_attributes_survive_the_cache() // tests/cache.rs: build, save, warm build -> same metadata
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test summary --test cache`. Expected: FAIL to compile.

- [ ] **Step 3: Record the attributes.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): record function and method attributes`.

### Task 2: Method calls with their literal arguments

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/mod.rs`
- Test: `crates/phplumb-index/tests/calls.rs`

**Interfaces:**

- Produces:

```rust
// phplumb_index::frameworks
/// A method call found in the index, with its literal arguments.
pub struct FoundCall {
    /// The method name at the call.
    pub location: Location,
    /// `Class::method` for every declaration the call resolves to.
    pub targets: Vec<CallTarget>,
    /// Positional arguments that are string literals or `X::class`, by position.
    pub strings: Vec<Option<String>>,
    /// The string literal elements of positional array arguments, by position.
    pub lists: Vec<Vec<String>>,
    /// The method or function holding the call: `Class::method` with the declaring
    /// class's fully qualified name, or the function's fully qualified name.
    pub caller: Option<String>,
}

impl Index {
    /// Instance, null-safe and static calls to methods named one of `names`, in
    /// every candidate file, sorted by path and offset.
    pub fn calls_named(&self, names: &[&str]) -> Vec<FoundCall>;
}
```

Candidate files come from `RefKey::Member` for each name and are read and inferred in parallel, like a references scan. Targets are resolved exactly as for definitions; a call on a receiver of unknown type has no targets.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn calls_named_lists_resolved_calls_with_literal_arguments()
// interface H { function invokeAll($hook); function alter($type, &$data); }
// class S { public function __construct(private H $h) {} public function run($x) {
//   $this->h->invokeAll('cron'); $this->h->alter(['a', 'b' . $x], $data); } }
// calls_named(&["invokeAll", "alter"]) -> 2 calls; targets H::invokeAll and H::alter;
// strings [Some("cron")] and [None, None]; lists [[]] and [["a"], []]; caller "S::run"
#[test] fn calls_on_receivers_of_unknown_type_have_no_targets() // $x->invokeAll('cron') -> one call, targets empty
#[test] fn calls_in_functions_name_the_function()                // function f(H $h) { $h->invokeAll('x'); } -> caller "f"
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test calls`. Expected: FAIL to compile.

- [ ] **Step 3: Implement `calls_named`.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): list method calls with literal arguments`.

### Task 3: Hook registry

**Files:**

- Create: `crates/phplumb-index/src/frameworks/drupal/hooks.rs`
- Modify: `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `Declaration::metadata`, `Member::metadata` (Task 1).
- Produces:

```rust
// phplumb_index::frameworks::drupal
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionKind { Module, Profile, Theme }

pub struct HookImplementation {
    pub hook: String,
    /// The module or theme the implementation is for.
    pub extension: String,
    pub kind: ExtensionKind,
    pub path: PathBuf,
    /// The function or method name, and its range.
    pub name: String,
    pub start: u32,
    pub end: u32,
}

pub struct HookDoc {
    /// The documented hook name, without `hook_`, placeholders included.
    pub name: String,
    pub path: PathBuf,
    pub start: u32,
    pub end: u32,
}

impl Drupal {
    /// Every implementation of `hook`, sorted by path and offset.
    pub fn hook_implementations(&self, hook: &str) -> &[HookImplementation];
    /// Every `hook_{name}` documentation function.
    pub fn hook_docs(&self, name: &str) -> &[HookDoc];
}
```

The registry is rebuilt from the index on `rebuild` and `update` (extensions are re-read when an `.info.yml` changes). The rules turning a function or a method into hooks are functions of the declaration and the extension list, so Task 4 applies them to the cursor file as it is in the editor.

- [ ] **Step 1: Write the failing tests** on the fixture, extended with `mymodule.info.yml`, `mymodule_extra.info.yml`, `gadget.info.yml`, a theme `themes/mytheme` (`mytheme.info.yml` with `type: theme`, `mytheme.theme`), stubs for the `Hook`, `LegacyHook` and `ProceduralHookScanStop` attributes, and `core.api.php` documenting `hook_cron`:
  - `hook_implementations("cron")` lists `mymodule_cron()` in `mymodule.module`, the `#[Hook('cron')]` method of `mymodule/src/Hook/MymoduleHooks.php`, a class-level `#[Hook('cron', method: 'tick')]` (the `tick` method) and `mymodule_extra_cron()` with extension `mymodule_extra`; not a `#[LegacyHook]` shim, not the test module's implementation, not a `#[Hook('cron')]` class outside `src/Hook`, not a function after a `#[ProceduralHookScanStop]` one.
  - A class-level `#[Hook('thing_info')]` on an invokable class gives its `__invoke`; `#[Hook('cron', module: 'gadget')]` has extension `gadget`.
  - `mymodule_update_10001()` and `mymodule_install()` implement nothing; `mytheme_preprocess_node()` implements `preprocess_node` with kind `Theme`.
  - `hook_docs("cron")` gives `hook_cron` in `core.api.php`.
  - Editing `mymodule.module` to add `mymodule_mail()` and updating the index adds an implementation of `mail`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the registry.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): read drupal hook implementations`.

### Task 4: Navigating hooks with literal names

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/hooks.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`, `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/**`
- Test: `crates/phplumb-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `Index::calls_named` (Task 2), `Drupal::hook_implementations`, `Drupal::hook_docs` (Task 3).
- Produces:

```rust
// phplumb_index::frameworks
/// The declaration a definition request is on: its name, or one of its attributes.
pub enum FrameworkDeclaration<'a> {
    Function { path: &'a Path, declaration: &'a Declaration },
    Method { path: &'a Path, class: &'a Declaration, member: &'a Member },
    /// One of the class's own attributes.
    Class { path: &'a Path, class: &'a Declaration, metadata: &'a Metadata },
}

pub trait Framework {
    // existing methods, with string_targets now returning each target with the text
    // expected at its range (compared case-insensitively by the stale-target check)
    fn string_targets(&self, index: &Index, call: &FrameworkCall<'_>, argument: usize, value: &str) -> Vec<(Location, String)>;
    /// Targets of a definition request on a declaration, when the framework knows
    /// what it stands for.
    fn declaration_targets(&self, index: &Index, declaration: &FrameworkDeclaration<'_>) -> Vec<(Location, String)>;
}
```

`Index::resolve` asks the frameworks when the cursor is on a function or method declaration's name, or inside one of a function's, method's or class's attributes but not on a class-like name. Their targets replace the declaration itself; with none, the current behavior stays. Services and entities return their ids as the expected text. `infer::string_argument_at` treats a string literal element of an array argument as that argument: the argument's position, the element's value.

The Drupal module answers a declaration with the invocations of each hook it implements (the global constraints' table, the literal hook name, the implementation's group) and its documentation (`hook_docs` of the exact name). It answers a hook name or alter type in an invocation with the implementations of the group (filtered by extension for `invoke` with a literal first argument) and the documentation. Expected texts: the method name at a call, the function or method name of an implementation, the `hook_*` name of a doc.

- [ ] **Step 1: Write the failing fixture cases**, with a `ModuleHandlerInterface` stub, a `ThemeManagerInterface` stub, `\Drupal::moduleHandler()` in the `Drupal` stub, and a service in `mymodule` calling `invokeAll('cron')`, `invoke('mymodule', 'ping')`, `alter('thing_info', $info)` on the module handler and on the theme manager, and `alter(['thing_data', 'other'], $data)`:
  - Cursor on `mymodule_cron`, on the `#[Hook('cron')]` method name, and inside its `#[Hook('cron')]` attribute: targets the `invokeAll` call and `hook_cron`.
  - Cursor on `'cron'` in `invokeAll('cron')`: every implementation of `cron` from Task 3 and `hook_cron`.
  - Cursor on `'ping'` in `invoke('mymodule', 'ping')`: `mymodule_ping()` only, not `gadget_ping()`.
  - Cursor on `'thing_data'` inside the array: the implementations of `thing_data_alter`.
  - Cursor on `mytheme_thing_info_alter`: the theme manager's `alter` call only; cursor on `mymodule_thing_info_alter`: the module handler's only.
  - Cursor on `mymodule_helper` (no invocation, no documentation): the declaration itself.
  - A VS Code test: Cmd+click on a hook implementation in the `basic` fixture opens its invocation.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test fixtures drupal`. Expected: FAIL.

- [ ] **Step 3: Implement the entry point and the Drupal answers.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate drupal hooks`.

### Task 5: Hooks invoked under computed names

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/hooks.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/fixtures.rs`, `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: Task 4's Drupal answers, `FoundCall::caller` (Task 2), the entity type registry (step 4b).
- Produces: a built-in table of patterns and the sites invoking them, in `hooks.rs`. In a pattern, an uppercase run (`FORM_ID`, `ENTITY_TYPE`, `TAG`) is a placeholder: `ENTITY_TYPE` matches a known entity type id, `N` digits, any other one a non-empty run of lowercase letters, digits and underscores. A hook's invocations add every hook-invoking call (of its group) whose caller is a site of a matching pattern. Its documentation is `hook_docs` of the exact name when there is one, else every documented name with placeholders that matches, including patterns not in the table (contrib ones).

| Patterns                                                                                                                                                                                                         | Sites                                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `ENTITY_TYPE_X` and `entity_X` for X in `create`, `presave`, `insert`, `update`, `predelete`, `delete`, `revision_delete`, `translation_create`, `translation_insert`, `translation_delete`, `field_values_init` | `Drupal\Core\Entity\EntityStorageBase::invokeHook`, `Drupal\Core\Config\Entity\ConfigEntityStorage::invokeHook`                      |
| `ENTITY_TYPE_load`                                                                                                                                                                                               | `Drupal\Core\Entity\EntityStorageBase::postLoad`                                                                                     |
| `ENTITY_TYPE_storage_load`                                                                                                                                                                                       | `Drupal\Core\Entity\ContentEntityStorageBase::invokeStorageLoadHook`                                                                 |
| `ENTITY_TYPE_revision_create`                                                                                                                                                                                    | `Drupal\Core\Entity\ContentEntityStorageBase::createRevision`                                                                        |
| `ENTITY_TYPE_duplicate`                                                                                                                                                                                          | `Drupal\Core\Entity\EntityBase::createDuplicate`, `Drupal\Core\Entity\ContentEntityBase::createDuplicate`                            |
| `ENTITY_TYPE_access`                                                                                                                                                                                             | `Drupal\Core\Entity\EntityAccessControlHandler::access`                                                                              |
| `ENTITY_TYPE_create_access`                                                                                                                                                                                      | `Drupal\Core\Entity\EntityAccessControlHandler::createAccess`                                                                        |
| `ENTITY_TYPE_view`, `ENTITY_TYPE_view_alter`                                                                                                                                                                     | `Drupal\Core\Entity\EntityViewBuilder::buildMultiple`                                                                                |
| `ENTITY_TYPE_build_defaults_alter`                                                                                                                                                                               | `Drupal\Core\Entity\EntityViewBuilder::viewMultiple`                                                                                 |
| `ENTITY_TYPE_view_mode_alter`                                                                                                                                                                                    | `Drupal\Core\Entity\EntityViewBuilder::getBuildDefaults`                                                                             |
| `ENTITY_TYPE_form_mode_alter`                                                                                                                                                                                    | `Drupal\Core\Entity\Entity\EntityFormDisplay::collectRenderDisplay`                                                                  |
| `ENTITY_TYPE_prepare_form`                                                                                                                                                                                       | `Drupal\Core\Entity\EntityForm::prepareInvokeAll`                                                                                    |
| `entity_query_ENTITY_TYPE_alter`, `entity_query_tag__TAG_alter`, `entity_query_tag__ENTITY_TYPE__TAG_alter`                                                                                                      | `Drupal\Core\Entity\Query\QueryBase::alter`                                                                                          |
| `form_alter`, `form_FORM_ID_alter`, `form_BASE_FORM_ID_alter`                                                                                                                                                    | `Drupal\Core\Form\FormBuilder::prepareForm`                                                                                          |
| `query_alter`, `query_TAG_alter`                                                                                                                                                                                 | `Drupal\Core\Database\Query\Select::preExecute`, `Drupal\Core\Entity\Plugin\EntityReferenceSelection\DefaultSelection::reAlterQuery` |
| `field_widget_single_element_WIDGET_TYPE_form_alter`                                                                                                                                                             | `Drupal\Core\Field\WidgetBase::formSingleElement`                                                                                    |
| `field_widget_complete_WIDGET_TYPE_form_alter`                                                                                                                                                                   | `Drupal\Core\Field\WidgetBase::form`                                                                                                 |
| `block_view_BASE_BLOCK_ID_alter`                                                                                                                                                                                 | `Drupal\block\BlockViewBuilder::buildPreRenderableBlock`                                                                             |
| `block_build_BASE_BLOCK_ID_alter`                                                                                                                                                                                | `Drupal\block\BlockViewBuilder::viewMultiple`                                                                                        |
| `theme_suggestions_HOOK`, `theme_suggestions_alter`, `theme_suggestions_HOOK_alter`                                                                                                                              | `Drupal\Core\Theme\ThemeManager::buildThemeHookSuggestions`                                                                          |
| `preprocess_HOOK`                                                                                                                                                                                                | `Drupal\Core\Theme\Registry::collectModulePreprocess`                                                                                |
| `plugin_filter_TYPE_alter`, `plugin_filter_TYPE__CONSUMER_alter`                                                                                                                                                 | `Drupal\Core\Plugin\FilteredPluginManagerTrait::getFilteredDefinitions`                                                              |
| `language_fallback_candidates_alter`, `language_fallback_candidates_OPERATION_alter`                                                                                                                             | `Drupal\language\ConfigurableLanguageManager::getFallbackCandidates`                                                                 |
| `migrate_MIGRATION_ID_prepare_row`                                                                                                                                                                               | `Drupal\migrate\Plugin\migrate\source\SourcePluginBase::prepareRow`                                                                  |
| `jsonapi_ENTITY_TYPE_filter_access`                                                                                                                                                                              | `Drupal\jsonapi\Access\TemporaryQueryGuard::getAccessResultsFromEntityFilterHook`                                                    |

- [ ] **Step 1: Write the failing tests**, with fixture stubs for `FormBuilder::prepareForm` (module handler and theme manager `alter($hooks, ...)`), `EntityStorageBase::invokeHook` (`invokeAll($this->entityTypeId . '_' . $hook)` and `invokeAll('entity_' . $hook)`), and docs for `hook_form_alter`, `hook_form_FORM_ID_alter`, `hook_form_BASE_FORM_ID_alter`, `hook_ENTITY_TYPE_presave` and `hook_entity_presave`:
  - Cursor on `mymodule_form_user_login_form_alter`: the module handler's `alter` call in `prepareForm`, `hook_form_FORM_ID_alter` and `hook_form_BASE_FORM_ID_alter`.
  - Cursor on `mytheme_form_alter`: the theme manager's `alter` call in `prepareForm` and `hook_form_alter`.
  - Cursor on a `#[Hook('thing_presave')]` method: both `invokeAll` calls in `invokeHook` and `hook_ENTITY_TYPE_presave`; on `#[Hook('entity_presave')]`: the same calls and `hook_entity_presave` only.
  - Cursor on a `#[Hook('report_presave')]` method: the declaration itself.
  - A unit-level test in `frameworks.rs` that a documented contrib pattern outside the table (`hook_gadget_widget_TYPE_alter` in `gadget.api.php`) is the only target of `gadget_gadget_widget_slider_alter()`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test fixtures drupal --test frameworks`. Expected: FAIL.

- [ ] **Step 3: Implement the pattern matcher and the table.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): resolve drupal hooks invoked under computed names`.

### Task 6: Reference cases, benchmarks and README

**Files:**

- Modify: `crates/phplumb-server/src/bench.rs`, `docs/benchmarks.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Time definitions in the bench:** the report gains `definition_ms`, the slowest definition case, like `references_ms`.

- [ ] **Step 2: Add at least 6 cases**, each checked by hand: a custom `#[Hook]` method with a literal invocation, a procedural hook in the custom theme, a `form_FORM_ID_alter` implementation, an `ENTITY_TYPE_presave` (or another `ENTITY_TYPE_` hook) implementation, a literal invocation string to its implementations, and an implementation whose only target is its documentation. Before adding them, confirm on the reference project that every site in Task 5's table resolves to an indexed method.

- [ ] **Step 3: Run the reference benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0 (earlier cases updated only after checking their new targets by hand); resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024; `definition_ms` under 1000. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 4: Add a dated section to `docs/benchmarks.md`**, move Hooks to the available Drupal features in `README.md`, and commit with message `docs: record drupal hook results`.
