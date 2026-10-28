# PHPlumb 6f: Doctrine mapping implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In `*.orm.xml` mapping files, Cmd+click on an entity, embeddable, target, repository or enum class opens the class, and on a field, association or id name opens the property; `getRepository(X::class)` returns the repository class the mapping declares for `X`.

**Architecture:** The Symfony module reads Doctrine mappings: the `*.orm.xml` files of the Symfony project and `#[ORM\Entity(repositoryClass:)]` attributes. XML files become framework files like YAML and Twig, the extension sends `*.orm.xml` to the engine, and the server watches them. A small XML scanner finds the attribute value under the cursor and its element.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goal 6 "Doctrine mapping", delivery step 6f; constraint "The extension sends Twig and Doctrine XML files". The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Mapping files:** every `*.orm.xml` of the Symfony project outside `vendor`.
- **XML navigation** (spec): "In `*.orm.xml` mapping files, Cmd+click on an entity, embeddable or target class opens the class, and on a field, association or id name opens the property."
  - classes: `name` of `entity`, `embeddable` and `mapped-superclass`; `target-entity` of the four association elements; `repository-class`; `class` of `embedded`; `enum-type` of `field` and `id`. A name without a namespace resolves against the namespace of the mapped class, as Doctrine does.
  - properties: `name` of `field`, `id` and `embedded`, and `field` of the association elements, on the mapped class; `mapped-by` and `inversed-by` on the target class. A property is looked up on the class and its parents.
- **Repositories** (spec): "`getRepository(X::class)` returns the repository class that the mapping, either XML `repository-class` or the `#[ORM\Entity(repositoryClass:)]` attribute, declares for `X`." `getRepository` is the one declared in `Doctrine\Persistence\ObjectManager` (and so `EntityManagerInterface`) or `Doctrine\Persistence\ManagerRegistry`. Without a declared repository, the declared return type stays.
- **Branch** `feat/symfony-doctrine` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **A `field` element inside an `embeddable`** opens the embeddable's property, not an entity's. Test in Task 2.
- **A `mapped-by` value** opens the property on the target entity, not on the mapped one. Test in Task 2.
- **An XML comment or the `doctrine-mapping` namespace URLs** give nothing. Test in Task 2.
- **An `*.orm.xml` in a nested Drupal project, or any other XML file,** gets no answer it should not. Test in Task 2.
- **`getRepository($class)` with a variable** keeps the declared return type. Test in Task 3.

---

### Task 1: Read Doctrine mappings

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/doctrine.rs`
- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Produces: `Symfony::repository_of(&self, entity: &str) -> Option<&str>`, the repository class the mappings declare for `entity`; `doctrine::Element { name: String, attributes: Vec<(String, String, u32, u32)> }` and `doctrine::elements(text: &str) -> Vec<Element>`, the XML elements of a mapping file in order, each attribute with its value and the value's range.

- [ ] **Step 1: Extend the fixture:** the `Doctrine\ORM\Mapping\Entity` attribute stub; entities `src/Entity/Post.php` (properties `id`, `title`, `author`, `address`), `src/Entity/Author.php` (properties `id`, `posts`), `src/Entity/Address.php` (embeddable, property `street`), `src/Entity/Comment.php` with `#[ORM\Entity(repositoryClass: CommentRepository::class)]`; repositories `src/Repository/PostRepository.php` and `src/Repository/CommentRepository.php`; mappings `config/doctrine/Post.orm.xml` (`entity name="App\Entity\Post" repository-class="App\Repository\PostRepository"`, `id name="id"`, `field name="title"`, `many-to-one field="author" target-entity="Author" inversed-by="posts"`, `embedded name="address" class="App\Entity\Address"`), `config/doctrine/Author.orm.xml` (`one-to-many field="posts" target-entity="App\Entity\Post" mapped-by="author"`, a comment naming `App\Entity\Post`) and `config/doctrine/Address.orm.xml` (`embeddable name="App\Entity\Address"`, `field name="street"`).

- [ ] **Step 2: Write the failing tests:**
  - `repositories_come_from_xml_and_attributes`: `repository_of("App\Entity\Post")` is `App\Repository\PostRepository`; of `App\Entity\Comment`, `App\Repository\CommentRepository`; of `App\Entity\Author`, none.
  - `elements` unit tests in `doctrine.rs`: attributes with both quote styles, self-closing and nested elements, comments and the `<?xml ?>` declaration skipped, and each value's range.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony repositories && cargo test -p phplumb-index --lib doctrine`. Expected: FAIL to compile.

