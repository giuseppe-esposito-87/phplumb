# PHPlumb 5c: element types implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The type of an element flows from a typed array or generic collection to the code that receives it: `foreach` values, array access, untyped callback parameters of built-in array functions, and members of generic collections such as `$items->first()`.

**Architecture:** PHPDoc types keep their generic arguments (`Collection<int, Item>`), and class summaries keep their `@template` names and the arguments they pass to generic parents (`@extends`, `@implements`). A new `generics` module walks those parents, substituting templates, to find what a type gives `Traversable` (its element) or `ArrayAccess` (its offset value), and what a generic receiver gives the templates of the class that declares a member. Inference uses it for `foreach`, `$a[$k]`, member types, and a fixed table of built-in array functions.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, goal "Core PHP 3", anti-goal "No wrong element types", delivery step 5c. The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds, including stubs for definition, hover and types only.
- **No wrong element types:** an element type comes only from a declared type: a native type, a PHPDoc type, a generic template argument, or a callback's declared return type. An array or collection with no declared element type gives nothing. A template with no argument gives nothing.
- **Generic counts as its class** wherever a class is read from a type, as `Tagged` does.
- **Built-in array functions,** only when the call reaches PHP's function (Task 4):
  - callbacks: `array_map` (parameter `i` gets the element of array argument `i + 1`); `array_filter` (parameter 0 gets the element; with a third argument, only `ARRAY_FILTER_USE_BOTH` keeps it); `array_reduce` (parameter 1); `usort` and `uasort` (parameters 0 and 1); `array_walk`, `array_find`, `array_find_key`, `array_any` and `array_all` (parameter 0);
  - returns, the element type of argument 0 kept as an array: `array_filter`, `array_values`, `array_reverse`, `array_slice`, `array_unique`, `array_diff`, `array_intersect`, `iterator_to_array`;
  - returns, the element of argument 0: `reset`, `end`, `current`, `array_pop`, `array_shift`, `array_first`, `array_last`;
  - `array_map` returns an array of its callback's return type hint, when the callback is a closure or arrow function written at that position.
- **Callbacks** are typed only when written as a closure or arrow function at a positional argument, and only on parameters with no type hint and no `@param`.
- **Diagnostics:** no new diagnostic on either reference project unless it is checked by hand as a real broken reference.
- **Targets:** Symfony reference coverage in `src` above 13,001 of 13,211; Drupal reference coverage at least the 5b build's on the same checkout; cold index under 10 seconds; `peak_rss_mb` under 1024; every case passes, apart from cases whose files are missing on the current checkout.
- **Branch** `feat/element-types` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **Generic parents that form a cycle,** such as two interfaces whose `@extends` name each other, end the walk with nothing. Test in Task 2.
- **A nullable typed array,** such as `@param list<Item>|null $items`, still gives `Item` to `foreach` and array access. Test in Task 2.
- **Fewer arguments than templates** bind from the first template, and the rest give nothing, as in `Pair<Item>` for `@template A @template B`. Test in Task 3.
- **`ARRAY_FILTER_USE_BOTH`** keeps the element on the first callback parameter, and `ARRAY_FILTER_USE_KEY` gives nothing. Test in Task 4.
- **Named arguments,** such as `array_map(array: $items, callback: fn ($i) => ...)`, give nothing. Test in Task 4.

---

### Task 1: Generic types and class templates in summaries

**Files:**

- Modify: `crates/phplumb-index/src/phpdoc.rs`, `crates/phplumb-index/src/summary.rs`, `crates/phplumb-index/src/hover.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`
- Create: `crates/phplumb-index/tests/elements.rs`, `crates/phplumb-index/tests/fixtures/elements/app/Hover.php`

**Interfaces:**

- Produces:
  - `TypeExpr::Generic { class: String, args: Vec<TypeExpr> }`: a class name written with `<...>`, its arguments in order, `Other` for scalars and `null`. `array`, `list`, `iterable` and the other array-like names stay `ArrayOf`.
  - `DocBlock::templates: Vec<String>`: names after `@template`, `@template-covariant` and `@template-contravariant`, resolved as class names in the docblock's context, so they equal the `Named` types the class's members carry.
  - `DocBlock::generic_supertypes: Vec<(String, Vec<TypeExpr>)>`: each `Generic` type of `@extends`, `@implements`, `@template-extends` and `@template-implements`, as its class and arguments.
  - Both tags take the `phpstan-` and `psalm-` prefixes; prefixed lists replace plain ones when not empty.
  - `ClassShape::templates` and `ClassShape::generic_supertypes`, same types, from the class-like's docblock.

