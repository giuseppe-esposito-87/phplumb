# PHP Navigator design

PHP Navigator (working name) is a VS Code extension that gives PHPStorm-grade code navigation for large PHP codebases, starting with Drupal. It uses its own engine and does not wrap an existing language server.

Status: draft, 2026-10-26.

## Why

The user reviews PHP code written by AI agents on Drupal projects. They read code and do not write it. Reading means jumping around: from a method call to its definition, from an interface to its implementations, from a method to every place that calls it.

The VS Code PHP extensions in use today (PHP Intelephense free tier and PHP IntelliSense) fail at this. Indexing is slow on large projects. Free Intelephense has no "find implementations". On Drupal code the type of an object is lost as soon as it comes out of the service container or entity storage, so navigation stops there.

## How it is used

- **Who.** The user first. The Drupal team at SparkFabrik may use it later.
- **Where it runs.** VS Code on the macOS host, reading the project from the local disk. The user almost never runs VS Code inside the project's dev container, so that setup is not supported.
- **What changes the code.** An AI agent edits files on disk, outside the editor, often many files at once. The user also switches git branches. The index must follow these changes without the user opening the files.
- **Where the user reads.** In normal editors and in the VS Code diff view for working-tree changes, where the modified side is the file on disk. Virtual documents are out of scope: commit and branch diffs, merge request reviews, and agent edits not yet saved to disk.

## Goals

The core PHP features, in any PHP project:

- **Go to definition** for classes, interfaces, traits, enums, functions, methods, properties, constants and class constants.
- **Find implementations.** From an interface or abstract class to every concrete implementation, transitively. From an interface or abstract method to every method that implements it.
- **Find references and callers** of a symbol. For a method, results include calls made through the interface or parent method it implements or overrides, grouped under that method, as PHPStorm does. Drupal code calls almost everything through interfaces, so without these the list would be nearly empty.
- **Type hierarchy.** Supertypes and subtypes of a class or interface.
- **Call hierarchy.** Incoming and outgoing calls of a function or method.
- **Hover** with the resolved type, the signature and the docblock.
- **Symbol search** across the workspace, and the outline of the current file.
- **Broken reference diagnostics.** Flag a class, function, method or constant that does not exist. This catches code an agent invented.

The Drupal module, optional and configurable:

1. **Services.** Read every `*.services.yml` in core, contrib and custom code, including aliases, factories, `parent` and services whose id is a class name. Use them to type the result of `\Drupal::service('id')`, `$container->get('id')` and shortcuts such as `\Drupal::entityTypeManager()`, so navigation continues on the next method call. Cmd+click on a service id string opens its YAML definition.
2. **Entities.** Type `getStorage('node')` as the storage class of that entity type, and `->load()` as the entity class. Read entity types from both PHP attributes and annotations.
3. **Hooks.** Navigate from a hook implementation (`#[Hook('form_alter')]` or a procedural `mymodule_form_alter()`) to the places that invoke the hook. From an invocation, list every implementation. Most hooks are invoked under computed names, such as `hook_form_FORM_ID_alter` or `hook_ENTITY_TYPE_presave`, so a built-in table of Drupal hook-name patterns maps those implementations to their invocation sites. Every implementation also links to its `hook_*` documentation in `*.api.php`. When no pattern matches, the documentation is the only target.
4. **Plugins.** Navigate from a plugin id to its class, and from a plugin manager to all its plugins. Support both attribute and annotation discovery.
5. **Routes.** Navigate from `Url::fromRoute('name')` and similar calls to the route in `*.routing.yml`, then to the controller or form class.
6. **YAML navigation.** Cmd+click on class and method references inside services, routing and links YAML files opens the PHP code.
7. **Broken Drupal references.** Flag service ids, route names and plugin ids that do not exist.

The Drupal registries (services, entity types, hooks, plugins and routes) include every module on disk except test modules, which are modules inside a `tests` directory. The PHP classes of test modules stay indexed for core navigation. The module does not read which modules are enabled. A broken Drupal reference is flagged only when the id is defined nowhere on disk, test modules included.

## Non-goals

- Autocomplete, signature help, snippets, formatting, rename and other refactorings, code actions and debugging. The user does not write the code.
- General static analysis beyond broken references. PHPStan already covers it.
- Symfony and Laravel modules. They may come later, and the architecture must leave room for them.
- Editors other than VS Code. The LSP boundary makes them possible, but they are not tested.

## Anti-goals

These make the tool a failure even if every feature exists:

- **Wrong jumps.** A wrong target is worse than no target. Results come only from resolved types, never from matching a method name alone. When the type of a receiver is unknown, the engine returns nothing for that call. When the type is a union, the engine returns one target per member, which is correct, not a guess. Calls grouped under an interface or parent method are labeled as such, so they do not count as wrong targets.
- **False alarms.** A broken reference diagnostic appears only when the engine is certain the target does not exist. If any part of the resolution is uncertain, it stays silent. Noisy diagnostics get ignored.
- **A heavy editor.** Indexing must not make VS Code sluggish or exhaust memory.

## Reference project and success criteria

The reference project is a client Drupal site at `<reference-project>`. It is used read-only and is never modified or committed. Tests and benchmarks find it through an environment variable, so other machines can point to another project.

Measured on 2026-10-26:

| Area                             | PHP files    | Lines      |
| -------------------------------- | ------------ | ---------- |
| `vendor`                         | 33,490       | 3.9M       |
| `web/core`                       | 11,475       | 1.7M       |
| `web/modules/contrib`            | 6,599        | 1.0M       |
| `web/modules/custom`             | 1,228        | 214k       |
| Total, including tests and stubs | about 53,000 | about 6.9M |