- [ ] **Step 4: Implement** `doctrine.rs` and read the mappings with the routes in the module.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(symfony): read doctrine mappings`.

### Task 2: Navigation in mapping files

**Files:**

- Modify: `crates/phplumb-index/src/filter.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-server/src/lsp.rs`, `crates/phplumb-server/tests/lsp_drupal.rs`, `editors/vscode/src/extension.ts`, `editors/vscode/src/test/manifest.test.ts`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/projects.rs`, fixtures

**Interfaces:**

- Consumes: `doctrine::elements` (Task 1).
- Produces: `filter::is_doctrine_xml(path)`, part of `is_framework_file`; the server watches `**/*.orm.xml`; `DOCUMENT_SELECTOR` holds `{ scheme: "file", pattern: "**/*.orm.xml" }`.

- [ ] **Step 1: Write the failing tests** in `tests/symfony.rs`, by line and needle:
  - `mapping_class_names_open_their_class`: in `Post.orm.xml`, `App\Entity\Post`, `App\Repository\PostRepository`, the short `Author` of `target-entity`, and `App\Entity\Address` of `embedded` open their classes.
  - `mapping_property_names_open_their_property`: `id`, `title`, `author` (association `field`), `address` (embedded `name`) open `Post`'s properties; `posts` in `inversed-by` opens `Author::$posts`; in `Author.orm.xml`, `author` in `mapped-by` opens `Post::$author`; in `Address.orm.xml`, `street` opens `Address::$street`.
  - `mapping_comments_and_namespaces_give_nothing`: the class in the comment and the `doctrine-mapping` namespace URL give nothing.
  - In `tests/projects.rs`, `doctrine_xml_stays_in_its_project`: an `*.orm.xml` placed in the nested Drupal project gets no target.
  - In `lsp_drupal.rs`, the watcher test also expects `**/*.orm.xml`; in `manifest.test.ts`, `sends Doctrine mappings to the engine`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test --workspace && npm --prefix editors/vscode test`. Expected: the new tests that expect a target FAIL, and the watcher and manifest tests FAIL.

- [ ] **Step 3: Implement** the XML answers in `Symfony::file_targets`, the filter, the watcher and the selector; the Drupal module answers YAML only, as before.

- [ ] **Step 4: Run the checks.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings && npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate doctrine xml mappings`.

### Task 3: Repository types

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, fixtures

**Interfaces:**

- Consumes: `Symfony::repository_of` (Task 1).

- [ ] **Step 1: Write the failing tests.** Add the `Doctrine\Persistence\ObjectManager` stub (`getRepository(string $className): ObjectRepository`) and `src/Service/Posts.php` with a promoted `ObjectManager $em` and, on their own lines, `$this->em->getRepository(Post::class)->findRecent();`, `$this->em->getRepository(Comment::class)->findThread();`, `$this->em->getRepository(Author::class)->find(1);` and `$this->em->getRepository($class)->find(1);`:
  - `get_repository_returns_the_mapped_repository`: definition on `findRecent` gives `PostRepository::findRecent`; on `findThread`, `CommentRepository::findThread`.
  - `get_repository_without_a_mapping_keeps_its_type`: definition on `find` of the last two lines gives `ObjectRepository::find`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony get_repository`. Expected: the first FAILS.

- [ ] **Step 3: Implement `Symfony::call_type`** for `getRepository` with a `X::class` argument.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): type getRepository from doctrine mappings`.

### Task 4: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Build the 6e baseline** from `git merge-base main HEAD`.

- [ ] **Step 2: Probe** every class and property name in the Symfony reference project's `*.orm.xml` files, count those that resolve, and check those that do not by hand. Add cases: an entity name, a field name, a `target-entity`, and an `enum-type`.

- [ ] **Step 3: Run both benchmarks and both diagnostics sweeps,** for this branch and the baseline, base first. Expected: the Drupal reference project unchanged; Symfony coverage, cases and diagnostics at least the baseline's; cold index under 10000 and `peak_rss_mb` under 1024. If a target is missed, stop and report to the user.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6f Doctrine mapping` to `docs/benchmarks.md`, a README bullet under Symfony, and to `docs/backlog.md` anything this step defers, such as annotation and attribute mappings beyond `repositoryClass`, YAML mappings, and `ManagerRegistry::getManager()` chains. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record doctrine mapping results`.
