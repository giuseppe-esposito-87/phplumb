# Benchmarks

Results of `phplumb bench` on the reference Drupal project. The case file lives outside the repository because it points into client code.

## 2026-10-27, step 4a Drupal services

Same machine and project. The Drupal module reads 252 services files (test modules and `vendor` excluded) and types `\Drupal::service()` and container `get()` calls.

| Metric                                  | Value                             | Target        |
| --------------------------------------- | --------------------------------- | ------------- |
| Member coverage in `web/modules/custom` | 42,303 of 46,204 accesses (91.6%) | above 90.7%   |
| Cold index                              | 3.4 to 4.5 s                      | under 10 s    |
| Peak resident memory                    | 382 to 402 MB                     | under 1024 MB |
| All cases                               | 52 passed, 0 failed               | all pass      |

New cases cover a method after `\Drupal::service()`, after `$container->get()` in `create()`, through an alias, a service id leading to its YAML key, and an id defined nowhere on disk. A service another module decorates resolves to both classes: `entity_type.manager` gives the core class and the webprofiler wrapper. Two earlier cases changed because more calls are now typed: a method on a service result that used to expect nothing, and the references of `EntityStorageInterface::load`, which grew from 1,033 to 1,104 with calls on storages taken from the container.

## 2026-10-27, step 3 broken reference diagnostics

Same machine and project. `phplumb bench --diagnostics <dir>` checks every file below a directory, whatever the editor's project-code scope, and lists each diagnostic.

| Area                  | Files  | Diagnostics | In `tests` | Slowest file |
| --------------------- | ------ | ----------- | ---------- | ------------ |
| `web/modules/custom`  | 1,228  | 88          | 0          | 2 ms         |
| `web/core`            | 11,473 | 65          | 64         | 32 ms        |
| `web/modules/contrib` | 6,596  | 253         | 89         | 28 ms        |

Cold index 4.3 s, peak memory 429 MB, 48 of 48 cases, member coverage 90.7%.

**Custom code: no false alarms.** All 88 diagnostics come from one module that uses three Composer packages (a SOAP client, its encoding library and a type library) that the project does not install; those classes and functions exist nowhere on disk.

**Core and contrib** were used to hunt false alarms, since that code works. The first run found four classes of false alarm, each now covered by a fixture case and fixed:

- A class declared twice (a static-analysis stub and the real class): member lookups now try every declaration of a name, which also fixes go to definition through such a parent.
- A receiver typed only by a wrong docblock, or holding a test mock: a method on any receiver other than `$this` is flagged only when no indexed or built-in class declares a method with that name.
- Methods added by trait `as` aliases.
- Example code in `*.api.php` hook documentation.

The final review found five more classes, now also fixed and covered: trait aliases declared inside another trait, `$this` and `self::` inside closures (whose scope can be rebound), classes created with `class_alias()`, `is_callable()` and named-argument guards, and names in namespaces that `phplumb.exclude` hides from the index.

What remains are references to modules and libraries the project does not install (optional integrations), Drupal 7 and Drush 8 functions in legacy files, and methods no class declares at all, such as `ModuleHandlerInterface::getImplementations()`, which Drupal 11 removed. One known false alarm class remains: classes that a vendor package loads with `include` instead of through Composer's autoloader (geoPHP's `Geometry`), because the vendor filter does not index those files. It does not occur in this project's custom code.

## 2026-10-27, step 2e Linux binary

Static musl builds in Docker Desktop on the same Mac, with the project copied into the container's own filesystem. Bind-mounting the project from macOS instead raises the cold index to 18 to 19 seconds on both architectures, so that number measures Docker Desktop's file sharing, not the engine.

| Metric                  | `x86_64` (emulated) | `aarch64` (native)  | Target        |
| ----------------------- | ------------------- | ------------------- | ------------- |
| Cold index              | 2.5 s               | 1.7 s               | under 10 s    |
| Peak resident memory    | 475 MB              | 447 MB              | under 1024 MB |
| Slowest references case | 273 ms              | 134 ms              | under 1 s     |
| Slowest symbol query    | 32 ms               | 33 ms               | under 100 ms  |
| All cases               | 48 passed, 0 failed | 48 passed, 0 failed | all pass      |

The `x86_64` binary also runs on Debian 11 (glibc 2.31), and the Rust test suite passes on `linux/amd64`. No native Linux x64 machine was used.

## 2026-10-26, step 2e symbols and outline

Same machine and project. Every run now times five workspace symbol queries (`node`, `EntityStorage`, `gEnt`, `load` and `zzqqxx`) over all declarations and members in the index.

| Metric                  | Value               | Target        |
| ----------------------- | ------------------- | ------------- |
| Cold index              | 2.6 to 4.3 s        | under 10 s    |
| Peak resident memory    | 345 to 366 MB       | under 1024 MB |
| Slowest symbol query    | 29 to 31 ms         | under 100 ms  |
| Slowest references case | 103 to 135 ms       | under 1 s     |
| All cases               | 48 passed, 0 failed | all pass      |

