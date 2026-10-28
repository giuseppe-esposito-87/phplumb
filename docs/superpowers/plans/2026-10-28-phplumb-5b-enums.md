# PHPlumb 5b: enums implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the implicit members of enums (`X::from()`, `X::tryFrom()`, `X::cases()`, `->value`, `->name`) and type an enum case access as its enum, so `Role::PlatformAdmin->label()` and `Severity::from($x)->label()` resolve.

**Architecture:** Every enum's summary lists the interface PHP gives it implicitly: `UnitEnum`, and `BackedEnum` too for a backed enum. Member lookup then reaches the stub declarations of step 5a through the ordinary interface walk. Inference types `X::NAME` as `X` when `NAME` is an enum case of `X`, and `static` returns of `from()` and `tryFrom()` resolve to the enum they are called on. Diagnostics treat `UnitEnum` and `BackedEnum` as fully known, since PHP fixes their members.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, goal "Core PHP 2", delivery step 5b. The earlier spec `2026-10-26-php-navigator-design.md` and the 5a plan's constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds, including stubs for definition, hover and types only.
- **Implicit members resolve to their stub declaration:** `from`, `tryFrom` and `$value` in `BackedEnum`, `cases` and `$name` in `UnitEnum`, all in `Core/Core_c.php`.
- **A pure enum has no `from`, `tryFrom` or `value`:** they resolve to nothing on an enum without a backing type.
- **Only an enum case has the enum's type.** A class constant of an enum or class gets no type from this step.
- **Diagnostics:** no new diagnostic on either reference project unless it is checked by hand as a real broken reference. The diagnostics fixture keeps every existing marker.
- **Targets:** Symfony reference coverage in `src` above 12,801 of 13,211; Drupal reference coverage at least the 5a build's on the same checkout; cold index under 10 seconds; `peak_rss_mb` under 1024; every case passes.
- **Branch** `feat/enums` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **`from()` on a pure enum,** such as `Plain::from('x')`, gives no target. Test in Task 1.
- **A missing case or method on an enum is still flagged,** such as `Suit::Spades` and `Plain::nowhereDeclared()`, now that every enum reaches a stub interface. Test in Task 1.
- **A class constant of an enum that is not a case,** such as `Severity::LIMIT`, gives nothing after `->`. Test in Task 2.
- **A case reached through `self::` or `static::` inside the enum,** such as `self::Low->label()`, resolves to the enum's method. Test in Task 2.
- **A null-safe chain on a nullable result,** such as `Severity::tryFrom($x)?->label()`, resolves. Test in Task 2.

---

### Task 1: Implicit enum interfaces

**Files:**

- Modify: `crates/phplumb-index/src/summary.rs` (`walk_in_enum` and the shape it builds), `crates/phplumb-index/src/diagnostics.rs` (`fully_known`)
- Create: `crates/phplumb-index/tests/enums.rs`, `crates/phplumb-index/tests/fixtures/enums/**`
- Modify: `crates/phplumb-index/tests/fixtures/diagnostics/app/Members.php`

**Interfaces:**

- Produces: an enum declaration's `ClassShape::interfaces` holds its declared interfaces, then `UnitEnum`, then `BackedEnum` when the enum has a backing type.

- [ ] **Step 1: Create the fixture** `crates/phplumb-index/tests/fixtures/enums`:
  - `app/Severity.php`: `namespace App; enum Severity: string { case Low = 'low'; case High = 'high'; const LIMIT = self::High; public function label(): string { return ucfirst($this->value); } public function lowest(): string { return self::Low->label(); } }`
  - `app/Plain.php`: `namespace App; enum Plain { case One; }`
  - `app/Uses.php` in namespace `App`, with a function taking `Severity $severity` and `Plain $plain` that holds, each on its own line:
    - `Severity::from('low');`
    - `Severity::tryFrom('low');`
    - `Severity::cases();`
    - `$severity->value;`
    - `$plain->name;`
    - `Plain::from('x');`

