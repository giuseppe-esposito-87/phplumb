# PHP Navigator 2a: index freshness implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the index in step with the files on disk: follow changes made by agents and branch switches, start warm from an on-disk cache, and never return a target whose file changed since it was indexed.

**Architecture:** `phpnav-index` gains a `PathFilter` shared by discovery and updates, an incremental `Index::update`, a postcard-encoded cache validated by file size and modification time, and a self-healing `Index::definition` that re-indexes a target file whose name moved. `phpnav-server` registers file watchers with the client, applies change batches, keeps the cache in the directory the extension provides, and logs at info level.

**Tech Stack:** Rust (existing workspace), `postcard` 1.1 with `serde`, LSP dynamic registration of `workspace/didChangeWatchedFiles`, VS Code `ExtensionContext.storageUri`.

**Spec:** `docs/superpowers/specs/2026-10-26-php-navigator-design.md`

**Scope:** First part of delivery step 2. Member navigation (2b), hierarchies (2c), references (2d), symbols, outline and Linux packaging (2e) have their own plans.

## Global constraints

- Everything in the skeleton plan's global constraints still holds: crate versions, names, UTF-16 positions, no wrong jumps, stdout reserved for the protocol, read-only reference project, comments describe the present, conventional commits with `Assisted-by: claude-code/claude-opus-5-5`, work on `main`.
- **New crate:** `postcard = { version = "1.1", features = ["use-std"] }`. `serde` derives on the cached types only.
- **Targets on the reference project (Apple M3 Pro):** warm start under 2 seconds; one changed file visible in under 0.5 seconds; 1,000 changed files visible in under 5 seconds; memory under 1 GB.
- **Cache file:** `<cacheDir>/index-v1.bin`, written to a temporary file in the same directory and renamed into place. A missing, corrupt or mismatched cache means a full build, never an error.
- **Rust toolchain** is keg-only: prepend `/opt/homebrew/opt/rustup/bin` to `PATH`.

## Review focus

- **A branch switch** delivers hundreds of watcher events in several notifications; the server must apply them as one batch and stay responsive. Test in Task 4.
- **A deleted directory** produces a single delete event for the directory, not for its files; every indexed file under it must disappear. Test in Task 2 and Task 4.
- **A file created inside an excluded or filtered path** (for example `vendor/composer/<hash>/X.php` or a path matching `phpNavigator.exclude`) must stay out of the index after an update. Test in Task 2.
- **A file rewritten with the same size and an older modification time** (some tools restore timestamps) must not keep stale declarations forever: the definition self-heal catches it. Test in Task 3.
- **A cache written by a different engine version or with different exclusions** must be ignored. Test in Task 3.

---

### Task 1: Log indexing at info level

**Files:**

- Modify: `crates/phpnav-server/src/lsp.rs`
- Test: `crates/phpnav-server/tests/lsp_definition.rs`

**Interfaces:**

- Produces: `fn log_info(connection: &Connection, message: String) -> anyhow::Result<()>` in `lsp.rs`, sending `window/logMessage` with `MessageType::INFO`. Later tasks use it for update and cache messages.

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn logs_indexing_at_info_level()
// Same workspace as definition_over_stdio. After initialize and initialized, read messages until a
// "window/logMessage" notification arrives (fail after 5 seconds).
// Assert: params.type == 3 and params.message starts with "Indexed 2 files in ".
```

Extend the test client with `fn wait_for_notification(&mut self, method: &str) -> Value`.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-server --test lsp_definition logs_indexing_at_info_level`
Expected: FAIL, no `window/logMessage` within 5 seconds.

- [ ] **Step 3: Replace the `eprintln!` in `build_index` with `log_info`.** Keep stderr for real errors only.

- [ ] **Step 4: Run the test and the whole server suite**

Run: `cargo test -p phpnav-server`
Expected: all pass.

- [ ] **Step 5: Commit** with message `fix(server): log indexing results at info level`.

### Task 2: Path filter and incremental updates

**Files:**

- Create: `crates/phpnav-index/src/filter.rs`
- Modify: `crates/phpnav-index/src/discover.rs`, `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/lib.rs`
- Test: `crates/phpnav-index/tests/update.rs`

