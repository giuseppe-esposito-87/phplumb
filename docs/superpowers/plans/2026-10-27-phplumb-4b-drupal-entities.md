# PHPlumb 4b: Drupal entities implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Type `getStorage('node')` as the storage class of that entity type and `->load()` as its entity class, with entity types read from both PHP attributes and Doctrine annotations, so navigation continues through entity code.

**Architecture:** The summary records, for every class-like, its attributes and its Doctrine docblock annotations as small value trees with class names resolved: generic PHP metadata that the plugins step (4d) reuses. Frameworks receive the whole index when they rebuild or update, so the Drupal module builds an entity type registry from that metadata. A new `TypeExpr::Tagged` carries a class plus a framework tag through inference: `getStorage('node')` gives the storage class tagged with the entity type id, and storage methods such as `load()` read the tag from their receiver.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Drupal module, item 2; registries exclude test modules).

**Scope:** Second part of delivery step 4. Bundle classes set by `hook_entity_bundle_info_alter()` and entity type changes made by `hook_entity_type_alter()` are out of scope: the entity class is the declared one.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, no client names or paths in committed files, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Registration point:** core code changes in this plan stay framework-agnostic (class metadata, the tagged type, the index passed to frameworks). Drupal knowledge lives in `frameworks/drupal`.
- **Entity types:** declared by the attributes `Drupal\Core\Entity\Attribute\ContentEntityType`, `ConfigEntityType` or `EntityType`, or the annotations `@ContentEntityType`, `@ConfigEntityType` or `@EntityType` (resolved like class names, so `Drupal\Core\Entity\Annotation\...`). Storage is `handlers.storage`, or by default `Drupal\Core\Entity\Sql\SqlContentEntityStorage` for content and `Drupal\Core\Config\Entity\ConfigEntityStorage` for config entity types. Classes in test modules (a `tests` path component) are left out.
- **Storage methods typed from the tag:** `load`, `loadUnchanged`, `loadRevision` and `create` give the entity class; `loadMultiple` and `loadByProperties` give an array of it. Only on classes that are or implement `Drupal\Core\Entity\EntityStorageInterface`.
- **Targets on the reference project:** member coverage in `web/modules/custom` rises above the 91.6% baseline; cold index under 10 seconds; memory under 1 GB.

## Review focus

- **An entity type id defined twice** (core and a contrib override, or two modules) gives every storage and entity class: a union. Test in Task 2.
- **Annotations with nested annotations, multi-line values and comments** (`label = @Translation("Content")`, `handlers = { "storage" = "..." }`) parse; a broken annotation gives nothing instead of garbage. Test in Task 1.
- **The tag survives variables and branches:** `$storage = ...->getStorage('node'); if (...) { ... } $storage->load(1)->...` still resolves. Test in Task 3.
- **A storage typed without a tag** (a parameter typed `EntityStorageInterface`) keeps its declared return types. Test in Task 3.
- **The cache:** class metadata survives a warm start from the cache. Test in Task 1.

---

### Task 1: Class metadata in the summary

**Files:**

- Modify: `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/phpdoc.rs` (annotation parser) or a new `crates/phplumb-index/src/metadata.rs`
- Test: `crates/phplumb-index/tests/summary.rs`, `crates/phplumb-index/tests/cache.rs`

**Interfaces:**

- Produces:

```rust
// phplumb_index::metadata
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MetaValue {
    String(String),
    /// `X::class` in attributes; fully qualified, no leading backslash.
    Class(String),
    Bool(bool),
    Number(String),
    /// Arrays, `{ ... }` annotation maps and lists: keyed entries keep their key.
    List(Vec<(Option<String>, MetaValue)>),
    /// A nested attribute (`new X(...)`) or annotation (`@X(...)`).
    Nested(Box<Metadata>),
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    /// Fully qualified attribute or annotation class name.
    pub name: String,
    /// Named arguments by name; positional ones with no name.
    pub arguments: Vec<(Option<String>, MetaValue)>,
    /// Where the attribute or annotation name is in the file.
    pub start: u32,
    pub end: u32,
}

impl Metadata {
    pub fn argument(&self, name: &str) -> Option<&MetaValue>;
}
```

`ClassShape` gains `#[serde(default)] pub metadata: Vec<Metadata>`: the class's attributes, then its docblock annotations. Annotation names resolve through the file's imports like class names; values keep strings as written (Drupal annotations write classes as strings).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn attributes_are_recorded_with_resolved_values()
// namespace App; use Drupal\Core\Entity\Attribute\ContentEntityType; use App\Storage;
// #[ContentEntityType(id: 'thing', label: new T('X'), handlers: ['storage' => Storage::class, 'form' => ['default' => F::class]])] class Thing {}
// -> name "Drupal\Core\Entity\Attribute\ContentEntityType", id String("thing"), handlers.storage Class("App\Storage"), label Nested(T)
#[test] fn annotations_are_recorded_with_resolved_names()
// /** Docs.\n * @ContentEntityType(\n *   id = "thing",\n *   label = @Translation("Thing"),\n *   handlers = {\n *     "storage" = "App\Storage",\n *   },\n * ) */ with `use Drupal\Core\Entity\Annotation\ContentEntityType;`
// -> name "Drupal\Core\Entity\Annotation\ContentEntityType", id String("thing"), handlers.storage String("App\Storage"), label Nested(Translation)
#[test] fn broken_annotations_give_nothing()     // "@ContentEntityType(id = \"x\"" unterminated -> no metadata, no panic
#[test] fn metadata_survives_the_cache()          // tests/cache.rs: build, save, warm build -> same metadata on the class
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test summary --test cache`. Expected: FAIL to compile.

- [ ] **Step 3: Implement attribute and annotation capture.**

- [ ] **Step 4: Run the index suite.** Run: `cargo test -p phplumb-index`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): record class attributes and annotations`.

