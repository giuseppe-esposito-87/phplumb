# Benchmarks

Results of `php-navigator bench` on the reference Drupal project. The case file lives outside the repository because it points into client code.

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
