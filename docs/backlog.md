# Backlog

Known gaps and deferred work. Each step adds what it leaves behind and removes what it fixes. Non-goals stay in the design documents, not here.

## To fix

### Performance

- **Warm start and large updates miss their targets on the Drupal reference project.** A warm start from the cache takes 4.7 s (target 2 s) and an update of 1,000 files takes 6.8 s (target 5 s). `main` at `35bce14` measures the same, so the slowdown predates step 5a; step 2 measured 1.6 s and 39 ms. Found in step 5a.
- **The stub index builds tables it never uses.** Stub files get reference keys and candidate entries like project files, although references never scan them. Skipping them would save part of the 40 MB the stubs add. Found in the step 5a review.

### Built-in stubs

- **The definition staleness check does not skip stub locations.** 24 namespaced constants declared with `define()`, such as `pcov\all`, never hold their name at their range. Each definition or hover on them runs every framework's update and resolves twice. Leave stub locations out of the stale list in `Index::definition_with`.
- **References from a vendor polyfill list uses that resolve to the stub.** Find references on a polyfill declaration, such as `Stringable` in `symfony/polyfill-php80`, lists project uses whose definition is the stub.
- **Hover on built-in members shows raw attributes.** Parameter attributes such as `#[LanguageLevelTypeAware(...)]` appear verbatim. A return type that only the attribute gives is missing, as in `DateTimeImmutable::modify`.
- **Stub loading has two fragile paths.** `stubs::load` keeps a failed build for the life of the process, and one unreadable directory below the stubs fails the whole stub index.

## Accepted for now

- **Implementations miss project classes below a built-in class.** On `$c->count()` with `$c` a `\Countable`, a project `Bag extends \ArrayObject` is missing, because subtypes come from the project table only.
- **Property types given only by `LanguageLevelTypeAware` are not read.** Summaries keep attributes on functions and methods only, so properties such as `Exception::$file` lose that type. Most such properties also carry `@var`.
- **Stub files are read again after every update.** Property-assignment inference on a class that extends a built-in class re-reads the stub file once the per-update cache is cleared. The files are at most 300 KB.
- **Stub duplicates keep the first file in path order.** `ds` wins over `ds_v2` and `couchbase` over `couchbase_v2`, so users of Ds 2.x or the Couchbase 2 SDK see the other version.
- **Members of built-in classes are never certain for diagnostics.** A typo such as `$date->whatever()` on a `DateTime` stays unflagged, because the stubs give one PHP version and the project may run another.
