# PHPlumb: class names in docblocks implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A class name written in a PHPDoc tag, such as `@var \Drupal\Core\Messenger\MessengerInterface`, works like the same name in code: go to definition, hover, go to implementations and find references.

**Architecture:** The file summary gains the class names its docblocks write in type positions, with their byte ranges, resolved like names in code. The index looks the cursor up in them after the names in code, so every feature that starts from "what is under the cursor" covers them; references also list them. They stay out of the names that broken reference diagnostics, call hierarchy and `new` detection read.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (Must have: "Go to definition for classes, interfaces, traits, enums, ..."; "Hover with the resolved type, the signature and the docblock"; anti-goal: a wrong jump is worse than no jump).

**Scope:** Out of scope: class names in prose (summaries, descriptions after the type), Drupal annotation names (`@Block(...)`), the member part of `@see Foo::bar()`, and diagnostics for unknown names in docblocks.

## Global constraints

- Everything in the earlier plans' global constraints still holds: a wrong jump is worse than no jump, so a word that is not certainly a class name in a type position leads nowhere.
- **Docblocks read:** every `/** ... */` comment of the file, before declarations and inline (`/** @var Foo $x */`).
- **Tags read** (each also with a `phpstan-` or `psalm-` prefix):

  | Tags                                                                                                                                                                                                                     | Type position                                                                      |
  | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
  | `var`, `param`, `return`, `throws`, `property`, `property-read`, `property-write`, `mixin`, `extends`, `implements`, `use`, `template-extends`, `template-implements`, `template-use`, `see`, `uses`, `covers`, `assert` | the leading type, read like `split_type` does                                      |
  | `method`                                                                                                                                                                                                                 | the return type after an optional `static`, and the leading type of each parameter |
  | `template`, `template-covariant`, `template-contravariant`                                                                                                                                                               | the bound after `of`                                                               |

- **Class names in a type:** the names `parse_type` resolves to `TypeExpr::Named`, generic arguments included (`array<string, Foo>`, `Collection<Foo>`, `class-string<Foo>`). Names inside shapes (`{...}`) and callable signatures (`(...)`) are skipped, as `parse_type` skips them. Text after `::` is ignored. A `@see` value holding `://` is skipped.
- **Resolution:** the file's names at the docblock's start, as for docblock types today (`FileNames::at`).
- **Kept apart:** docblock names never reach `FileSummary::names`, so step 3 diagnostics, call hierarchy and `new` detection do not change.

## Review focus

- **Prose never links:** a class name in a summary line or in a tag's description (`@param Foo $foo Uses a Bar.`) leads nowhere. Test in Task 1.
- **A type continued on the next line** (`@param array<string,` then ` *   Foo> $map`) gives the right class or nothing, never another word's class. Test in Task 1.
- **Non-ASCII text earlier in the docblock** (`Crée un élément.`) keeps ranges on the name. Test in Task 1.
- **A file with two braced namespaces** resolves each docblock in its own namespace. Test in Task 1.
- **A class mentioned only in a docblock** is found by references after the file is edited and the index updated. Test in Task 1.

---

### Task 1: Docblock class names

**Files:**

- Modify: `crates/phplumb-index/src/phpdoc.rs`, `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/src/references.rs`
- Test: `crates/phplumb-index/tests/docblocks.rs` (new; marker fixtures cannot sit inside a docblock, so tests find offsets by needle)

**Interfaces:**

- Produces:

```rust
// phpdoc.rs
/// The class names a docblock writes in type positions, by the rules of the
/// global constraints: byte ranges relative to `text`, fully qualified names
/// without a leading backslash.
pub fn doc_class_names(text: &str, names: &dyn NameContext) -> Vec<(usize, usize, String)>;

// summary.rs, FileSummary
/// Class names written in docblock type positions, sorted by start; kind
/// `ClassLike`. Kept apart from `names`.
pub doc_names: Vec<NameRef>,
```

`ref_keys` includes `RefKey::Class` of every docblock name.

- [ ] **Step 1: Write the failing tests** in `tests/docblocks.rs`, on a workspace with `lib/` declaring `Lib\Foo`, `Lib\FooInterface` (implemented by `Lib\Foo`), `Lib\FooException`, `Lib\Base` and `Lib\Sub\Thing`, and `Use.php` in namespace `App` importing `Lib\Foo` and `Lib\FooInterface`:
  - `docblock_class_names_lead_to_their_declarations`: the definition of each of these leads to its class: `@var \Lib\FooInterface`, `@param Foo $foo`, `@return Foo[]|null` (on `Foo`), `@throws \Lib\FooException`, `@param array<string, Foo> $map`, `@property Foo $p`, `@method Foo make(FooInterface $f)` (both names), `@template T of Foo`, `@see Foo::run()`, `@phpstan-param Foo $x`, `@mixin Foo`, `@extends \Lib\Base<Foo>` (both names), an inline `/** @var Foo $x */` inside a method, and `@var \Lib\Sub\Thing`.
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

- [ ] **Step 3: Implement it.** `TypeParser` records the byte range of each name it resolves to `TypeExpr::Named`; `parse_type` ignores them, `doc_class_names` keeps them. `summarize` reads every docblock of `program.trivia` (`TriviaKind::DocBlockComment`). `Index::targets` (or the lookup it starts from) tries `doc_names` when no name in code is at the offset; `references.rs` adds `doc_names` to a class's occurrences in a file.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate class names in docblocks`.

### Task 2: Editor test, benchmarks and README

**Files:**

- Modify: `editors/vscode/src/test/definition.test.ts`, `editors/vscode/test-fixtures/basic/src/**`, `docs/benchmarks.md`, `README.md`

- [ ] **Step 1: Write the failing VS Code test** "go to definition on a docblock class name reaches Target", on a new fixture file whose property docblock reads `@var Target`. Run it against the debug binary built before Task 1. Run: `npm --prefix editors/vscode test`. Expected: FAIL on the new test only.

- [ ] **Step 2: Rebuild and run it again.** Run: `cargo build -p phplumb-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 3: Run the reference benchmark.** Add a definition case on `\Drupal\Core\Messenger\MessengerInterface` in a custom `@var` docblock. Reference cases on classes may gain docblock mentions: check each new entry by hand and update their `expect`. Expected: `cases_failed` 0; resolved accesses at least 42,455; cold index under 10000; `peak_rss_mb` under 1024.

- [ ] **Step 4: Record and document.** Add a dated section to `docs/benchmarks.md`, extend the Go to definition line of `README.md` with class names in docblocks, and commit with message `docs: record docblock name results`.