- [ ] **Step 1: Write the failing tests** in `tests/elements.rs`:
  - `generic_doc_types_keep_their_arguments`: in `<?php namespace App; use Lib\Collection; class S { /** @var Collection<int, Item> */ public $c; }`, property `c` has type `Generic { class: "Lib\\Collection", args: [Other, Named("App\\Item")] }`.
  - `class_templates_and_generic_supertypes`: in `<?php namespace Lib; /** @template TKey @template-covariant T @template-extends \IteratorAggregate<TKey, T> */ interface C extends \IteratorAggregate {}`, the shape has templates `["Lib\\TKey", "Lib\\T"]` and generic supertypes `[("IteratorAggregate", [Named("Lib\\TKey"), Named("Lib\\T")])]`.
  - `hover_shows_generic_arguments`: in the fixture file `app/Hover.php`, a function holding `/** @var \Lib\Collection<int, \App\Item> $c */ $c = make();` and then `$c;`, hover on that last `$c` shows `\Lib\Collection<mixed, \App\Item> $c`.
  - Change the two `phpdoc.rs` unit tests that expect `Collection<Foo>` and `\Lib\Map<string, Foo>` to give `Named`: they now give `Generic`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test elements && cargo test -p phplumb-index --lib phpdoc`. Expected: FAIL, `Generic`, `templates` and `generic_supertypes` do not exist.

- [ ] **Step 3: Add `Generic`** to `TypeExpr` and to every place that reads a class from a type: `class_names`, `names_a_class`, `bind` (into the arguments), hover's `display` (`\Class<A, B>`), and each `TypeExpr::Named` match in `infer.rs` and `frameworks/drupal/mod.rs` that reads a receiver's class.

- [ ] **Step 4: Parse templates and generic supertypes** in `parse_docblock`, and copy them into `ClassShape` in `summary.rs`.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(index): keep generic arguments and class templates`.

### Task 2: Element and offset types

**Files:**

- Create: `crates/phplumb-index/src/generics.rs`, `crates/phplumb-index/tests/fixtures/elements/**`
- Modify: `crates/phplumb-index/src/lib.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/tests/elements.rs`

**Interfaces:**

- Consumes: `TypeExpr::Generic`, `ClassShape::templates`, `ClassShape::generic_supertypes` (Task 1).
- Produces, in `generics.rs`:
  - `pub(crate) fn supertype_args(index: &Index, class: &str, args: &[TypeExpr], ancestor: &str) -> Option<Vec<TypeExpr>>`: the arguments that `class<args>` gives `ancestor` (case-insensitive), walking `generic_supertypes` and substituting each class's templates. `Some(args)` when `class` is `ancestor`. `None` when the walk does not reach it.
  - `pub(crate) fn substitute(ty: &TypeExpr, templates: &[String], args: &[TypeExpr]) -> TypeExpr`: each `Named` template replaced by the argument at its position, `Other` when there is none, inside arrays, unions and generic arguments too.
  - `pub(crate) fn element_type(index: &Index, ty: &TypeExpr) -> TypeExpr`: the element of `ArrayOf`. For a `Generic` or `Named` class: with exactly one argument, on a class that reaches `Traversable` through `Index::supertypes`, that argument; otherwise argument 1 of what it gives `Traversable`. Unions map member by member. Anything else is `Other`.
  - `pub(crate) fn offset_type(index: &Index, ty: &TypeExpr) -> TypeExpr`: the same, with `ArrayAccess` in place of `Traversable` and no one-argument rule.

- [ ] **Step 1: Create the fixture** `crates/phplumb-index/tests/fixtures/elements`, synthetic, shaped like Doctrine Collections:
  - `lib/ReadableCollection.php`: `namespace Lib;` interface with `@template TKey`, `@template-covariant T`, `@template-extends \IteratorAggregate<TKey, T>`, and methods `first()` with `@return T|false`, `toArray()` with `@return array<TKey, T>`, `filter(\Closure $p)` with `@return ReadableCollection<TKey, T>`.
  - `lib/Collection.php`: interface `Collection extends ReadableCollection, \ArrayAccess` with `@template TKey`, `@template T`, `@template-extends ReadableCollection<TKey, T>`, `@template-extends \ArrayAccess<TKey, T>`.
  - `lib/ArrayCollection.php`: class with `@template TKey`, `@template T`, `@template-implements Collection<TKey, T>`.
  - `lib/Pair.php`: class with `@template A`, `@template B`, methods `first()` with `@return A` and `second()` with `@return B`.
  - `lib/Loop.php`: interfaces `LoopA` and `LoopB`, each with `@template T` and an `@extends` naming the other with `<T>`.
  - `app/Item.php`: `namespace App; class Item { public string $name = ''; public function label(): string { return ''; } }`, and `app/Other.php`: class `Other` with a `label()` method.
  - `app/Items.php`: `/** @extends \Lib\ArrayCollection<int, Item> */ final class Items extends \Lib\ArrayCollection {}`.
  - `app/Shelf.php`: class `Shelf` with properties `/** @var \Lib\Collection<int, Item> */ private Collection $items`, `/** @var array<string, Item> */ private array $byName`, `private array $loose`, `private \Lib\Collection $plain`, and methods using them.
  - `app/Uses.php`: free functions holding the cases below, each on its own line.

