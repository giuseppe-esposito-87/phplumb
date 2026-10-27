# PHPlumb 4d: Drupal plugins implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on a plugin id opens its plugin class: `'#type' => 'textfield'`, `BaseFieldDefinition::create('string')`, `setDisplayOptions('form', ['type' => 'string_textfield'])`, `addConstraint('UniqueField')` and `createInstance('id')` on a plugin manager. Go to implementations on a plugin manager lists all its plugins.

**Architecture:** The Drupal module reads plugin types from the constructors of `DefaultPluginManager` subclasses (subdirectory, attribute class, annotation class), then plugins from the classes in that subdirectory of every module's namespace, by attribute or annotation, with the class metadata of step 4b. A built-in table maps id contexts (call arguments, array keys) to plugin managers. Core gains two framework-agnostic entry points: string literals that are values of array keys, with the call around the array, and implementations a framework adds to a class.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, item 4; registries exclude test modules; anti-goal: a wrong target is worse than no target).

**Scope:** Fourth part of delivery step 4. Out of scope: plugin ids in YAML (migrations, config, views), plugin managers whose subdirectory is computed (`"Plugin/migrate/$type"`), plugins altered or added by `alterInfo()` hooks, ids built at run time, and plugins of themes.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, no client names or paths in committed files, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Registration point:** core changes stay framework-agnostic. Drupal knowledge lives in `frameworks/drupal`. The extension registry of step 4c is shared by hooks and plugins.
- **Plugin types:** every class that is or extends `Drupal\Core\Plugin\DefaultPluginManager` and whose own `__construct` calls `parent::__construct` with a string literal subdirectory (argument 0). The attribute class is argument 4 (`X::class` or a string), else `Drupal\Component\Plugin\Attribute\Plugin`; the annotation class is argument 5 (a string), else `Drupal\Component\Annotation\Plugin`. A manager whose subdirectory is not a literal is skipped.
- **Plugins of a type:** class-likes outside test modules whose namespace is `Drupal\{extension}\{subdirectory}` or below, for every module and profile on disk, or `Drupal\Core\{subdirectory}` or `Drupal\Component\{subdirectory}` or below (the subdirectory's `/` read as `\`). The class needs an attribute whose class is the type's attribute class or extends it; failing that, a docblock annotation whose class is the type's annotation class or extends it. The plugin id is the `id` argument, named or at position 0, a string.
- **Id lookup:** the plugins of the manager with that exact id; failing that, for an id `base:derivative`, the plugins with id `base`. Each target is the plugin class name, with that name as the expected text.
- **Id contexts:**

  | Context                                                                                                                                                                | Plugin manager                                                      |
  | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
  | Argument 0 of `createInstance`, `getDefinition` or `hasDefinition` on a receiver that is a plugin manager class, or an interface exactly one plugin manager implements | that manager                                                        |
  | Argument 0 of `Drupal\Core\Field\BaseFieldDefinition::create`                                                                                                          | `Drupal\Core\Field\FieldTypePluginManager`                          |
  | Argument 0 of `addConstraint` on a `Drupal\Core\TypedData\DataDefinitionInterface` or `Drupal\Core\Field\FieldConfigInterface`                                         | `Drupal\Core\Validation\ConstraintManager`                          |
  | The value of the array key `#type`                                                                                                                                     | `Drupal\Core\Render\ElementInfoManager`                             |
  | The value of key `type` in argument 1 of `setDisplayOptions` on a `Drupal\Core\Field\BaseFieldDefinition`, with argument 0 `'form'` or `'view'`                        | `Drupal\Core\Field\WidgetPluginManager` or `FormatterPluginManager` |

- **Implementations:** on a plugin manager class, or on an interface exactly one plugin manager implements, every plugin of that manager is added to the existing results.
- **Property cache:** the index empties its property assignment cache after the frameworks rebuild, as it does after they update, since rebuilding now runs inference.
- **Targets on the reference project:** member coverage in `web/modules/custom` stays at or above 42,455 accesses; cold index under 10 seconds; memory under 1 GB; `definition_ms` under 1000.

## Review focus

- **An id shared by two plugin types** (`string` is a field type and a formatter): `BaseFieldDefinition::create('string')` gives the field type only, `setDisplayOptions('view', ['type' => 'string'])` the formatter only. Test in Task 3.
- **A `type` key outside a display options call** (`['type' => 'string']` in any other array, 173 in custom code) gives nothing. Test in Task 3.
- **An attribute subclass:** `#[FormElement('textfield')]` is a plugin of the type declared with the `RenderElement` attribute. Test in Task 1.
- **A class with a plugin attribute outside the type's subdirectory** is not a plugin of that type. Test in Task 1.
- **A generic receiver:** `createInstance('x')` on `PluginManagerInterface`, which every manager implements, gives nothing. Test in Task 2.

---

### Task 1: Plugin registry

**Files:**

- Create: `crates/phplumb-index/src/frameworks/drupal/plugins.rs`, `crates/phplumb-index/src/frameworks/drupal/extensions.rs` (the extension registry, moved out of `hooks.rs`)
- Modify: `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/hooks.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `ClassShape::metadata` (step 4b), `infer::calls_named` per file and `FoundCall::caller` (step 4c), the extension registry (step 4c).
- Produces:

```rust
// phplumb_index::frameworks::drupal
pub struct PluginType {
    /// The plugin manager class.
    pub manager: String,
    pub subdir: String,
    pub attribute: String,
    pub annotation: String,
}

pub struct Plugin {
    pub id: String,
    pub manager: String,
    pub class: String,
    pub path: PathBuf,
    /// The class name.
    pub start: u32,
    pub end: u32,
}

impl Drupal {
    /// Every plugin type, sorted by manager.
    pub fn plugin_types(&self) -> &[PluginType];
    /// Every plugin of `manager`, sorted by path and offset.
    pub fn plugins_of(&self, manager: &str) -> &[Plugin];
    /// The plugins of `manager` an id names, by the id lookup rule.
    pub fn plugins(&self, manager: &str, id: &str) -> Vec<&Plugin>;
}
```

- [ ] **Step 1: Write the failing tests** on the fixture, extended with stubs for `DefaultPluginManager`, a `BlockManager` (`Plugin/Block`, a `Block` attribute, a `Block` annotation), an `ElementInfoManager` (`Element`, a `RenderElement` attribute and annotation, a `FormElement` attribute extending `RenderElement`), a manager with a computed subdirectory, plugins in `mymodule` (attribute) and `gadget` (annotation), a block in the test module, and a class with a `#[Block]` attribute in `mymodule/src/Plugin/Other`:
  - `plugin_types()` lists the block and element managers with their subdirectories and classes, not the computed one.
  - `plugins_of(BlockManager)` lists the attribute and annotation blocks, not the test module's, not the one outside `Plugin/Block`.
  - `plugins(ElementInfoManager, "textfield")` gives the `#[FormElement('textfield')]` class; `plugins(BlockManager, "menu_block:main")` gives the `menu_block` plugin.
  - Editing a plugin's id and updating the index changes `plugins()`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the registry**, rebuilt on `rebuild` and `update`, and the property cache emptied after the frameworks rebuild.

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): read drupal plugin types and plugins`.

### Task 2: Plugin ids in call arguments

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/plugins.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `Drupal::plugins` (Task 1), `Framework::string_targets`.
- Produces: string targets for the call contexts of the id context table.

- [ ] **Step 1: Write the failing tests** (needle offsets, as for hook strings), with stubs for `PluginManagerInterface`, `BaseFieldDefinition::create`, `DataDefinitionInterface::addConstraint`, a `FieldTypePluginManager` with a `string` field type and a `ConstraintManager` with a `UniqueField` constraint:
  - `$blocks->createInstance('my_block')` on a `BlockManager` parameter, and `\Drupal::service('plugin.manager.block')->getDefinition('my_block')`: the plugin class.
  - `createInstance('my_block')` on a `PluginManagerInterface` parameter: nothing.
  - `BaseFieldDefinition::create('string')`: the field type class.
  - `->addConstraint('UniqueField')` on a `BaseFieldDefinition`: the constraint class.
  - An unknown id: nothing.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL.

- [ ] **Step 3: Implement the call contexts.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate drupal plugin ids in calls`.

