# PHP Navigator 2d: references and call hierarchy implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Find every reference to a class, function, constant or member, including calls made through the interface or parent method a method implements, and answer the LSP call hierarchy (incoming and outgoing calls).

**Architecture:** At index time each file records hashed keys for the names it mentions (classes, functions, constants and member names), and the index keeps an inverted table from key to files. A query looks up the candidate files for the symbol's key, reads each one from disk and keeps only the occurrences that resolve to the symbol: names through the existing name resolution, member accesses through a batched form of the existing inference, run in parallel over the candidate files. A method's family (itself plus every declaration it overrides or implements up the hierarchy) decides which member accesses count. Call hierarchy builds on references and on resolving every call inside a function body.

**Tech Stack:** existing workspace; `rayon` for the candidate scan; LSP references and call hierarchy from `lsp-types` 0.97.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** Fourth part of delivery step 2. Symbol search, outline and Linux packaging (2e) have their own plan.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong targets, UTF-16 positions, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main` with no push, never kill processes by name.
- **Only resolved occurrences count.** A member access counts as a reference only when its receiver resolves and a resolved target is in the method's family. Name matching alone never makes a reference.
- **Family:** for a method `M` declared in class `C`, `M` itself plus every declaration of the same name in `C`'s ancestors (parents and interfaces, transitively). Calls that resolve to an ancestor declaration count as calls through it.
- **Ordering:** results from project code first, then contrib, then core, then `vendor`, decided by the first matching path component (`vendor`, `core`, `contrib`); then by path and offset.
- **Targets on the reference project:** references, incoming calls and outgoing calls answer in under 1 second, even for `load()` and `get()` on `EntityInterface`; cold index under 10 seconds; memory under 1 GB.
- **Hash keys** use `std::hash::DefaultHasher` over a prefixed, lowercased key (`c:` class, `f:` function, `k:` constant with its namespace lowercased only, `m:` member name lowercased). A collision only adds a candidate file that verification then drops.

## Review focus

- **A call on a receiver typed as an interface** (`$entity->save()` with `$entity` an `EntityInterface`) must be found among the references of `ContentEntityBase::save()`, and a call on an unrelated class's `save()` must not. Test in Task 2.
- **A file that changes or disappears** must change the candidate table, so references never list a call that is gone and do find a new one. Test in Task 1.
- **The same method name on unrelated classes** (`load()` on storages and on a custom class) must not leak between their reference lists. Test in Task 2.
- **A call in top-level code** (outside any function) must still appear among incoming calls, under a file-level caller. Test in Task 3.
- **A function called through PHP's global fallback** from a namespace (`helper()` reaching `\helper`) must be found among the function's references. Test in Task 2.

---

### Task 1: Reference keys, the candidate table and declaration ranges

**Files:**

- Modify: `crates/phpnav-index/src/summary.rs`, `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/cache.rs`
- Test: `crates/phpnav-index/tests/references.rs`

**Interfaces:**

- Produces:

```rust
/// A symbol whose references can be searched.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RefKey { Class(String), Function(String), Constant(String), Member(String) }
impl RefKey { pub(crate) fn hash(&self) -> u64; }

