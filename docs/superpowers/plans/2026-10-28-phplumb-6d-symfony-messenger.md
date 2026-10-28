# PHPlumb 6d: Symfony Messenger implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cmd+click on `dispatch` in a `dispatch()` call on a message bus lists every handler of the message, and Cmd+click on a handler lists every place that dispatches its message.

**Architecture:** The Symfony module reads message handlers from `#[AsMessageHandler]` attributes of project classes. The index gains two framework hooks: the types of each call's positional arguments in `FoundCall`, and `Framework::call_targets`, asked on a definition request on a method call's name with the types of its arguments. Handlers answer `call_targets` for `MessageBusInterface::dispatch()`, and a handler declaration answers `declaration_targets` with the dispatch calls the index finds.

**Tech Stack:** existing workspace; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-28-phplumb-symfony-design.md`, Symfony goal 3 "Messenger", delivery step 6d. The earlier plans' constraints still apply.

**Backlog:** this step picks up no item from `docs/backlog.md`. Anything it defers goes there, per `CLAUDE.md`.

## Global constraints

- Everything in the earlier plans' global constraints still holds.
- **Handlers** (spec): "Handlers are classes or methods with `#[AsMessageHandler]`; the handled class comes from the attribute's `handles` argument or from the type of the handler's first parameter. A handler of a parent class or interface of the message also handles it, as in Symfony."
  - A class-level attribute names its handler method with `method:`, `__invoke` by default; a method-level attribute marks that method.
  - Each class of a union first-parameter type is handled; a parameter with no class type handles nothing.
  - Handlers come from concrete classes in project code, outside `vendor`, inside the Symfony project.
- **Dispatches:** a call whose target is `dispatch()` declared in `Symfony\Component\Messenger\MessageBusInterface` (or a class that is one), with a positional argument 0 whose type names a class. A message wrapped in `new Envelope(...)` or of unknown type has no handlers here.
- **Navigation:**
  - definition on the `dispatch` name of such a call gives the handler methods of every handler whose handled class the message is, extends or implements; with none, the usual declaration;
  - definition on a handler method's name or on its `#[AsMessageHandler]` attribute gives every such dispatch call, at its `dispatch` name, whose message is, extends or implements the handled class.
- **Branch** `feat/symfony-messenger` from `main`; conventional commits with the `Assisted-by: claude-code/claude-opus-5-5` trailer and no issue reference.

## Review focus

- **A handler of an interface the message implements** is listed for the message, and its dispatches include those of every implementing message. Tests in Tasks 1 and 3.
- **A message with no handler** keeps the usual definition of `dispatch`. Test in Task 2.
- **`dispatch($message)` with an untyped variable** gives the usual definition. Test in Task 2.
- **A class-level `#[AsMessageHandler(method: 'handle')]`** uses `handle`, not `__invoke`. Test in Task 1.
- **A Drupal project** never asks the Symfony module about its calls. Covered by module gating; checked in Task 4.

---

### Task 1: Read message handlers

**Files:**

- Create: `crates/phplumb-index/src/frameworks/symfony/messenger.rs`
- Modify: `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Produces: `Symfony::handlers_of(&self, index: &Index, message: &str) -> Vec<Location>`: the handler methods whose handled class `message` is, extends or implements, each at its method name; and `Symfony::handled_by(&self, path: &Path, method_start: u32) -> Vec<String>`: the classes the handler method at that name offset handles.

- [ ] **Step 1: Extend the fixture:** the `AsMessageHandler` attribute and `MessageBusInterface` (`dispatch(object $message, array $stamps = []): Envelope`) and `Envelope` stubs under `vendor/symfony/messenger` with an `installed.json` entry; messages `src/Message/Notify.php` (interface), `src/Message/SendEmail.php` (implements `Notify`), `src/Message/SendSms.php` (implements `Notify`), `src/Message/Unhandled.php`; handlers `src/Handler/SendEmailHandler.php` (class attribute, `__invoke(SendEmail $m)`), `src/Handler/NotifyLogger.php` (class attribute with `method: 'handle'`, `handle(Notify $n)`), `src/Handler/SmsHandlers.php` (method attribute on `onSms(SendSms|SendEmail $m)`), `src/Handler/Explicit.php` (method attribute `#[AsMessageHandler(handles: SendSms::class)]` on `run(object $m)`), and an abstract handler `src/Handler/BaseHandler.php` with a class attribute and `__invoke(Unhandled $m)`.

- [ ] **Step 2: Write the failing tests** in `tests/symfony.rs`, each location as (path relative to the fixture, text at its range):
  - `handlers_of_a_message`: `App\Message\SendEmail` gives `SendEmailHandler::__invoke`, `NotifyLogger::handle` and `SmsHandlers::onSms`; `App\Message\SendSms` gives `NotifyLogger::handle`, `SmsHandlers::onSms` and `Explicit::run`.
  - `messages_without_handlers_give_nothing`: `App\Message\Unhandled` gives nothing, the abstract handler not counting.
  - `handlers_name_what_they_handle`: `handled_by` on `NotifyLogger::handle` gives `["App\Message\Notify"]`, on `SmsHandlers::onSms` both message classes.