- [ ] **Step 2: Write the failing tests** in `tests/elements.rs`, each asserting `label` (or `name`) after the access gives exactly one location, `app/Item.php` with text `label`, unless it says nothing:
  - `foreach_over_a_generic_collection`: `foreach ($this->items as $item) { $item->label(); }`.
  - `foreach_over_a_concrete_collection_subclass`: a parameter `Items $items` in `foreach`.
  - `foreach_over_single_argument_iterables`: `@param \Iterator<Item> $it` in `foreach`, and `foreach (gen() as $g)` with `function gen(): \Generator` documented `@return \Generator<int, Item>`.
  - `foreach_over_a_nullable_typed_array`: `@param list<Item>|null $maybe` in `foreach` and in `$maybe[0]->label()`.
  - `array_access_on_typed_arrays`: `$this->byName['a']->label()` and `($this->byName[$k] ?? null)?->label()`.
  - `array_access_on_a_generic_collection`: `$this->items[0]->label()`.
  - `undeclared_elements_give_nothing`: `foreach ($this->loose as $x) { $x->label(); }`, `$this->loose[0]->label()` and `foreach ($this->plain as $p) { $p->label(); }` give no location.
  - `cyclic_generic_supertypes_end`: `@param \Lib\LoopA<Item> $loop` in `foreach` gives no location and returns.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test elements`. Expected: the Task 2 tests FAIL, apart from `undeclared_elements_give_nothing` and `cyclic_generic_supertypes_end`.

- [ ] **Step 4: Write `generics.rs`** with the four functions, a visited set and a depth limit of 16 classes on every walk.

- [ ] **Step 5: Use it in `infer.rs`:** `foreach` values take `element_type` in place of `element_of`, and `Expression::ArrayAccess` takes `offset_type` of its array.

- [ ] **Step 6: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 7: Commit** with message `feat(index): type foreach values and array access from declared elements`.

### Task 3: Members of generic receivers

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/tests/elements.rs`, `crates/phplumb-index/tests/fixtures/elements/**`

**Interfaces:**

- Consumes: `supertype_args`, `substitute` (Task 2).

- [ ] **Step 1: Write the failing tests**, adding the lines to `Shelf.php` or `Uses.php`:
  - `template_returns_take_the_receiver_arguments`: `$this->items->first()->label()`.
  - `template_arrays_take_the_receiver_arguments`: `foreach ($this->items->toArray() as $t) { $t->label(); }`.
  - `generic_returns_continue_the_chain`: `$this->items->filter(fn () => true)->first()->label()`.
  - `fewer_arguments_leave_templates_unknown`: with `@param \Lib\Pair<Item> $pair`, `$pair->first()->label()` gives `Item::label`, and `$pair->second()->label()` gives nothing.
  - `unbound_templates_give_nothing`: `$this->plain->first()->label()` gives nothing.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test elements`. Expected: the first four FAIL; `unbound_templates_give_nothing` passes.

- [ ] **Step 3: Substitute templates in member types.** In `member_type`, for each receiver class with its arguments (none for `Named`), when the member's declaring class has templates, replace them with `supertype_args(receiver class, receiver args, declaring class)` before `bind`; when that gives `None`, every template becomes `Other`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): resolve templates on generic receivers`.

