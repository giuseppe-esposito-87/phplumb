# PHP Navigator 2c: implementations and type hierarchy implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Find implementations of interfaces, abstract classes and methods; show the type hierarchy (supertypes and subtypes) of a class or interface; and let inference take a member's type from an ancestor when the member itself has none (`{@inheritdoc}`).

**Architecture:** The index gains a reverse hierarchy table (parent or interface to its direct subtypes), kept in step by `insert` and `remove`. Implementations of a class are its non-abstract subtypes, found transitively. Implementations of a method are, for each non-abstract subtype, the declaration that subtype actually uses (via the existing member lookup) when it lies outside the original class and is not abstract. The server answers `textDocument/implementation` and the three type hierarchy requests. Inference falls back to the nearest ancestor declaration that has a type.

**Tech Stack:** existing workspace; LSP 3.17 type hierarchy from `lsp-types` 0.97.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** Third part of delivery step 2. References and call hierarchy (2d), symbols, outline and Linux packaging (2e) have their own plans. Traits have no subtypes in the hierarchy; a method a class gets from a trait counts as that class's implementation.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main` with no push, never kill processes by name.
- **Hierarchy walks** are transitive, deduplicated and terminate on cycles.
- **Abstract** means a class with the `abstract` modifier, an interface, or a method that is `abstract` or declared in an interface.
- **Performance:** implementations of a widely implemented interface on the reference project (for example `Drupal\Core\Entity\EntityInterface`) answer in under 1 second; cold index stays under 10 seconds and memory under 1 GB.

## Review focus

- **An interface method implemented through a trait** must list the trait's method for each class using it, once. Test in Task 2.
- **An abstract class between the interface and the concrete class** must not appear as an implementation of the class, but its concrete method must appear as an implementation of the interface method. Test in Task 2.
- **A subtype overriding a method that an intermediate class already implements** must list both declarations, deduplicated. Test in Task 2.
- **Removing or changing a file** must drop its classes from the subtype table so they stop appearing as implementations. Test in Task 1.
- **A member with `{@inheritdoc}` and no type in a long hierarchy** must take the nearest typed ancestor's type, binding `static` to the receiver. Test in Task 4.

---

### Task 1: Abstract flags and the subtype table

**Files:**

- Modify: `crates/phpnav-index/src/summary.rs`, `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/members.rs`
- Test: `crates/phpnav-index/tests/hierarchy.rs`, `crates/phpnav-index/tests/summary.rs`

**Interfaces:**

- Produces:

```rust
pub struct ClassShape { /* existing */ pub is_abstract: bool }   // abstract class or interface
pub struct Member { /* existing */ pub is_abstract: bool }       // abstract method or interface method

impl Index {
    /// Direct subtypes: classes extending `fqn`, classes and enums implementing it,
    /// interfaces extending it.
    pub fn subtypes(&self, fqn: &str) -> Vec<ClassRef<'_>>;
    /// Every subtype, transitively, each once.
    pub fn all_subtypes(&self, fqn: &str) -> Vec<ClassRef<'_>>;
    /// Direct supertypes: the parent and the interfaces, as declared.
    pub fn supertypes(&self, fqn: &str) -> Vec<ClassRef<'_>>;
}
```

- [ ] **Step 1: Write the failing tests**