### Task 2: Entity type registry

**Files:**

- Create: `crates/phplumb-index/src/frameworks/drupal/entities.rs`
- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Consumes: `ClassShape::metadata` (Task 1).
- Produces: `Framework::rebuild(&mut self, index: &Index)` and `Framework::update(&mut self, index: &Index, changed, removed)` replace the `PathFilter` versions (`Index::filter()` gives the filter); the index runs them after its own build or update.

```rust
// phplumb_index::frameworks::drupal
pub struct EntityType {
    pub id: String,
    pub class: String,
    pub storage: String,
    pub path: PathBuf,
    /// The `id` value in the declaration.
    pub start: u32, pub end: u32,
}
impl Drupal {
    pub fn entity_types(&self, id: &str) -> &[EntityType];
}
```

- [ ] **Step 1: Write the failing tests** on the fixture, extended with: an attribute entity type with a custom storage, an annotation entity type without a storage handler (content default), a config entity type without one (config default), an id declared twice, and an entity type in a test module (ignored).

- [ ] **Step 2: Run them and watch them fail.** Expected: FAIL to compile.

- [ ] **Step 3: Implement the registry and the trait change.**

- [ ] **Step 4: Run the index suite.** Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): read drupal entity types`.

### Task 3: Typing storages and entities

**Files:**

- Modify: `crates/phplumb-index/src/phpdoc.rs` (`TypeExpr::Tagged`), `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/hover.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Test: `crates/phplumb-index/tests/fixtures/drupal/**`, `crates/phplumb-index/tests/fixtures.rs`

**Interfaces:**

- Produces:

```rust
pub enum TypeExpr { /* existing */
    /// A class carrying a framework tag, such as a storage that knows its entity type.
    Tagged { class: String, tag: String },
}
pub struct FrameworkCall<'a> { /* existing */
    /// The receiver's type, for method calls.
    pub receiver: Option<&'a TypeExpr>,
}
```

Everything that reads classes from a type (`class_names`, member lookups, hover) treats `Tagged` as its class; binding and unions keep the tag. The Drupal module answers `getStorage(id)` on classes that are or implement `Drupal\Core\Entity\EntityTypeManagerInterface` with `Tagged { storage, "drupal.entity:<id>" }` (a union for an id declared twice), and the storage methods of the global constraints from the receiver's tag.

- [ ] **Step 1: Write the failing fixture cases**: a method on `getStorage('thing')` (the custom storage), on `->load(1)` after it (the entity class), through a variable and an `if` branch, on an element of `loadMultiple()` in a `foreach`, on `\Drupal::entityTypeManager()->getStorage('thing')`, on the default storage of an annotation entity type, and on `load()` from a parameter typed `EntityStorageInterface` (the declared return type only).

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phplumb-index --test fixtures drupal`. Expected: FAIL.

- [ ] **Step 3: Implement the tagged type and the Drupal answers.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): type drupal entity storages`.

### Task 4: Entity type ids to their declaration

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/**`
- Test: `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces: Cmd+click on the id string of `getStorage('id')` opens every declaration's `id` value (the attribute argument or annotation property).

- [ ] **Step 1: Write the failing tests**: the index test (the id in `getStorage('thing')` leads to the `id` value in the entity class) and a VS Code test on a fixture entity type.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test frameworks`. Expected: FAIL.

- [ ] **Step 3: Implement it.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate drupal entity type ids`.

### Task 5: Reference cases, benchmarks and README

**Files:**

- Modify: `docs/benchmarks.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Add at least 5 cases**, each checked by hand: a method after `getStorage('node')->load()`, through a storage variable, on a custom entity type's storage, an entity type id string to its declaration, and a contrib annotation entity type.

- [ ] **Step 2: Run the reference benchmark.** Run: `cargo build --release -p phplumb-server && ./target/release/phplumb bench --root <reference-project> --cases "$HOME/.local/share/phplumb/cases/reference.toml" --coverage web/modules/custom`. Expected: `cases_failed` 0 (update earlier cases only after checking their new targets by hand); coverage above 91.6%; cold index under 10000; `peak_rss_mb` under 1024. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 3: Add a dated section to `docs/benchmarks.md`**, move Entities to the available Drupal features in `README.md`, and commit with message `docs: record drupal entity results`.
