# PHP Navigator 2b: members, type inference and hover implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Go to definition for methods, properties, class constants, enum cases, global constants and `self`/`static`/`parent`, driven by type inference, plus hover with the resolved type, signature and docblock.

**Architecture:** `phpnav-index` learns class shapes at index time: each class-like declaration carries its parent, interfaces, traits and members with their declared and PHPDoc types, resolved against the file's namespace and imports. At query time, a new `infer` module walks the cursor file's syntax tree to find the expression under the cursor and infers the type of the object it is accessed on, from `$this`, `new`, parameters, properties, assignments, return types, `foreach`, `catch`, inline `@var` and `instanceof` checks. Member lookup walks the class hierarchy. Unknown types return nothing.

**Tech Stack:** existing workspace; mago CST walker and `comments::docblock::get_docblock_for_node`; a small PHPDoc tag and type parser written here (the mago docblock crates are not published for 1.59).

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** Second part of delivery step 2. Implementations and type hierarchy (2c), references and call hierarchy (2d), symbols, outline and Linux packaging (2e) have their own plans. Not in 2b: generics (`@template`), `@method` and `@property` class tags, magic `__get`/`__call`, Drupal container types.

## Global constraints

- Everything in the earlier plans' global constraints still holds: no wrong jumps, UTF-16 positions, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`, never kill processes by name.
- **Names:** class, function and method names are case-insensitive; property names and constant names are case-sensitive. A constant's namespace part is case-insensitive.
- **Uncertainty returns nothing.** A receiver whose type cannot be inferred yields no target. A union yields one target per member that resolves; members of the union that do not resolve are dropped. `mixed`, `object`, scalars and `null` resolve to nothing.
- **Member lookup order** for a class: its own members, then its traits (recursively), then its parent chain (each with its traits), then all its interfaces (recursively). The first declaration found wins per class in the union. Cycles in broken code must terminate.
- **PHPDoc wins over the declared type** when the PHPDoc type names at least one class or is an array of classes; otherwise the declared type is used.
- **Inference depth** is capped at 16 nested steps, so recursive assignments such as `$a = $a->next()` terminate.
- **Performance:** cold index on the reference project stays under 10 seconds and memory under 1 GB; definition and hover answer in under 100 ms on files up to 5,000 lines.

## Review focus