**Interfaces:**

- Produces:

```rust
/// The rules `discover` applies to each file, usable on a single path.
pub struct PathFilter { /* canonical root, exclude GlobSet, max_file_bytes, VendorFilter cache */ }
impl PathFilter {
    pub fn new(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<PathFilter>;
    pub fn root(&self) -> &Path;
    /// Extension, hidden directories, node_modules, exclude globs, size limit and the
    /// Composer vendor filter. `path` is absolute and need not exist for the path rules.
    pub fn accepts(&mut self, path: &Path) -> bool;
}

#[derive(Debug, Default, PartialEq)]
pub struct UpdateStats { pub reparsed: usize, pub removed: usize, pub elapsed_ms: u128 }

impl Index {
    /// `changed` holds created or modified paths; `removed` holds deleted files or
    /// directories. Changed paths the filter rejects, or that no longer exist, are
    /// treated as removed.
    pub fn update(&mut self, changed: &[PathBuf], removed: &[PathBuf]) -> UpdateStats;
}
```

`discover` keeps its signature and uses `PathFilter` internally. `Index` keeps its `PathFilter` from `build`. Files are slots in a `Vec<Option<FileEntry>>` plus a `HashMap<PathBuf, FileId>`; removing a file leaves an empty slot. Each `FileEntry` remembers its declarations so their keys can be removed from the two maps.

- [ ] **Step 1: Write the failing tests** in `tests/update.rs`, each on a `TempDir` workspace built with `Index::build`:

```rust
#[test] fn update_sees_a_moved_class()          // rewrite A.php so `class Target` moves to line 5; update(changed=[A.php]); definition from Use.php now points at the new offset
#[test] fn update_adds_a_created_file()         // create New.php declaring `class Fresh`; update(changed=[New.php]); `new Fresh()` resolves
#[test] fn update_removes_a_deleted_file()      // delete A.php; update(removed=[A.php]); definition of Target returns nothing; stats.removed == 1
#[test] fn update_removes_a_deleted_directory() // delete dir lib/ holding 2 files; update(removed=[lib]); both declarations gone; stats.removed == 2
#[test] fn update_rejects_filtered_paths()      // create vendor/composer/abc/X.php (with an installed.json) and excluded/Y.php (exclude ["excluded/**"]); update(changed=[both]); neither class resolves
#[test] fn update_treats_missing_changed_paths_as_removed() // update(changed=[deleted A.php]) removes it
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test update`
Expected: FAIL to compile, `Index::update` not found.

- [ ] **Step 3: Implement `PathFilter`, refactor `discover` onto it, and implement `Index::update`.** Summaries of changed files run in parallel with `rayon`.

- [ ] **Step 4: Run the whole index suite**

Run: `cargo test -p phpnav-index`
Expected: all pass, including the existing `discover` and `fixtures` tests.

- [ ] **Step 5: Commit** with message `feat(index): update the index incrementally`.

### Task 3: On-disk cache and self-healing definitions

**Files:**

- Create: `crates/phpnav-index/src/cache.rs`
- Modify: `crates/phpnav-index/src/index.rs`, `crates/phpnav-index/src/summary.rs` (serde derives on `Declaration`, `SymbolKind`), `crates/phpnav-index/Cargo.toml`, root `Cargo.toml`
- Test: `crates/phpnav-index/tests/cache.rs`, `crates/phpnav-index/tests/update.rs`

**Interfaces:**

- Produces:

```rust
pub struct IndexStats { /* existing fields */ pub reused: usize, pub reparsed: usize }

impl Index {
    /// Loads `cache` when it was written by this engine version for the same root and
    /// options, reuses entries whose file size and modification time are unchanged,
    /// and reparses the rest. Falls back to a full build on any cache problem.
    pub fn build_with_cache(root: &Path, opts: &DiscoverOptions, cache: &Path) -> anyhow::Result<(Index, IndexStats)>;
    pub fn save_cache(&self, cache: &Path) -> anyhow::Result<()>;
    /// Now takes `&mut self`: a target whose file no longer holds the declared name at
    /// the indexed range is re-indexed with `update`, and the query runs once more.
    pub fn definition(&mut self, path: &Path, source: &[u8], offset: u32) -> Vec<Location>;
}
```