## 2026-10-26, step 2d references and call hierarchy

Same machine and project. Each indexed file now records hashes of the names it mentions, so a references query parses only the files that can contain a match.

| Metric                                  | Value                                                       | Target        |
| --------------------------------------- | ----------------------------------------------------------- | ------------- |
| Cold index                              | 2.8 s                                                       | under 10 s    |
| Peak resident memory                    | 381 MB                                                      | under 1024 MB |
| Slowest references case                 | 108 ms (1,033 references of `EntityStorageInterface::load`) | under 1 s     |
| Slowest implementation case             | 11 ms                                                       | under 1 s     |
| All cases                               | 48 passed, 0 failed                                         | all pass      |
| Member coverage in `web/modules/custom` | 41,894 of 46,204 accesses (90.7%)                           | 90.7% before  |

The custom references cases match an independent text search exactly. They cover two classes named `ImportStatisticsTracker` in different modules, where each class and its `trackCreated` method must list only its own uses, and a global function whose docblock mentions are not references. The expected list of the `load` case comes from the engine, with 20 sampled entries checked by hand.

## 2026-10-26, step 2c implementations and type hierarchy

Same machine and project. Members documented with `{@inheritdoc}` and no type of their own now take the nearest typed ancestor's type.

| Metric                                  | Value                                             | Target        |
| --------------------------------------- | ------------------------------------------------- | ------------- |
| Cold index                              | 2.4 to 3.9 s                                      | under 10 s    |
| Peak resident memory                    | 354 MB                                            | under 1024 MB |
| Slowest implementation case             | 7 ms (195 implementations of `ProcessPluginBase`) | under 1 s     |
| Definition and implementation cases     | 42 passed, 0 failed                               | all pass      |
| Member coverage in `web/modules/custom` | 41,894 of 46,204 accesses (90.7%)                 | 88.6% before  |

The implementation cases were checked against an independent text scan of `extends` and `implements` across core, contrib and custom code; every difference was a limit of the scan (classes sharing a short name), not of the engine.

## 2026-10-26, step 2b members and hover

Same machine and project. The index now records class shapes (parents, interfaces, traits and typed members); signatures and docblocks are read from the file when hover needs them.

| Metric                                  | Value                             | Target        |
| --------------------------------------- | --------------------------------- | ------------- |
| Cold index                              | 2.6 to 3.8 s                      | under 10 s    |
| Warm start from the cache               | 1.6 to 1.7 s                      | under 2 s     |
| Update of 1,000 files                   | 39 ms                             | under 5 s     |
| Peak resident memory                    | 426 to 471 MB                     | under 1024 MB |
| Cache size                              | 21 MB                             |               |
| Definition cases                        | 36 passed, 0 failed               | all pass      |
| Member coverage in `web/modules/custom` | 40,951 of 46,204 accesses (88.6%) | baseline      |

The coverage counts every `->`, `?->` and `::` access whose member is named in the code and reaches at least one definition. A sample of 27 resolved accesses, read by hand, had no wrong target. Most unresolved accesses go through the Drupal service container or magic field properties, which the Drupal module addresses.

## 2026-10-26, step 2a index freshness

Same machine and project. `bench --cache` writes the cache after the cold build, builds again from it, then updates files without changing them.

| Metric                    | Value               | Target        |
| ------------------------- | ------------------- | ------------- |
| PHP files indexed         | 39,417              |               |
| Cold index                | 2.2 to 3.7 s        | under 10 s    |
| Warm start from the cache | 1.6 to 1.7 s        | under 2 s     |
| Update of one file        | under 1 ms          | under 0.5 s   |
| Update of 1,000 files     | 37 to 39 ms         | under 5 s     |
| Peak resident memory      | 220 to 339 MB       | under 1024 MB |
| Cache size                | 8 MB                |               |
| Definition cases          | 20 passed, 0 failed | all pass      |

Warm start time is mostly the directory walk and file stats; parsing is skipped for unchanged files.

## 2026-10-26, step 1 skeleton

Machine: Apple M3 Pro, 11 cores, 36 GB RAM. Release build. Reference project: Drupal `11.4.10`.

| Metric                              | Value               | Target        |
| ----------------------------------- | ------------------- | ------------- |
| PHP files indexed                   | 39,417              |               |
| Declarations                        | 39,830              |               |
| Files with parse errors             | 0                   |               |
| Cold index, warm file cache         | 2.9 to 3.8 s        | under 10 s    |
| Cold index, first run after a while | 6.7 s               | under 10 s    |
| Peak resident memory                | 185 to 283 MB       | under 1024 MB |
| Definition cases                    | 20 passed, 0 failed | all pass      |

Without the Composer vendor filter the same project yields 52,948 files and 242 files with parse errors. The difference is package tests, fixtures and a stale copy of Drupal core under `vendor/composer` that no autoloader reaches.

The case set covers custom code reaching core, contrib and vendor classes, functions declared in `.module` and `.inc` files, an aliased namespace import, a class declared in two places, and four positions that must return nothing.