- [ ] **Step 2: Write the failing tests** in `tests/enums.rs`, with the cursor helpers of `tests/stubs.rs` (copy them; the two files stay independent):
  - `enum_interfaces_include_the_implicit_ones`: `summarize` of `<?php enum S: string implements Foo {}` gives interfaces `["Foo", "UnitEnum", "BackedEnum"]`; of `<?php enum P {}` gives `["UnitEnum"]`.
  - `backed_enum_members_open_their_stub`: `from`, `tryFrom` and `value` in `Uses.php` each give one location in `Core/Core_c.php`, with text `from`, `tryFrom` and `value`.
  - `unit_enum_members_open_their_stub`: `cases` and `name` each give one location in `Core/Core_c.php`, with text `cases` and `name`.
  - `pure_enums_have_no_backed_members`: `from` in `Plain::from('x')` gives no location.
  - In `Members.php` of the diagnostics fixture, add `Plain::/*@broken*/nowhereDeclared();` after `Plain::cases();`. The `diagnostics` fixture test must keep `Suit::/*@broken*/Spades` flagged.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test enums --test fixtures`. Expected: the new `enums` tests FAIL; `diagnostics` passes, because enums are fully known today.

- [ ] **Step 4: Add the implicit interfaces** in `summary.rs`: an enum with a backing type gets `BackedEnum` after `UnitEnum`.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace`. Expected: the `diagnostics` fixture test FAILS on `Suit::Spades` and `Plain::nowhereDeclared`, because both hierarchies now reach a stub.

- [ ] **Step 6: Count `UnitEnum` and `BackedEnum` as fully known** in `diagnostics::fully_known`, by those two names, even though their declarations are stubs. Every other stub class stays uncertain.

- [ ] **Step 7: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 8: Commit** with message `feat(index): give enums their implicit interfaces`.

### Task 2: Enum case types

**Files:**

- Modify: `crates/phplumb-index/src/infer.rs`
- Test: `crates/phplumb-index/tests/enums.rs`, `crates/phplumb-index/tests/fixtures/enums/**`, `crates/phplumb-index/tests/fixtures/diagnostics/app/Members.php`

**Interfaces:**

- Consumes: the implicit interfaces (Task 1).

- [ ] **Step 1: Write the failing tests.** Add to `app/Uses.php`, each on its own line:
  - `Severity::Low->label();`
  - `Severity::from('low')->label();`
  - `Severity::tryFrom('low')?->label();`
  - `Severity::LIMIT->label();`

  Add the tests:
  - `enum_cases_have_their_enum_type`: `label` after `Severity::Low->` gives one location, `app/Severity.php` with text `label`.
  - `from_returns_the_enum`: `label` after `from('low')->` and after `tryFrom('low')?->` each give that location.
  - `self_cases_resolve_inside_the_enum`: `label` in `self::Low->label()` in `Severity.php` gives that location.
  - `class_constants_get_no_enum_type`: `label` after `Severity::LIMIT->` gives no location.
  - In `Members.php` of the diagnostics fixture, add `Suit::Hearts->/*@broken*/invented();`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test enums --test fixtures`. Expected: the four new `enums` tests except `class_constants_get_no_enum_type` FAIL, and `diagnostics` FAILS on `invented`.

- [ ] **Step 3: Type enum case accesses** in `infer.rs`: `X::NAME` is `Named(X's declaring enum)` when the `Constant` lookup of `NAME` on `X` finds a member of kind `EnumCase`; anything else stays `Other`. `self`, `static` and `parent` resolve as for other static accesses. When the static return of `from()` or `tryFrom()` does not already resolve to the enum the call is on, resolve it there.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): type enum cases as their enum`.

### Task 3: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`, `$HOME/.local/share/phplumb/cases/reference.toml`

- [ ] **Step 1: Build the 5a baseline.** Build `main` before this branch (`git merge-base main HEAD`) in a scratch copy with `git archive` and `cargo build --release`, as step 5a did, so both reference projects can be compared on their current checkouts.

- [ ] **Step 2: Add Symfony cases**, each checked by hand in `<symfony-reference-project>/src`:
  - an enum `X::from(...)`;
  - a `->value` on an enum-typed property or parameter;
  - a method called on an enum case (`X::Case->method()`);
  - a method called after `X::from(...)`.

- [ ] **Step 3: Add a Drupal case** if custom code on the current checkout uses an enum; otherwise record that it has none.

- [ ] **Step 4: Run both benchmarks** with `--cases` and `--coverage`, for this branch and for the baseline. Expected:
  - every case passes, apart from cases that already fail on the baseline because their files are missing on the current checkout;
  - Symfony resolved accesses above 12,801;
  - Drupal resolved accesses at least the baseline's;
  - cold index under 10000 and `peak_rss_mb` under 1024.

  If a target is missed, stop and report the numbers to the user.

- [ ] **Step 5: Run the diagnostics sweep** on Symfony `src` and Drupal `web/modules/custom`, for this branch and the baseline. Check every new diagnostic by hand. Expected: none, or only real broken references, each listed in the benchmark entry.

- [ ] **Step 6: Record the results and the backlog.**
  - Add a dated section `2026-10-28, step 5b enums` to `docs/benchmarks.md`, with one table per reference project.
  - Update `docs/backlog.md` with anything this step defers.
  - Run the Markdown formatter on both files.

- [ ] **Step 7: Commit** with message `docs(benchmarks): record enum results`.