- **Chained calls through PHPDoc `@return static` or `$this`** (fluent APIs such as Drupal's `FieldDefinition::create()->setLabel()->setRequired()`) must keep the receiver's class. Test in Task 4.
- **A variable reassigned to a different type later in the function** must use the last assignment before the cursor, not the first or the last in the file. Test in Task 4.
- **A property declared in a trait or parent and typed only by PHPDoc** must still resolve. Test in Task 3 and Task 4.
- **A cursor on a member of an untyped receiver** (`$x->foo()` with `$x` from an unknown source) must return nothing even when a method `foo` exists in some class. Test in Task 4.
- **A class hierarchy with a cycle** (`class A extends B`, `class B extends A` in broken code) must not hang lookup. Test in Task 3.

---

### Task 1: PHPDoc tags and type expressions

**Files:**

- Create: `crates/phpnav-index/src/phpdoc.rs`
- Modify: `crates/phpnav-index/src/lib.rs`
- Test: unit tests in `phpdoc.rs`

**Interfaces:**

- Produces:

```rust
/// A type as navigation needs it. Names are fully qualified, without a leading backslash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Named(String),
    SelfType,          // self
    StaticType,        // static and $this
    ParentType,        // parent
    ArrayOf(Box<TypeExpr>), // X[], array<X>, array<K, X>, iterable<X>, list<X>
    Union(Vec<TypeExpr>),   // also used for intersections
    Other,             // scalars, mixed, null, shapes, callables, anything unknown
}

/// Resolves a class name written in a file to its fully qualified form.
pub trait NameContext { fn resolve_class(&self, name: &str) -> String; }

pub fn parse_type(text: &str, names: &dyn NameContext) -> TypeExpr;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocBlock {
    pub summary: String,                                 // text before the first tag, lines joined
    pub var: Vec<(Option<String>, TypeExpr)>,            // @var Type [$name]
    pub params: Vec<(String, TypeExpr)>,                 // @param Type $name (name without $)
    pub returns: Option<TypeExpr>,                       // @return Type
}

pub fn parse_docblock(text: &str, names: &dyn NameContext) -> DocBlock;
```

Rules: `?X` is `Named(X)`; `null` inside a union is dropped and a single remaining member is unwrapped; generic arguments on a named type are ignored (`Collection<Foo>` is `Named(Collection)`), except the array forms above; `$this` and `static` are `StaticType`; leading `\` marks a fully qualified name; keywords (`int`, `string`, `bool`, `float`, `array`, `mixed`, `void`, `null`, `false`, `true`, `object`, `callable`, `iterable`, `never`, `resource`, `class-string`, and anything with `{` or `(`) are `Other`. `@psalm-` and `@phpstan-` prefixed tags count as their plain form and win over the plain tag.

- [ ] **Step 1: Write the failing tests**

```rust
// A NameContext that maps "Foo" to "App\Foo" and "Alias" to "Lib\Thing", and leaves other names under "App\".
#[test] fn named_and_qualified()      // "Foo" -> Named("App\Foo"); "\Lib\Bar" -> Named("Lib\Bar"); "Alias" -> Named("Lib\Thing")
#[test] fn nullable_and_null_unions() // "?Foo" and "Foo|null" and "null|Foo" -> Named("App\Foo")
#[test] fn unions_and_intersections() // "Foo|\Lib\Bar" -> Union([Named App\Foo, Named Lib\Bar]); "Foo&Bar" -> Union(...)
#[test] fn arrays_of_classes()        // "Foo[]", "array<Foo>", "array<int, Foo>", "iterable<Foo>", "list<Foo>" -> ArrayOf(Named App\Foo)
#[test] fn self_static_this_parent()  // "self", "static", "$this", "parent"
#[test] fn scalars_and_shapes_are_other() // "int", "array", "array{a: int}", "callable(int): void", "mixed" -> Other
#[test] fn generics_keep_the_base()   // "Collection<Foo>" -> Named("App\Collection")
#[test] fn docblock_tags()
// "/**\n * Loads it.\n *\n * @param Foo $a\n * @param int $b\n * @return static\n * @var Foo|null $x\n */"
// summary "Loads it."; params [("a", Named App\Foo), ("b", Other)]; returns StaticType; var [(Some("x"), Named App\Foo)]
#[test] fn phpstan_tags_win()         // "@return array\n@phpstan-return Foo[]" -> returns ArrayOf(Named App\Foo)
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --lib phpdoc`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `phpdoc.rs`.** A hand-written tokenizer over the type text; the docblock parser strips `/**`, `*/` and leading `*`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p phpnav-index --lib phpdoc`
Expected: 9 passed.

- [ ] **Step 5: Commit** with message `feat(index): parse phpdoc tags and type expressions`.

### Task 2: Class shapes, function returns and global constants in the summary

**Files:**

- Modify: `crates/phpnav-index/src/summary.rs`
- Test: `crates/phpnav-index/tests/summary.rs`

**Interfaces:**

- Consumes: `TypeExpr`, `NameContext`, `parse_type`, `parse_docblock` (Task 1).
- Produces, extending `Declaration` (all new fields `#[serde(default)]`):

```rust
pub enum SymbolKind { Class, Interface, Trait, Enum, Function, Constant }

pub struct Declaration {
    pub kind: SymbolKind,
    pub fqn: String,
    pub name_start: u32,
    pub name_end: u32,
    pub signature: String,          // one line, e.g. "final class Foo extends Bar implements Baz", "function foo(int $a): ?Foo"
    pub doc: Option<String>,        // the raw docblock text
    pub returns: Option<TypeExpr>,  // functions: PHPDoc or declared return type per the global rule
    pub class: Option<ClassShape>,  // class-likes only
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClassShape {
    pub parent: Option<String>,      // fqn
    pub interfaces: Vec<String>,     // implements, or extends for interfaces
    pub traits: Vec<String>,
    pub members: Vec<Member>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberKind { Method, Property, Constant, EnumCase }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
    pub kind: MemberKind,
    pub name: String,               // properties without `$`
    pub is_static: bool,
    pub name_start: u32,
    pub name_end: u32,
    pub signature: String,          // e.g. "public function load(int $id): ?static", "protected ?Foo $bar"
    pub doc: Option<String>,
    pub ty: Option<TypeExpr>,       // property type, or method return type, per the global rule
    pub params: Vec<(String, Option<TypeExpr>)>, // methods: parameter names without `$` and types
}

/// The namespace and imports in effect at `offset` in a summarized file.
pub struct FileNames { /* per-namespace import maps */ }
impl FileNames { pub fn at(&self, offset: u32) -> impl NameContext + '_; }

pub struct FileSummary { pub declarations: Vec<Declaration>, pub names: Vec<NameRef>, pub has_parse_errors: bool, pub file_names: FileNames }
```

Constructor-promoted parameters become `Property` members. `const X = 1;` at namespace level and `define('X', ...)` with a literal string name become `Constant` declarations; `define` names are global.

- [ ] **Step 1: Write the failing tests** in `tests/summary.rs`:

```rust
#[test] fn class_shape_lists_parent_interfaces_and_traits()
// "namespace App; use Lib\Base; class Foo extends Base implements \Countable, Bar { use T1, \Lib\T2; }"
// class.parent == Some("Lib\Base"); interfaces == ["Countable", "App\Bar"]; traits == ["App\T1", "Lib\T2"]

#[test] fn members_with_types()
// class Foo { const A = 1; public static ?Bar $p; /** @var Baz[] */ protected array $list; public function __construct(private Qux $q) {}
//             /** @return static */ public function make(): self {} }
// members: Constant A; Property p (static, ty Named App\Bar); Property list (ty ArrayOf Named App\Baz);
// Property q (ty Named App\Qux); Method make (ty StaticType); Method __construct (params [("q", Some(Named App\Qux))])
// &source[name_start..name_end] of make == b"make"

#[test] fn enum_cases_and_interface_extends()
// "enum Suit: string { case Hearts = 'h'; } interface I extends J, K {}" -> Hearts is EnumCase; I.interfaces == ["App\J", "App\K"] (in namespace App)

#[test] fn function_returns_and_signatures()
// "/** Builds. @return Foo */ function build(int $a = 1): ?object {}" -> returns Named App\Foo; signature "function build(int $a = 1): ?object"; doc contains "Builds."

#[test] fn global_constants()
// "namespace App; const LIMIT = 10; define('GLOBAL_LIMIT', 5);" -> Constant "App\LIMIT", Constant "GLOBAL_LIMIT"
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test summary`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the shapes in the walker.** Signatures come from the source bytes of the declaration head, whitespace collapsed to single spaces, without bodies. Update `Index` for the new `SymbolKind::Constant`: constants get their own table keyed per the global naming rule.

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass, including fixtures and cache tests.

- [ ] **Step 5: Commit** with message `feat(index): record class shapes, returns and constants`.

### Task 3: Hierarchy, static members and keywords

**Files:**

- Create: `crates/phpnav-index/src/members.rs`
- Modify: `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/summary.rs` (record static access names)
- Test: `crates/phpnav-index/tests/fixtures/static_members/**`, `crates/phpnav-index/tests/fixtures.rs` (one more test), `crates/phpnav-index/tests/members.rs`

**Interfaces:**

- Produces in `index.rs`:

```rust
/// A class-like declaration found in the index.
pub struct ClassRef<'a> { pub path: &'a Path, pub declaration: &'a Declaration }

impl Index {
    pub fn classes_named(&self, fqn: &str) -> Vec<ClassRef<'_>>;
    /// Every declaration of `name` of `kind` visible on `class` per the lookup order,
    /// one per class declaration of `class`, deduplicated.
    pub fn find_member(&self, class: &str, kind: MemberKind, name: &str) -> Vec<(ClassRef<'_>, &Member)>;
}
```

`members.rs` holds the hierarchy walk. Definition gains: `X::method()`, `X::CONST`, `X::$prop`, `Enum::Case`, `self`, `static` and `parent` keywords (enclosing class, its parent), `self::`/`static::`/`parent::` members, `new self`/`new static`/`new parent`, and global constants with the namespace fallback for unqualified, non-imported names.

- [ ] **Step 1: Write the failing tests**

Fixture `static_members` with cursors for: static method, class constant, static property, enum case, `self::CONST` and `static::make()` inside the class, `parent::__construct()` reaching the parent's constructor, a constant inherited from an interface, a static method from a trait, the keywords `self`, `static` and `parent` themselves, a namespaced constant, a global constant through fallback, and `Unknown::foo()` returning nothing.

```rust
// tests/members.rs
#[test] fn find_member_follows_the_lookup_order()
// class C extends P uses trait T; P implements I. method m declared in T and P: find_member("C", Method, "m") -> the T declaration only.
// method only in I: returns the interface declaration.
#[test] fn find_member_survives_cycles()
// class A extends B {} class B extends A {}; find_member("A", Method, "x") returns empty, quickly.
#[test] fn property_typed_only_by_phpdoc_in_a_parent()
// class P { /** @var Foo */ protected $foo; } class C extends P {}; find_member("C", Property, "foo") -> P's member with ty Named Foo
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test members --test fixtures`
Expected: FAIL.

- [ ] **Step 3: Implement `members.rs`, the new lookups and the static definitions.**

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): navigate static members and class keywords`.

### Task 4: Type inference and instance members

**Files:**

- Create: `crates/phpnav-index/src/infer.rs`
- Modify: `crates/phpnav-index/src/index.rs`
- Test: `crates/phpnav-index/tests/fixtures/inference/**`, `crates/phpnav-index/tests/fixtures.rs`

**Interfaces:**

- Consumes: `find_member`, `classes_named` (Task 3), `TypeExpr` (Task 1), `FileNames` (Task 2).
- Produces:

```rust
/// What the cursor is on, with the inferred classes of the receiver when it is a member access.
pub(crate) enum Target { Member { classes: Vec<String>, kind: MemberKind, name: String }, Variable { name: String, ty: TypeExpr }, None }

pub(crate) fn target_at(index: &Index, source: &[u8], offset: u32) -> Target;

impl Index {
    /// The inferred type of the expression or variable at `offset`, for hover.
    pub fn type_at(&self, source: &[u8], offset: u32) -> Option<TypeExpr>;
}
```

`Index::definition` uses `target_at` for `->` and `?->` accesses. Inference sources, in order of precedence for a variable at the cursor: `instanceof` in the condition of an enclosing `if` (alone or in an `&&` chain); the nearest inline `/** @var T $v */` before the cursor in the same function; the last assignment to the variable before the cursor in the same function (including `list()`-free plain assignments only); `foreach` value variables (element type of `ArrayOf`); `catch` variables; parameters (declared type, or `@param`). Expressions: `$this`, `new X`, `clone`, parenthesized, `??` and ternaries (union), property fetch, method call, static call, function call (`returns`), nullsafe forms. `StaticType` and `SelfType` in a member's type resolve to the receiver class and the declaring class.

- [ ] **Step 1: Write the failing fixtures** in `tests/fixtures/inference/`, with cursors for:
  - `$this->method()` and `$this->prop`, including a property declared in a trait and one in a parent typed only by PHPDoc;
  - a typed parameter, a parameter typed by `@param`;
  - `$x = new Foo(); $x->bar();`, then `$x = new Baz(); $x->qux();` later in the same function (each cursor must reach its own class);
  - a typed property's method (`$this->repo->find()`), a method's return type (`$this->repo()->find()`), a function's return type;
  - a fluent chain through `@return static` and through a declared `self` return on a parent class, called on a subclass instance;
  - `$a?->b()`, `foreach ($items as $item)` with `@var Foo[] $items`, `catch (Bar $e)`, inline `/** @var Foo $x */`;
  - `if ($x instanceof Foo) { $x->bar(); }`;
  - a union `Foo|Bar` reaching both `m` declarations;
  - nothing for `$unknown->m()`, `$this->missing()`, a method on a scalar-typed parameter, and a dynamic `$x->$name()`.
- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test fixtures inference`
Expected: FAIL with the list of mismatching cursors.

- [ ] **Step 3: Implement `infer.rs` and wire it into `definition`.** One walk of the cursor file collects scopes, assignments, docblock variables and narrowing conditions as references into the arena; inference runs before the arena drops.

- [ ] **Step 4: Run the index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): infer receiver types for member navigation`.

### Task 5: Hover

**Files:**

- Create: `crates/phpnav-index/src/hover.rs`
- Modify: `crates/phpnav-index/src/lib.rs`, `crates/phpnav-server/src/lsp.rs`
- Test: `crates/phpnav-index/tests/hover.rs`, `crates/phpnav-server/tests/lsp_definition.rs`, `editors/vscode/src/test/definition.test.ts`

**Interfaces:**

- Produces:

```rust
pub struct Hover { pub start: u32, pub end: u32, pub markdown: String }
impl Index { pub fn hover(&mut self, path: &Path, source: &[u8], offset: u32) -> Option<Hover>; }
```

Markdown: a fenced `php` block with the signature, prefixed by the declaring class for members (`App\Foo::load(int $id): ?static`), then the docblock summary. On a variable: a fenced `php` block `Foo $x` with the fully qualified type. The server advertises `hoverProvider` and answers `textDocument/hover` with `MarkupContent` in markdown and the range.

- [ ] **Step 1: Write the failing tests**

```rust
// tests/hover.rs
#[test] fn hover_on_a_method_shows_class_signature_and_summary()
#[test] fn hover_on_a_class_shows_its_signature()
#[test] fn hover_on_a_variable_shows_its_inferred_type()
#[test] fn hover_on_whitespace_is_none()
// lsp_definition.rs
#[test] fn hover_over_stdio()  // markdown contains "class Target"
```

```ts
test("hover shows the class signature", ...) // vscode.executeHoverProvider on Use.php line 2, character 14; contents include "class Target"
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test hover && cargo test -p phpnav-server --test lsp_definition`
Expected: FAIL.

- [ ] **Step 3: Implement hover in the index and the server.**

- [ ] **Step 4: Run every suite**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo build -p phpnav-server && npm --prefix editors/vscode test`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat: show signatures, docblocks and types on hover`.

### Task 6: Coverage metric and reference cases

**Files:**

- Modify: `crates/phpnav-server/src/bench.rs`, `crates/phpnav-server/tests/bench.rs`, `docs/benchmarks.md`
- Modify outside the repository: `$HOME/.local/share/php-navigator/cases/reference.toml`

**Interfaces:**

- Produces: `bench --coverage <relative dir>` adds `"coverage": {"member_accesses": N, "resolved": M, "percent": P}` over every `->`, `?->` and `::` member access in PHP files under that directory. A member access counts as resolved when `definition` returns at least one target.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn bench_reports_member_coverage()
// bench --root <fixtures/inference> --coverage .; coverage.member_accesses > 0 and resolved <= member_accesses
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-server --test bench`
Expected: FAIL, unknown option `--coverage`.

- [ ] **Step 3: Implement the option.**

- [ ] **Step 4: Add at least 12 member cases to the reference case file**, each verified by reading the target: `$this->` calls, typed property chains, a static call, a class constant, an enum case, a `parent::` call, a fluent chain on a field definition, a `foreach` over a PHPDoc array, and at least 3 that expect nothing (untyped receivers).

- [ ] **Step 5: Run the reference benchmark**

Run: `cargo build --release -p phpnav-server && ./target/release/php-navigator bench --root <reference-project> --cases "$HOME/.local/share/php-navigator/cases/reference.toml" --cache "$TMPDIR/phpnav-bench-cache.bin" --coverage web/modules/custom`
Expected: `cases_failed` 0; `cold_index_ms` under 10000; `peak_rss_mb` under 1024; a coverage figure recorded as the baseline before the Drupal module.

If a target is missed, stop and report the numbers to the user.

- [ ] **Step 6: Add a dated section to `docs/benchmarks.md`** with the numbers and the coverage baseline.

- [ ] **Step 7: Commit** with message `feat(server): measure member coverage`.