The project runs Drupal `11.4.10` and PHP `8.5` locally. Custom code uses only modern Drupal style: 132 `#[Hook]` attributes, 91 plugin attributes, no plugin annotations and a single `.module` file. Core and contrib mix attributes and annotations. The code contains about 6,700 `\Drupal::service(` calls, 3,500 `$container->get(` calls and 3,700 `->getStorage(` calls.

Success targets on the user's machine (Apple M3 Pro, 11 cores, 36 GB RAM):

- **Cold index** of the whole reference project in under 10 seconds.
- **Warm start** from the on-disk index in under 2 seconds.
- **Change on disk** of a single file visible in navigation in under 0.5 seconds. A branch switch or `composer install` that changes 1,000 files is visible in under 5 seconds.
- **Queries** for references, callers and incoming calls answer in under 1 second, even for common method names such as `load()` or `get()`. They search all indexed code, including `vendor` and core, and list results from project code first.
- **Memory** of the engine under 1 GB resident on the reference project.
- **Correctness** is checked by a set of navigation cases. Each case states its expected result: one target, a list of targets, or nothing when the type cannot be known. A case passes only on an exact match, so returning nothing where a target is expected fails, and a wrong target fails the build.
- **Coverage.** The benchmark reports the share of method calls in `web/modules/custom` whose target the engine resolves. Once the Drupal module ships, the target is at least 90%.

The agent writes the first correctness cases, and every problem the user reports from the UI becomes a new case. Cases and fixtures committed to the repository use synthetic code only. Cases that point into the reference project contain client code, so they stay outside the repository in a location the benchmark reads from an environment variable.

## Constraints

- **Engine in Rust**, running as a Language Server Protocol server over stdio. The user chose Rust for indexing speed and accepts that they will not read the engine code.
- **Extension in TypeScript**, kept thin: it starts the engine, passes settings and offers a command to restart it.
- **Framework modules are optional.** Drupal can be turned off. Adding Symfony or Laravel later must not require changes to the core navigation code beyond a registration point.
- **Index everything** under the project root by default, including `vendor`, core and contrib, with configurable exclusions. Some vendor packages, such as `rector/rector`, ship scoped copies of other libraries that would create duplicate definitions.
- **Current PHP syntax** up to PHP 8.5, parsed tolerantly: a syntax error in one place must not drop the rest of the file.
- **Platforms:** macOS arm64 and Linux x64.

## Development and test environment

The agent develops and tests automatically. The user tests from the VS Code UI.

Automated tests, run by the agent before handing over any change:

- Rust unit tests for parsing, indexing and type resolution.
- Fixture tests: small PHP and Drupal projects in the repository, with cursor markers and expected targets.
- LSP integration tests that drive the engine over stdio like an editor would.
- Extension end-to-end tests with `@vscode/test-cli` and `@vscode/test-electron`, run against VS Code Insiders so they work while the user's stable VS Code is open.
- A benchmark command that measures cold index, warm start and incremental update on the reference project, and the correctness case set against it.

The user's loop:

1. Press F5 in VS Code to open an Extension Development Host with a separate profile. A watch task rebuilds the extension and the engine.
2. After the agent's change, press Cmd+R in that window to reload with the new build.
3. Report what works and what does not.

A `.vsix` package can also be built and installed in a dedicated profile to test as a normal user would.

## Delivery order

Each step is usable on its own:

1. **Skeleton.** Extension, engine, the test layers and the benchmark harness, with one navigation feature working end to end.
2. **Core PHP navigation.** All core goals except diagnostics, meeting the speed targets.
3. **Core broken references.**
4. **Drupal module**, in the order listed in the goals.

## Decisions made by the agent

The user has not explicitly confirmed these:

- **Working name** PHP Navigator.
- **Delivery order** above.
- **Framework activation.** A setting with the default `auto`: the Drupal module turns on when `composer.lock` contains `drupal/core`. The user can force it on or off.
- **Settings live in VS Code settings** (workspace or user), passed to the engine by the extension.
- **Coexistence with Intelephense.** No special handling. The user disables Intelephense in workspaces where PHP Navigator runs, to avoid duplicate results.
- **Memory target** of 1 GB.
- **Platforms** macOS arm64 and Linux x64, because the team uses both.
- **Distribution.** Local `.vsix` builds for now. No marketplace publishing.
- **Location.** This design lives in `/Users/lussoluca/Sites/SparkFabrik/Internal/php_code`, which becomes the project repository.
- **Diff views** limited to working-tree diffs.
- **Query targets.** References answer in under 1 second, search `vendor` and core too, and list project code first. A batch of 1,000 changed files is visible in under 5 seconds.
- **Correctness policy.** Exact-match cases, including expected "nothing" results. A coverage target of 90% on custom code. Client code stays out of the repository.
- **Drupal module scope.** Test modules are left out of the Drupal registries. Enabled modules are not read.
- **Computed hook names** resolved through a built-in pattern table, with `*.api.php` documentation as a fallback target.

## Left to the builder

- Whether references are resolved during indexing or when a query runs, as long as the speed and memory targets hold.
- Parser choice and how to meet the PHP 8.5 syntax constraint.
- Index data structures, on-disk format, cache invalidation and file watching strategy.
- Depth and design of type inference, including PHPDoc types and generics, as long as the anti-goals hold.
- Concurrency model, crate layout, setting names and packaging.