### Task 4: Built-in array functions

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/elements.rs`, `crates/phplumb-index/tests/fixtures/elements/**`

**Interfaces:**

- Consumes: `element_type` (Task 2).
- Produces: `Index::builtin_function(&self, name: &NameRef) -> Option<String>`: the lowercase global name the call reaches when no project file declares the function, through PHP's global fallback for an unqualified, not imported name. Vendor polyfills give way, as in step 5a.

- [ ] **Step 1: Write the failing tests**, in `app/Uses.php` inside a function documented `@param list<Item> $items` and `@param array $loose`:
  - `array_map_callbacks_receive_elements`: `array_map(static fn ($item) => $item->label(), $items)`.
  - `array_map_feeds_each_array_to_its_parameter`: `array_map(fn ($n, $i) => $i->label(), $names, $items)` with `@param list<string> $names`.
  - `array_filter_modes`: `array_filter($items, fn ($i) => $i->label())` and the same with `ARRAY_FILTER_USE_BOTH` resolve; with `ARRAY_FILTER_USE_KEY`, nothing.
  - `sort_and_reduce_callbacks`: both parameters in `usort($items, fn ($a, $b) => $a->label() <=> $b->label())`; in `array_reduce($items, fn ($carry, $i) => $i->label(), '')`, `$i` resolves and `$carry->label()` gives nothing.
  - `typed_callback_parameters_keep_their_type`: `array_map(fn (Other $o) => $o->label(), $items)` gives `app/Other.php`.
  - `callbacks_on_undeclared_arrays_give_nothing`: `array_map(fn ($x) => $x->label(), $loose)`.
  - `named_arguments_give_nothing`: `array_map(array: $items, callback: fn ($n) => $n->label())`.
  - `project_functions_named_like_built_ins_give_nothing`: in `app/Shadow.php`, namespace `App\Shadow`, which declares its own `function array_map(callable $f, array $a): array`, the call `array_map(fn ($item) => $item->label(), $items)` in a function documented `@param list<\App\Item> $items` gives nothing.
  - `array_functions_keep_element_types`: `foreach (array_values(array_filter($items)) as $v) { $v->label(); }`, `reset($items)->label()`, and `array_map(fn ($x): Item => $x, $loose)[0]->label()`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test elements`. Expected: every new test FAILS, apart from `typed_callback_parameters_keep_their_type`, `callbacks_on_undeclared_arrays_give_nothing`, `named_arguments_give_nothing` and `project_functions_named_like_built_ins_give_nothing`.

- [ ] **Step 3: Add `Index::builtin_function`.**

- [ ] **Step 4: Type callback parameters.** Record each closure or arrow function written at a positional argument of a function call, with the call and the position. In `param_type`, a parameter with no hint and no `@param`, in a callback of a function in the global constraints' callback list, gets `element_type` of the array argument that list names.

- [ ] **Step 5: Type the returns** in the global constraints' return lists, in the `Call::Function` arm of `expr`, before `function_returns`.

- [ ] **Step 6: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 7: Commit** with message `feat(index): give array function callbacks their element types`.

### Task 5: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`, `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Build the 5b baseline.** Build `git merge-base main HEAD` in a scratch copy with `git archive` and `cargo build --release`.

- [ ] **Step 2: Add Symfony cases**, each checked by hand in `<symfony-reference-project>/src`:
  - a method on a `foreach` value over a `Collection<int, X>` property;
  - a property in an untyped `array_map` or `array_filter` callback over a `list<X>` method result;
  - a method after array access on an `array<K, X>`;
  - a method on a value from `->toArray()` or `->first()` of a collection;
  - one access on an undeclared element, expecting nothing (`expect = []`).

- [ ] **Step 3: Add a Drupal case** if custom code on the current checkout has a callback or `foreach` over a typed array that resolves only with this step; otherwise record that it has none.

- [ ] **Step 4: Run both benchmarks** with `--cases`, `--coverage` and, for Drupal, `--cache <file>`, for this branch and the baseline. Expected:
  - every case passes, apart from cases that already fail on the baseline because their files are missing;
  - Symfony resolved accesses above 13,001;
  - Drupal resolved accesses at least the baseline's;
  - cold index under 10000 and `peak_rss_mb` under 1024.

  If a target is missed, stop and report the numbers to the user.

- [ ] **Step 5: Run the diagnostics sweep** on Symfony `src` and Drupal `web/modules/custom`, for this branch and the baseline. Check every new diagnostic by hand. Expected: none, or only real broken references, each listed in the benchmark entry.

- [ ] **Step 6: Record the results and the backlog.**
  - Add a dated section `2026-10-28, step 5c element types` to `docs/benchmarks.md`, with one table per reference project.
  - List the Symfony accesses still unresolved, and add to `docs/backlog.md` each remaining cause this step leaves. Candidates seen before this step: array shapes and destructuring (`foreach ($rows as [$a, $b])`), `instanceof` inside `match (true)` arms, `static` returns on generic receivers dropping their arguments, callbacks of generic methods such as `Collection::filter`, and template bounds (`@template T of X`) unused.
  - Run the Markdown formatter on both files.

- [ ] **Step 7: Commit** with message `docs(benchmarks): record element type results`.
