# PHPlumb: names in docblocks implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Names written in docblocks work like the same names in code: a class in a PHPDoc tag (`@var \Drupal\Core\Messenger\MessengerInterface`), a member or function in `@see` (`@see Foo::bar()`, `@see hook_form_alter()`), and an annotation name (`@Block(...)`, `@Translation(...)`) give go to definition, hover, go to implementations and find references.

**Architecture:** The file summary gains the names its docblocks write, with their byte ranges, resolved like names in code: classes in type positions, members and functions in `@see`-like tags, and annotation names. The index looks the cursor up in them after the names in code, so every feature that starts from "what is under the cursor" covers them; references also list them. An annotation name that its file's imports do not resolve goes to the frameworks, and the Drupal module resolves it as plugin discovery does. Docblock names stay out of the names that broken reference diagnostics, call hierarchy and `new` detection read.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Must have: "Go to definition for classes, interfaces, traits, enums, functions, methods, properties, constants and class constants"; "Hover with the resolved type, the signature and the docblock"; anti-goal: a wrong jump is worse than no jump).

**Scope:** Out of scope: names in prose (summaries, descriptions after the type), class names in annotation string values (`"class" = "Drupal\node\NodeForm"`), and diagnostics for unknown names in docblocks.

## Global constraints

- Everything in the earlier plans' global constraints still holds: a wrong jump is worse than no jump, so a word that is not certainly a name in one of the positions below leads nowhere.
- **Registration point:** core reads docblock names framework-agnostically; Drupal knowledge (unimported annotation names) lives in `frameworks/drupal`.
- **Docblocks read:** every `/** ... */` comment of the file, before declarations and inline (`/** @var Foo $x */`).
- **Tags read** (each also with a `phpstan-` or `psalm-` prefix):

  | Tags                                                                                                                                                                                            | Names                                                                                                    |
  | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
  | `var`, `param`, `return`, `throws`, `property`, `property-read`, `property-write`, `mixin`, `extends`, `implements`, `use`, `template-extends`, `template-implements`, `template-use`, `assert` | the classes of the leading type, read like `split_type` does                                             |
  | `method`                                                                                                                                                                                        | the classes of the return type after an optional `static`, and of the leading type of each parameter     |
  | `template`, `template-covariant`, `template-contravariant`                                                                                                                                      | the classes of the bound after `of`                                                                      |
  | `see`, `uses`, `covers`                                                                                                                                                                         | the leading value: a class, `Class::method()`, `Class::$property`, `Class::NAME`, or a function `name()` |

- **Classes in a type:** the names `parse_type` resolves to `TypeExpr::Named`, generic arguments included (`array<string, Foo>`, `Collection<Foo>`, `class-string<Foo>`). Names inside shapes (`{...}`) and callable signatures (`(...)`) are skipped, as `parse_type` skips them.
- **`@see` values:** `Class::name()` is a method, `Class::$name` a property, and `Class::NAME` without parentheses whichever of a method or a constant the class has (PHPUnit writes `@covers Foo::bar`). `self::`, `static::` and `parent::` name the enclosing class or its parent. A value `name()` without `::` is a function, with code's fallback to the global function for an unqualified name. A value holding `://` is skipped. Members are found as for `Foo::bar()` in code, inherited ones included.
- **Annotation names:** every annotation `metadata::annotations` reads in any docblock, nested ones included (`@Translation(...)` inside `@Block(...)`), outside `@code` examples, resolved by the file's imports. An unqualified name that no import resolves and no indexed class has goes to the frameworks. The Drupal module resolves it in `Drupal\Component\Annotation`, `Drupal\Core\Annotation` and the namespace of the annotation class of the plugin type whose subdirectory holds the annotated class, or of every plugin type when none does; only indexed classes count.
- **Resolution:** the file's names at the docblock's start, as for docblock types today (`FileNames::at`).
- **Kept apart:** docblock names never reach `FileSummary::names` or `static_refs`, so step 3 diagnostics, call hierarchy and `new` detection do not change.