The cache header holds the format version `1`, `env!("CARGO_PKG_VERSION")`, the canonical root, the exclude patterns and `max_file_bytes`. Each entry holds the path, size, modification time in nanoseconds and declarations.

- [ ] **Step 1: Write the failing tests**

```rust
// tests/cache.rs
#[test] fn warm_start_reuses_unchanged_files()  // build_with_cache twice on a 3-file workspace; second stats: reused == 3, reparsed == 0; definitions identical
#[test] fn warm_start_reparses_changed_files()  // between the two builds, append a class to one file; second stats: reused == 2, reparsed == 1; the new class resolves
#[test] fn warm_start_drops_deleted_and_adds_new_files() // delete one file and create another between builds; definitions reflect both
#[test] fn cache_with_other_exclusions_is_ignored()      // second build with a different exclude list: reused == 0
#[test] fn corrupt_cache_falls_back_to_full_build()      // write garbage to the cache file; build_with_cache succeeds with reused == 0
#[test] fn save_cache_replaces_atomically()              // save twice; only index-v1.bin exists in the directory afterwards

// tests/update.rs
#[test] fn definition_heals_a_stale_target()
// Build the index, then rewrite A.php so `class Target` moves, set its mtime back to the original
// value, and do not call update. definition from Use.php returns the new offset.
```

Set modification times with `std::fs::File::set_modified`.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-index --test cache --test update`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `cache.rs`, `build_with_cache`, `save_cache` and the self-healing `definition`.** Update the callers in `crates/phpnav-index/tests/fixtures.rs`, `crates/phpnav-server/src/lsp.rs` and `crates/phpnav-server/src/bench.rs` for `&mut self`.

- [ ] **Step 4: Run the workspace suite**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(index): cache the index on disk and heal stale targets`.

### Task 4: File watching in the server

**Files:**

- Modify: `crates/phpnav-server/src/lsp.rs`
- Test: `crates/phpnav-server/tests/lsp_watch.rs` (move the shared stdio test client to `crates/phpnav-server/tests/common/mod.rs`)

**Interfaces:**

- Consumes: `Index::update`, `Index::build_with_cache`, `Index::save_cache` (Tasks 2 and 3), `log_info` (Task 1).
- Produces: `ServerSettings` gains `cache_dir: Option<PathBuf>` (JSON key `cacheDir`).

Server behavior:

- After `initialized`, when the client supports `workspace.didChangeWatchedFiles.dynamicRegistration`, the server sends `client/registerCapability` with two watchers: `**/*.{php,module,inc,install,theme,profile,engine}` (create, change, delete) and `**/*` (delete only, so deleted directories are seen).
- With `cacheDir` set, the index is built with `build_with_cache` and saved after the first build and on `shutdown`.
- On `workspace/didChangeWatchedFiles`, the server drains every queued `didChangeWatchedFiles` notification first, then applies one `update`. A change to any `vendor/composer/installed.json` triggers a full rebuild instead. Each batch logs `Reindexed <reparsed> files, removed <removed> in <ms> ms`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn registers_file_watchers()
// initialize with capabilities.workspace.didChangeWatchedFiles.dynamicRegistration = true.
// Assert: a client/registerCapability request arrives whose registration has method
// "workspace/didChangeWatchedFiles" and two watchers; answer it with a null result.

#[test] fn applies_watched_file_changes()
// Rewrite src/Target.php so the class moves to line 4, send didChangeWatchedFiles (type 2),
// then definition from Use.php: range.start.line == 4.

#[test] fn applies_a_deleted_directory()
// Workspace: lib/Target.php and src/Use.php. Delete lib/ entirely and send didChangeWatchedFiles
// with one delete event for the lib directory. Definition from Use.php returns an empty array.

#[test] fn coalesces_bursts_of_changes()
// Send 50 didChangeWatchedFiles notifications back to back, then one definition request.
// Assert: exactly one "Reindexed" logMessage arrives before the definition response.