### Task 3: Plugin ids as array values

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/plugins.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`, `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
pub trait Framework {
    // existing methods
    /// Targets of a string literal that is the value of the array key `key`, each
    /// with the text expected at its range. `call` is the call whose positional
    /// argument at the given position holds the array, if any.
    fn keyed_string_targets(
        &self,
        index: &Index,
        key: &str,
        value: &str,
        call: Option<(&FrameworkCall<'_>, usize)>,
    ) -> Vec<(Location, String)>;
}
```

`Index::resolve` asks the frameworks after call string arguments: for the innermost `'key' => 'value'` array element whose key and value are string literals and whose value covers the cursor.

- [ ] **Step 1: Write the failing tests**, with a `FormatterPluginManager` and a `WidgetPluginManager` stub, a `string` formatter, a `string_textfield` widget, and a form array in `mymodule`:
  - `'#type' => 'textfield'`: the `FormElement` class.
  - `setDisplayOptions('form', ['type' => 'string_textfield'])`: the widget; `setDisplayOptions('view', ['type' => 'string'])`: the formatter only, while `BaseFieldDefinition::create('string')` still gives the field type only.
  - `['type' => 'string']` in a plain array: nothing.
  - A VS Code test: Cmd+click on a `#type` value in the `basic` fixture opens the element class.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement the keyed strings and the Drupal answers.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate drupal plugin ids in render and display arrays`.

### Task 4: Plugin managers to their plugins

**Files:**

- Modify: `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
pub trait Framework {
    // existing methods
    /// Implementations the framework adds to the class-like `class`, each with the
    /// text expected at its range.
    fn class_implementations(&self, index: &Index, class: &str) -> Vec<(Location, String)>;
}
```

`Index::implementations` on a class-like, declared or referenced, adds these to its subtypes, sorted and deduplicated.

- [ ] **Step 1: Write the failing tests**:
  - Implementations on `BlockManager` (its declaration and a reference): every block plugin of Task 1.
  - On a `BlockManagerInterface` only `BlockManager` implements: `BlockManager` and every block plugin.
  - On `PluginManagerInterface`: its implementing classes only, no plugins.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL to compile.

- [ ] **Step 3: Implement it.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: list the plugins of drupal plugin managers`.

### Task 5: Reference cases, benchmarks and README

**Files:**

- Modify: `docs/benchmarks.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Add at least 6 cases**, each checked by hand: a `#type` in a custom form, `BaseFieldDefinition::create()` in a custom entity, a widget and a formatter in `setDisplayOptions()`, a custom or contrib `createInstance()` or `addConstraint()`, and implementations on a plugin manager with few plugins.

- [ ] **Step 2: Run the reference benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0 (earlier cases updated only after checking their new targets by hand); resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024; `definition_ms` under 1000. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 3: Add a dated section to `docs/benchmarks.md`**, move Plugins to the available Drupal features in `README.md`, and commit with message `docs: record drupal plugin results`.