```rust
// tests/summary.rs
#[test] fn abstract_flags()  // "abstract class A { abstract function f(); function g() {} } interface I { function h(); }" -> A and I abstract; f and h abstract; g not

// tests/hierarchy.rs
#[test] fn subtypes_are_direct_and_all_subtypes_transitive()
// interface I; interface J extends I; abstract class A implements J; class B extends A; enum E implements I
// subtypes("I") == {J, E}; all_subtypes("I") == {J, A, B, E}; supertypes("B") == {A}; supertypes("A") == {J}
#[test] fn all_subtypes_survive_cycles()   // class A extends B; class B extends A: all_subtypes("A") terminates with {B}
#[test] fn update_drops_removed_subtypes() // delete B.php and update: all_subtypes("I") no longer contains B
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test hierarchy --test summary`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the flags, the table and the three lookups.** Keys are lowercased fully qualified names; `remove` drops a file's entries.

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): track subtypes and abstract declarations`.

### Task 2: Implementations

**Files:**

- Modify: `crates/phpnav-index/src/index.rs`
- Test: `crates/phpnav-index/tests/fixtures/implementations/**`, `crates/phpnav-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `all_subtypes`, `find_member`, `is_abstract` flags (Task 1); `definition` targets.
- Produces:

```rust
impl Index {
    /// Implementations of what the cursor is on. On a class-like (declaration or
    /// reference): its non-abstract subtypes, transitively. On a method (declaration,
    /// member access or static call): for each non-abstract subtype of the declaring
    /// class, the non-abstract declaration that subtype uses, when it is not the
    /// original. Sorted and deduplicated.
    pub fn implementations(&mut self, path: &Path, source: &[u8], offset: u32) -> Vec<Location>;
}
```

The fixture runner gets `run_implementation_fixture(name)`, the same marker format, calling `implementations` instead of `definition`.

- [ ] **Step 1: Write the failing fixture** `implementations` with cursors on:
  - an interface name in its declaration and in a type hint: every concrete implementing class, through an intermediate interface and an abstract class, but not the abstract class itself;
  - an interface method declaration and a call `$i->run()` on an interface-typed variable: the concrete implementations, including one provided by a trait used by two classes (two classes, one trait declaration: listed once), one in the abstract class used by its concrete subclass, and an override further down (both listed);
  - a concrete class with no subclasses: nothing;
  - an unknown receiver: nothing.
- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-index --test fixtures implementations`
Expected: FAIL to compile, `implementations` not found.

- [ ] **Step 3: Implement `implementations`.**

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): find implementations of classes and methods`.

### Task 3: Server and extension

**Files:**

- Modify: `crates/phpnav-server/src/lsp.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`
- Create: `editors/vscode/test-fixtures/basic/src/Shape.php`
- Test: `crates/phpnav-server/tests/lsp_hierarchy.rs`

**Interfaces:**

- Consumes: `implementations`, `subtypes`, `supertypes`, `classes_named` (Tasks 1 and 2), `definition`.
- Produces: capabilities `implementationProvider` and `typeHierarchyProvider`. `textDocument/prepareTypeHierarchy` returns one item per class-like the cursor resolves to, with `name` the short name, `detail` the fully qualified name, `kind` CLASS, INTERFACE, ENUM or STRUCT (traits), and `data` `{"fqn": "..."}`. `typeHierarchy/supertypes` and `typeHierarchy/subtypes` answer from `data.fqn`. The extension skips implementation and type hierarchy requests on documents with unsaved edits, like definition and hover.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn implementation_over_stdio()      // interface Shape; class Square implements Shape; cursor on Shape -> one location in Square.php
#[test] fn type_hierarchy_over_stdio()      // prepare on Square -> item detail "App\Square"; supertypes -> [Shape]; subtypes of Shape -> [Square]
```

```ts
test("go to implementations reaches the implementing class", ...) // vscode.executeImplementationProvider on Shape.php
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-server --test lsp_hierarchy`
Expected: FAIL.

- [ ] **Step 3: Implement the handlers and the middleware.**

- [ ] **Step 4: Run every suite**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phpnav-server && npm --prefix editors/vscode test`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: serve implementations and type hierarchy`.

### Task 4: Inherited member types

**Files:**

- Modify: `crates/phpnav-index/src/infer.rs`, `crates/phpnav-index/src/index.rs` or `members.rs`
- Test: `crates/phpnav-index/tests/fixtures/inheritdoc/**`, `crates/phpnav-index/tests/fixtures.rs`

**Interfaces:**

- Produces: `pub(crate) fn typed_ancestor(&self, class: &str, kind: MemberKind, name: &str) -> Option<(ClassRef<'_>, &Member)>` on `Index`: the nearest declaration of the member above `class` (parent chain first, then interfaces) whose type is known. Inference uses it when the member found has no type, binding `self` to that ancestor and `static` to the receiver.

- [ ] **Step 1: Write the failing fixture** `inheritdoc`: an interface method `@return $this`, an abstract base implementing it with `{@inheritdoc}` and no return type, a concrete entity class; cursors on a chained call after the untyped method reaching the concrete class's method; a property typed only on the interface's implementer parent; and an untyped member with no typed ancestor returning nothing.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-index --test fixtures inheritdoc`
Expected: FAIL.

- [ ] **Step 3: Implement `typed_ancestor` and use it in inference.**

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): take member types from typed ancestors`.

### Task 5: Reference cases and benchmarks

**Files:**

- Modify: `crates/phpnav-server/src/bench.rs`, `crates/phpnav-server/tests/bench.rs`, `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/php-navigator/cases/reference.toml`

**Interfaces:**

- Produces: case `kind = "implementation"` in the bench case file, compared exactly like definitions. The bench report adds `implementation_ms` when an implementation case exists: the slowest implementation case.

- [ ] **Step 1: Write the failing test** `bench_runs_implementation_cases` on the `implementations` fixture: one passing case and one failing case; exit 1, `cases_passed` 1.

- [ ] **Step 2: Run it and watch it fail.** Run: `cargo test -p phpnav-server --test bench`. Expected: FAIL, `unsupported case kind implementation`.

- [ ] **Step 3: Implement the case kind.**

- [ ] **Step 4: Add at least 6 implementation cases** to the reference case file, each verified by a text search for the implementing declarations: a custom interface with its custom implementations, a core interface implemented in custom code, an interface method implemented in custom classes, and one that expects nothing.

- [ ] **Step 5: Run the reference benchmark**

Run: `cargo build --release -p phpnav-server && ./target/release/php-navigator bench --root <reference-project> --cases "$HOME/.local/share/php-navigator/cases/reference.toml" --coverage web/modules/custom`
Expected: `cases_failed` 0; `implementation_ms` under 1000; cold index under 10000; `peak_rss_mb` under 1024; coverage at or above the 88.6% baseline.

If a target is missed, stop and report the numbers to the user.

- [ ] **Step 6: Add a dated section to `docs/benchmarks.md`** and commit with message `feat(server): benchmark implementation cases`.