#[test] fn writes_the_cache_dir()
// initialize with initializationOptions.cacheDir = <temp dir>; after shutdown, <temp dir>/index-v1.bin exists.
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p phpnav-server --test lsp_watch`
Expected: FAIL.

- [ ] **Step 3: Implement registration, batching, the `installed.json` rebuild and the cache directory.**

- [ ] **Step 4: Run the workspace suite**

Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit** with message `feat(server): follow file changes and keep a warm cache`.

### Task 5: Benchmark freshness targets

**Files:**

- Modify: `crates/phpnav-server/src/bench.rs`, `crates/phpnav-server/tests/bench.rs`, `docs/benchmarks.md`

**Interfaces:**

- Produces: `bench` accepts `--cache <FILE>`. With it, the report adds `warm_start_ms`, `update_one_ms` and `update_1000_ms` (updating the first file, then the first 1,000 files in index order, without changing them on disk).

- [ ] **Step 1: Write the failing test**

```rust
#[test] fn bench_reports_warm_start_and_updates()
// bench --root <fixtures/names> --cache <temp file>
// Assert: exit 0; warm_start_ms, update_one_ms and update_1000_ms are numbers; the cache file exists.
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p phpnav-server --test bench`
Expected: FAIL, unknown option `--cache`.

- [ ] **Step 3: Implement the option.**

- [ ] **Step 4: Run the test, then the reference benchmark**

Run: `cargo build --release -p phpnav-server && ./target/release/php-navigator bench --root <reference-project> --cases "$HOME/.local/share/php-navigator/cases/reference.toml" --cache "$TMPDIR/phpnav-bench-cache.bin"`
Expected: `cases_failed` 0, `warm_start_ms` under 2000, `update_one_ms` under 500, `update_1000_ms` under 5000, `peak_rss_mb` under 1024.

If a target is missed, stop and report the numbers to the user.

- [ ] **Step 5: Add a dated section to `docs/benchmarks.md`** with the new numbers.

- [ ] **Step 6: Commit** with message `feat(server): benchmark warm start and incremental updates`.

### Task 6: Extension cache directory and an end-to-end freshness test

**Files:**

- Modify: `editors/vscode/src/extension.ts`, `editors/vscode/src/test/definition.test.ts`, `docs/development.md`
- Create: `editors/vscode/test-fixtures/basic/src/Moving.php`, `editors/vscode/test-fixtures/basic/src/UseMoving.php`

**Interfaces:**

- Consumes: `initializationOptions.cacheDir` (Task 4).
- Produces: the extension passes `cacheDir: context.storageUri?.fsPath`, creating the directory first; without a workspace it passes nothing. `activate` returns `{ cacheDir: string | undefined }` so tests can find the cache.

- [ ] **Step 1: Write the failing tests**

```ts
test("keeps the index cache in workspace storage", async () => {
  // const api = await vscode.extensions.getExtension("sparkfabrik.php-navigator")!.activate();
  // Wait for a definition to answer, then poll up to 10 seconds for path.join(api.cacheDir, "index-v1.bin").
  // Assert: api.cacheDir is defined and the file exists.
});

test("follows a class moved on disk", async () => {
  // UseMoving.php: "<?php\nnamespace App;\nnew Moving();\n"; Moving.php declares `class Moving {}` on line 2.
  // Wait for the definition at line 2, character 4 to reach Moving.php line 2.
  // Rewrite Moving.php with Node's fs so the class is on line 5; poll the definition for up to 5 seconds.
  // Assert: it reaches line 5. Restore the original content in a finally block.
});
```

The second test is an end-to-end check of Task 4 and passes as soon as it is written.

- [ ] **Step 2: Run them and watch the first fail**

Run: `cargo build -p phpnav-server && npm --prefix editors/vscode test`
Expected: "keeps the index cache in workspace storage" fails because `activate` returns nothing; "follows a class moved on disk" passes.

- [ ] **Step 3: Pass `cacheDir` from `extension.ts` and return it from `activate`.**

- [ ] **Step 4: Run the extension suite**

Run: `npm --prefix editors/vscode run typecheck && npm --prefix editors/vscode test`
Expected: all pass.

- [ ] **Step 5: Update `docs/development.md`** with a short note: the index follows file changes and is cached per workspace.

- [ ] **Step 6: Commit** with message `feat(vscode): keep the index cache in workspace storage`.