## Review focus

- **Prose never links:** a name in a summary line or in a tag's description (`@param Foo $foo Uses a Bar.`, `@see Foo::run() for the Bar::stop() case`) leads nowhere. Tests in Tasks 1 and 2.
- **A type continued on the next line** (`@param array<string,` then ` *   Foo> $map`) gives the right class or nothing, never another word's class. Test in Task 1.
- **Non-ASCII text earlier in the docblock** (`Crée un élément.`) keeps ranges on the name. Test in Task 1.
- **An annotation example inside `@code` ... `@endcode`**, common in Drupal API docs, leads nowhere. Test in Task 3.
- **A name mentioned only in a docblock** is found by references after the file is edited and the index updated. Tests in Tasks 1 and 3.

---

### Task 1: Classes in docblock types

**Files:**

- Modify: `crates/phplumb-index/src/phpdoc.rs`, `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/references.rs`
- Test: `crates/phplumb-index/tests/docblocks.rs` (new; marker fixtures cannot sit inside a docblock, so tests find offsets by needle)

**Interfaces:**

- Produces:

```rust
// phpdoc.rs
/// The class names a docblock writes in type positions and as `@see`-like
/// values, by the rules of the global constraints: byte ranges relative to
/// `text`, fully qualified names without a leading backslash.
pub fn doc_class_names(text: &str, names: &dyn NameContext) -> Vec<(usize, usize, String)>;

// summary.rs, FileSummary
/// Names written in docblocks, sorted by start: classes in type positions and
/// `@see`-like values, and (Task 2) functions in `@see`-like values. Kept apart
/// from `names`.
pub doc_names: Vec<NameRef>,
```

`ref_keys` includes the `RefKey` of every docblock name.

- [ ] **Step 1: Write the failing tests** in `tests/docblocks.rs`, on a workspace with `lib/` declaring `Lib\Foo`, `Lib\FooInterface` (implemented by `Lib\Foo`), `Lib\FooException`, `Lib\Base` and `Lib\Sub\Thing`, and `Use.php` in namespace `App` importing `Lib\Foo` and `Lib\FooInterface`:
  - `docblock_class_names_lead_to_their_declarations`: the definition of each of these leads to its class: `@var \Lib\FooInterface`, `@param Foo $foo`, `@return Foo[]|null` (on `Foo`), `@throws \Lib\FooException`, `@param array<string, Foo> $map`, `@property Foo $p`, `@method Foo make(FooInterface $f)` (both names), `@template T of Foo`, `@see Foo`, `@see Foo::run()` (on `Foo`), `@phpstan-param Foo $x`, `@mixin Foo`, `@extends \Lib\Base<Foo>` (both names), an inline `/** @var Foo $x */` inside a method, and `@var \Lib\Sub\Thing`.
  - `docblock_words_that_are_not_classes_lead_nowhere`: `int`, `$this`, `null`, the variable `$foo`, `T` in `@return T`, a summary line `Builds a Foo.`, the description in `@param Foo $foo Uses a Bar.` (on `Bar`), `@see https://example.com/Foo`, and `@var \Nowhere\Missing`.
  - `continued_types_lead_to_the_right_class_or_nowhere`: in `@param array<string,` / ` *   Foo> $map`, the cursor on `Foo` gives `Lib\Foo` or nothing.
  - `non_ascii_text_keeps_docblock_ranges`: after a summary line `Crée un élément.`, `@var Foo` leads to `Lib\Foo`.
  - `braced_namespaces_resolve_their_own_docblocks`: a file with `namespace Lib { /** @var Foo */ ... }` and `namespace Other { /** @var Foo */ ... }` leads to `Lib\Foo` and to nothing.
  - `hover_on_a_docblock_class_name_shows_the_class`: hover on `FooInterface` in `@var \Lib\FooInterface` contains `interface FooInterface`.
  - `implementations_of_a_docblock_interface_name_are_its_implementers`: implementations at that name give `Lib\Foo`.
  - `references_of_a_class_include_docblock_mentions`: references of `Lib\FooException` from its declaration include the `@throws` name; `candidate_files(RefKey::Class("Lib\\FooException"))` holds `Use.php`.
  - `docblock_only_mentions_follow_updates`: after `Use.php` is rewritten so `Lib\Base` appears only in a new docblock and the index updated, references of `Lib\Base` include it.
  - `docblock_names_are_not_diagnosed`: `Index::diagnostics` on a file with `@var \Nowhere\Missing` gives nothing.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test docblocks`. Expected: FAIL (no target, no hover, no reference at docblock names); `docblock_names_are_not_diagnosed` and the lead-nowhere test may pass already.

- [ ] **Step 3: Implement it.** `TypeParser` records the byte range of each name it resolves to `TypeExpr::Named`; `parse_type` ignores them, `doc_class_names` keeps them. `summarize` reads every docblock of `program.trivia` (`TriviaKind::DocBlockComment`). The lookup `Index::targets` starts from tries `doc_names` when no name in code is at the offset; `references.rs` adds `doc_names` to a class's occurrences in a file.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate class names in docblocks`.

