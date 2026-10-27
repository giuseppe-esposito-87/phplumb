# PHPlumb 4b.2: property types from assignments implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give a property the types its class assigns to it, so `$this->nodeStorage->load()` resolves when the constructor or `create()` stores `getStorage('node')` there, and untyped properties assigned `new X()` get a type.

**Architecture:** Inference collects property assignments (`$this->p = ...`, and `$x->p = ...` where `$x` is the class itself, as in `create()`). When it types a property fetch, it adds to the declared type the types of the assignments made in the receiver's class and its parents. Those are computed lazily per class, from the class's file, cached in the index, and the cache empties on every index update. A per-thread guard stops a class's assignments from depending on themselves.

**Tech Stack:** existing workspace.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md` (anti-goal: a wrong target is worse than no target; unions give one target per member).

**Scope:** Follow-up to step 4b, before 4c. Assignments in traits, in subclasses below the receiver, and from outside the class are not read.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Union, never replacement:** a property's type is its declared type plus its assigned types. Without a declaration, the assigned types alone.
- **Determinism:** results must not depend on thread scheduling; a class being computed is never visible to other threads as empty.
- **Targets on the reference project:** member coverage in `web/modules/custom` rises above the 91.9% baseline; cold index under 10 seconds; memory under 1 GB; the slowest references case stays under 1 second.

## Review focus

- **Storage kept by `create()`:** `$instance = new static(...)` or `parent::create(...)`, then `$instance->storage = ...->getStorage('id')`, types `$this->storage`. Test in Task 1.
- **A subclass reading a parent's property** gets the parent's assigned types. Test in Task 1.
- **A property assigned from itself or from another property of the same class** terminates. Test in Task 1.
- **An edited assignment** changes the property's type after an index update. Test in Task 1.
- **Parallel reference scans** give the same results on every run. Test in Task 1.

---

### Task 1: Property assignment types

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`
- Test: `crates/phplumb-index/tests/fixtures/drupal/**`, `crates/phplumb-index/tests/members.rs`

**Interfaces:**

- Produces: `Index::assigned_types(&self, class: &str, property: &str) -> Vec<TypeExpr>` (pub(crate)): the types assigned to `property` in `class` and its parents.

- [ ] **Step 1: Write the failing tests**: fixture cases on a class keeping a typed storage from its constructor and a docblock-typed one from `create()`, an untyped property assigned `new X()`, a subclass method, and a property never assigned (declared type only); a `members.rs` test that edits an assignment and updates the index; a test that a property assigned from another property of the same class terminates; and a test that references of a method reached through such properties are identical over 5 runs.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test fixtures drupal --test members`. Expected: FAIL.

- [ ] **Step 3: Implement the facts, the cache and the union.**

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): type properties from their assignments`.

### Task 2: Reference cases and benchmarks

**Files:**

- Modify: `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Add at least 3 cases**, each checked by hand: a method after `$this->...Storage->load()` with the storage kept by the constructor, one kept by `create()`, and an untyped property assigned an object.

- [ ] **Step 2: Run the reference benchmark.** Expected: `cases_failed` 0 (earlier cases updated only after checking new targets by hand); coverage above 91.9%; cold index under 10000; `peak_rss_mb` under 1024; slowest references case under 1000 ms. If a target is missed, stop and report the numbers to the user.

- [ ] **Step 3: Add a dated section to `docs/benchmarks.md`** and commit with message `docs(benchmarks): record property assignment results`.