- [ ] **Step 3: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: FAIL to compile.

- [ ] **Step 4: Implement** `messenger.rs`, read with the routes in the module's `rebuild` and `update`.

- [ ] **Step 5: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 6: Commit** with message `feat(symfony): read message handlers`.

### Task 2: From dispatch to handlers

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/frameworks/drupal/mod.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/index.rs`, `crates/phplumb-index/tests/symfony.rs`, `crates/phplumb-index/tests/fixtures/symfony/**`

**Interfaces:**

- Consumes: `Symfony::handlers_of` (Task 1).
- Produces:
  - `Framework::call_targets(&self, index: &Index, call: &FrameworkCall<'_>, arguments: &[TypeExpr]) -> Vec<(Location, String)>`: targets for a definition request on the name of a method call, given the types of its positional arguments; the Drupal module answers nothing.
  - `infer::call_at(index, path, summary, source, offset) -> Option<(Vec<CallTarget>, Vec<Option<String>>, TypeExpr, Vec<TypeExpr>)>`: for the method or static call whose name holds `offset`, its targets, string arguments, receiver type and argument types.
  - `Index::resolve` asks `frameworks_for(path)` for `call_targets` when the cursor is on a method call's name, and returns their targets instead of the declaration when there are any.

- [ ] **Step 1: Write the failing tests.** Add `src/Controller/MessagesController.php` with a promoted `MessageBusInterface $bus` and, on their own lines, `$this->bus->dispatch(new SendEmail());`, `$this->bus->dispatch(new SendSms());`, `$this->bus->dispatch(new Unhandled());`, `$this->bus->dispatch($message);` with an untyped `$message`, and `$this->bus->dispatch(new Envelope(new SendEmail()));`:
  - `dispatch_lists_the_handlers`: definition on `dispatch` of the first two lines gives the Task 1 handlers.
  - `dispatch_without_handlers_keeps_its_declaration`: on the other three lines, definition on `dispatch` gives `MessageBusInterface::dispatch`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: `dispatch_lists_the_handlers` FAILS.

- [ ] **Step 3: Implement** the hook, `infer::call_at`, the resolve wiring and the Symfony answer.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from dispatch to message handlers`.

### Task 3: From handlers to dispatches

**Files:**

- Modify: `crates/phplumb-index/src/frameworks/mod.rs`, `crates/phplumb-index/src/infer.rs`, `crates/phplumb-index/src/frameworks/symfony/mod.rs`, `crates/phplumb-index/tests/symfony.rs`

**Interfaces:**

- Consumes: `Symfony::handled_by` (Task 1).
- Produces: `FoundCall::arguments: Vec<TypeExpr>`, the type of each positional argument, `Other` when unknown.

- [ ] **Step 1: Write the failing tests:**
  - `handlers_list_their_dispatches`: definition on `__invoke` of `SendEmailHandler` gives the `dispatch` of `new SendEmail()`; on `handle` of `NotifyLogger`, the dispatches of `new SendEmail()` and `new SendSms()`; on `run` of `Explicit`, that of `new SendSms()`.
  - `the_handler_attribute_lists_the_dispatches`: definition inside the class-level `#[AsMessageHandler]` of `SendEmailHandler` gives the same as on its `__invoke`.

- [ ] **Step 2: Run them and watch them fail.** Run: `cargo test -p phplumb-index --test symfony`. Expected: both FAIL.

- [ ] **Step 3: Implement** `FoundCall::arguments` and the Symfony `declaration_targets` for handlers, finding dispatches with `index.calls_named(&["dispatch"])`.

- [ ] **Step 4: Run every suite.** Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`. Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(symfony): navigate from message handlers to dispatches`.

### Task 4: Reference cases, benchmarks and backlog

**Files:**

- Modify: `docs/benchmarks.md`, `docs/backlog.md`, `README.md`
- Modify outside the repository: `$HOME/.local/share/phplumb/cases/symfony.toml`

- [ ] **Step 1: Build the 6c baseline** from `git merge-base main HEAD`.

- [ ] **Step 2: Probe every `dispatch(` call** in the Symfony reference project's `src`, count those whose definition lists handlers, and check those that do not by hand. Add cases: a `dispatch` with its handler, and a handler `__invoke` with its dispatch.

- [ ] **Step 3: Run both benchmarks and both diagnostics sweeps,** for this branch and the baseline. Expected: the Drupal reference project unchanged; Symfony coverage, cases and diagnostics at least the baseline's; cold index under 10000 and `peak_rss_mb` under 1024. If a target is missed, stop and report to the user.

- [ ] **Step 4: Record the results.** Add a dated section `2026-10-28, step 6d Symfony Messenger` to `docs/benchmarks.md`, a README bullet under Symfony, and to `docs/backlog.md` anything this step defers, such as `Envelope` messages, handlers configured in YAML, `fromTransport` and bus-specific handlers. Run the Markdown formatter on the three files.

- [ ] **Step 5: Commit** with message `docs: record symfony messenger results`.