### Task 2: Members and functions in `@see`

**Files:**

- Modify: `crates/phplumb-index/src/phpdoc.rs`, `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/references.rs`
- Test: `crates/phplumb-index/tests/docblocks.rs`

**Interfaces:**

- Consumes: `FileSummary::doc_names` (Task 1).
- Produces:

```rust
// summary.rs
/// A member named in a `@see`-like docblock value: `Foo::bar()`, `Foo::$bar`,
/// `Foo::BAR`. The range covers the member name as written.
pub struct DocMemberRef {
    pub class: ClassSel,
    pub name: String,
    /// `None` for `Foo::NAME` without parentheses: a method or a constant.
    pub kind: Option<MemberKind>,
    pub start: u32,
    pub end: u32,
}

// FileSummary
pub doc_members: Vec<DocMemberRef>,
```

Functions in `@see`-like values go to `doc_names` with kind `Function`, `qualified` and `imported` set as for code. `ref_keys` includes `RefKey::Member` of every docblock member.

- [ ] **Step 1: Write the failing tests** in `tests/docblocks.rs`, with `Lib\Base` declaring `inherited()`, `Lib\Foo extends Base` declaring `run()`, `$name` and `LIMIT`, and functions `Lib\helper()` and global `global_helper()`:
  - `see_members_lead_to_their_declarations`: `@see Foo::run()` leads to `run`, `@see Foo::$name` to `$name`, `@see \Lib\Foo::LIMIT` to `LIMIT`, `@covers \Lib\Foo::run` to `run`, `@see Foo::inherited()` to `Base::inherited`, and `@see self::run()` inside `Foo`'s own docblocks to `run`.
  - `see_functions_lead_to_their_declarations`: `@see \Lib\helper()` leads to `Lib\helper`, and `@see global_helper()` in namespace `App` to the global function.
  - `see_words_that_are_not_members_lead_nowhere`: `@see Foo::missing()`, `@see https://example.com/a::b()`, and `Bar::stop()` in the description of `@see Foo::run() for the Bar::stop() case`.
  - `references_of_a_method_include_see_mentions`: references of `Foo::run` from its declaration include the `@see` and `@covers` names.
  - `hover_on_a_see_member_shows_the_member`: hover on `run` in `@see Foo::run()` contains `function run`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test docblocks`. Expected: FAIL on the new tests (no target at `@see` members and functions).

- [ ] **Step 3: Implement it.** `@see`-like values are parsed in `phpdoc.rs`; `summarize` fills `doc_members` and the function `doc_names`. The cursor lookup resolves a `DocMemberRef` like a static reference that never dispatches (`member_targets` of its class; for `kind: None`, methods, then constants when there is no method); `references.rs` counts docblock members whose targets include the member searched.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate members and functions in see tags`.