pub struct FileSummary { /* existing */ pub ref_keys: Vec<u64> }   // sorted, deduplicated
pub struct CachedFile { /* existing */ pub ref_keys: Vec<u64> }
pub struct Declaration { /* existing */ #[serde(default)] pub range: (u32, u32) }  // whole declaration
pub struct Member { /* existing */ #[serde(default)] pub range: (u32, u32) }       // whole member

impl Index {
    /// Files that mention `key`, possibly with false positives, never false negatives.
    pub fn candidate_files(&self, key: &RefKey) -> Vec<&Path>;
}
```

Every name reference contributes its kind's key; every member access (`->`, `?->`, `::`) contributes `Member` of its name; every declaration contributes its own key, so declaring files are candidates too.

- [ ] **Step 1: Write the failing tests** in `tests/references.rs`:

```rust
#[test] fn candidates_include_files_mentioning_the_symbol()
// A.php declares class A and function helper; B.php `new A(); helper(); $x->run();`; C.php mentions nothing.
// candidate_files(Class("A")) == {A.php, B.php}; Function("helper") == {A.php, B.php}; Member("run") == {B.php}
#[test] fn candidates_follow_updates()
// rewrite B.php without `run`, update: Member("run") no longer lists B.php; create D.php with `$y->run()`, update: lists D.php
#[test] fn declaration_ranges_cover_bodies()
// summarize "function f() { return 1; }" -> declaration range covers from `function` to the closing brace; a method's member range likewise
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test references`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the keys, the table (kept in `insert`/`remove`) and the ranges.**

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): record reference keys and declaration ranges`.

### Task 2: References

**Files:**

- Modify: `crates/phpnav-index/src/infer.rs` (batched member resolution), `crates/phpnav-index/src/index.rs`
- Create: `crates/phpnav-index/src/references.rs`
- Test: `crates/phpnav-index/tests/fixtures/references/**`, `crates/phpnav-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `candidate_files` (Task 1), `member_targets`, `find_member`, inference.
- Produces:

```rust
impl Index {
    /// References of what the cursor is on, ordered per the global rule. With
    /// `include_declaration`, the declarations themselves are listed too.
    pub fn references(&mut self, path: &Path, source: &[u8], offset: u32, include_declaration: bool) -> Vec<Location>;
}
// infer.rs
/// Every member access in a file whose name is `name` (any case for methods), resolved
/// with one parse: (start, end, receiver, kind).
pub(crate) fn member_accesses_named(index: &Index, summary: &FileSummary, source: &[u8], name: &str) -> Vec<(u32, u32, Receiver, MemberKind)>;
```

The fixture runner gets `run_reference_fixture(name)`, calling `references(..., false)`; a target marker marks a reference start.

- [ ] **Step 1: Write the failing fixture** `references` with cursors on:
  - a class declaration: `new`, type hints, `extends`, `instanceof`, `X::CONST` class parts and a `use` statement are references; a same-named class in another namespace is not;
  - a function declaration: a qualified call, an unqualified call through the global fallback from a namespace, and a call through `use function`;
  - a namespaced constant;
  - a method declared in an interface and implemented in a class: calls on the interface type and on the class type both reference the class method; a call on an unrelated class with the same method name does not; a call on an unknown receiver does not;
  - a method called through a parent (`parent::save()` and `$this->save()` in a subclass that does not override it);
  - a property (`$this->prop` reads and writes) and a static method (`X::make()`).
- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-index --test fixtures references`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `references`.** Candidate files are scanned in parallel with `rayon`; each file is read from disk, summarized once and its member accesses resolved once.

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): find references to symbols and members`.

### Task 3: Call hierarchy

**Files:**

- Create: `crates/phpnav-index/src/calls.rs`
- Modify: `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/lib.rs`
- Test: `crates/phpnav-index/tests/calls.rs`

**Interfaces:**

- Consumes: `references`, the declaration and member ranges (Task 1), definition resolution.
- Produces:

```rust
/// A function, method or file taking part in calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallItem { pub name: String, pub kind: CallKind, pub detail: String, pub path: PathBuf, pub range: (u32, u32), pub selection: (u32, u32) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind { Function, Method, File }

impl Index {
    /// The functions and methods the cursor resolves to.
    pub fn call_items(&mut self, path: &Path, source: &[u8], offset: u32) -> Vec<CallItem>;
    /// Callers of `item`, each with the call ranges inside it. Calls in top-level code
    /// belong to a `File` item.
    pub fn incoming_calls(&mut self, item: &CallItem) -> Vec<(CallItem, Vec<(u32, u32)>)>;
    /// Functions and methods called inside `item`'s body, each with the call ranges.
    pub fn outgoing_calls(&mut self, item: &CallItem) -> Vec<(CallItem, Vec<(u32, u32)>)>;
}
```

`detail` is the declaring class for methods, the namespace for functions, and the relative path for files.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn incoming_calls_group_by_caller()        // two calls from method A::a and one from function f: two callers, A::a with two ranges
#[test] fn incoming_calls_include_top_level_code() // a call outside any function: a File caller
#[test] fn outgoing_calls_list_resolved_callees()  // body calls $this->b(), helper(), new X() and $unknown->c(): callees A::b, helper and X::__construct (if declared); nothing for $unknown
#[test] fn call_items_on_a_call_site()             // cursor on `$this->b()` -> the item for A::b
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test calls`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `calls.rs`.**

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): build call hierarchies`.

### Task 4: Server and extension

**Files:**

- Modify: `crates/phpnav-server/src/lsp.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`
- Test: `crates/phpnav-server/tests/lsp_references.rs`

**Interfaces:**

- Produces: capabilities `referencesProvider` and `callHierarchyProvider`. `CallHierarchyItem.data` carries `{"path", "range", "selection", "kind", "name", "detail"}` so incoming and outgoing requests rebuild the `CallItem`. The extension skips references and call hierarchy preparation on documents with unsaved edits.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn references_over_stdio()       // Target.php class; Use.php `new Target()` twice: two locations; includeDeclaration adds the declaration
#[test] fn call_hierarchy_over_stdio()   // prepare on a method, incoming lists its caller, outgoing lists its callee
```

```ts
test("find references lists the uses of Target", ...) // vscode.executeReferenceProvider on Target.php
```

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phpnav-server --test lsp_references`. Expected: FAIL.

- [ ] **Step 3: Implement the handlers and the middleware.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phpnav-server && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: serve references and call hierarchy`.

### Task 5: Reference cases and benchmarks

**Files:**

- Modify: `crates/phpnav-server/src/bench.rs`, `crates/phpnav-server/tests/bench.rs`, `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/php-navigator/cases/reference.toml`

**Interfaces:**

- Produces: case `kind = "references"` (declarations excluded), compared exactly; the report adds `references_ms`, the slowest references case.

- [ ] **Step 1: Write the failing test** `bench_runs_reference_cases` on the `references` fixture: one passing case and one failing case; exit 1, `cases_passed` 1, `references_ms` a number.

- [ ] **Step 2: Run it and watch it fail.** Expected: `unsupported case kind references`.

- [ ] **Step 3: Implement the case kind.**

- [ ] **Step 4: Add at least 5 references cases** to the reference case file, each checked against an independent text search: a custom service method with a few callers, a custom class, a custom function or constant, and one high-fan-out method (`load` on `EntityStorageInterface` or `get` on `ContentEntityBase`) used as the timing case, with its expected list taken from the engine after spot-checking 20 entries by hand.

- [ ] **Step 5: Run the reference benchmark**

Run: `cargo build --release -p phpnav-server && ./target/release/php-navigator bench --root <reference-project> --cases "$HOME/.local/share/php-navigator/cases/reference.toml" --coverage web/modules/custom`
Expected: `cases_failed` 0; `references_ms` under 1000; cold index under 10000; `peak_rss_mb` under 1024.

If a target is missed, stop and report the numbers to the user.

- [ ] **Step 6: Add a dated section to `docs/benchmarks.md`** and commit with message `feat(server): benchmark reference cases`.