### Task 3: Annotation names

**Files:**

- Modify: `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/references.rs`, `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/{mod,plugins}.rs`, `crates/phplumb-index/tests/fixtures/drupal/**`
- Test: `crates/phplumb-index/tests/docblocks.rs`, `crates/phplumb-index/tests/frameworks.rs`

**Interfaces:**

- Produces:

```rust
// summary.rs, FileSummary
/// Annotation names written in docblocks, nested ones included, resolved by the
/// file's imports; sorted by start.
pub annotation_names: Vec<NameRef>,

// RefKey
/// An unqualified annotation name no import resolves, by its lowercased short
/// name: the frameworks may resolve it to any class with that short name.
Annotation(String),

pub trait Framework {
    // existing methods
    /// The classes an unqualified annotation name in `path` may be when no import
    /// resolves it and no indexed class has its name.
    fn annotation_classes(&self, index: &Index, path: &Path, name: &NameRef) -> Vec<String> {
        Vec::new()
    }
}
```

- [ ] **Step 1: Write the failing tests:**
  - In `tests/docblocks.rs`: `imported_annotation_names_lead_to_their_class` (`use Lib\Ann\Route;` and `/** @Route("/x") */` lead to `Lib\Ann\Route`); `annotation_examples_lead_nowhere` (an `@Route("/x")` between `@code` and `@endcode` leads nowhere).
  - In `tests/frameworks.rs`, with the fixture gaining a `Drupal\Core\Annotation\Translation` stub and the unimported `@Block` of `UnimportedBlock` gaining `admin_label = @Translation("Unimported")`:
    - `unimported_annotation_names_lead_to_drupal_annotation_classes`: `@Block` leads to `Drupal\Core\Block\Annotation\Block` and the nested `@Translation` to `Drupal\Core\Annotation\Translation`.
    - `references_of_an_annotation_class_include_unimported_uses`: references of `Drupal\Core\Annotation\Translation` from its declaration include the nested `@Translation` of `UnimportedBlock`.
    - `annotation_names_follow_updates`: after a new plugin class with `@Translation(...)` is written and the index updated, those references include it.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test docblocks --test frameworks`. Expected: FAIL on the new tests.

- [ ] **Step 3: Implement it.** `summarize` runs `metadata::annotations` on every docblock and fills `annotation_names` from the annotations and their nested ones, adding `RefKey::Annotation` for unqualified names no import resolves. The cursor lookup tries `annotation_names` after `doc_names`: the classes of the resolved name, else every framework's `annotation_classes`. References of a class also read files with `RefKey::Annotation` of its short name and keep the annotation names that resolve to it. The Drupal module implements `annotation_classes` by the rule of the global constraints.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: navigate annotation names in docblocks`.

### Task 4: Editor test, benchmarks and README

**Files:**

- Modify: `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/src/**`, `docs/benchmarks.md`, `README.md`

- [ ] **Step 1: Write the failing VS Code test** "go to definition on a docblock class name reaches Target", on a new fixture file whose property docblock reads `@var Target`. Run it against the debug binary built before Task 1. Run: `npm --prefix editors/vscode test`. Expected: FAIL on the new test only.

- [ ] **Step 2: Rebuild and run it again.** Run: `cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 3: Run the reference benchmark.** Add definition cases in custom code on `\Drupal\Core\Messenger\MessengerInterface` in a `@var`, on the method of a `@see Class::method()`, and on an unimported `@Block` or `@Translation`. Reference cases may gain docblock mentions: check each new entry by hand and update their `expect`. Expected: `cases_failed` 0; resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024.

- [ ] **Step 4: Record and document.** Add a dated section to `docs/benchmarks.md`, extend the Go to definition line of `README.md` with names in docblocks and the Drupal section with annotation names, and commit with message `docs: record docblock name results`.
