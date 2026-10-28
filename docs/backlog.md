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

### Enums

- **Diagnostics on `\UnitEnum` and `\BackedEnum` receivers have no test.** Since step 5b, `$u->nowhere()` on a `\UnitEnum $u`, on a `\BackedEnum`, or on a project interface that extends one, is flagged when no class declares the method. The doc comment of `diagnostics::fully_known` does not mention the exception for these two interfaces. Add a flagged and a silent call to the diagnostics fixture. Found in the step 5b review.
- **Two step 5b behaviours have no test.** `static::Low->label()` inside the enum resolves, and `->value` on a case of a pure enum gives nothing. Add both to the `enums` fixture. Found in the step 5b review.
- **Stub enums list `UnitEnum` twice.** Stubs such as `enum RoundingMode implements \UnitEnum` already name it, and the implicit interface is appended again. Lookups skip names already visited, so nothing breaks today. Skip an implicit name already in `interfaces`. Found in the step 5b review.
- **Some case accesses get no type.** `$this::Low->label()` and `$severity::from('low')->label()` resolve to nothing, because a variable class gives no class. `self::Low->label()` in a trait the enum uses resolves `self` to the trait. Found in the step 5b review.

### Element types

- **Array shapes are not read.** `$record['startedAt']->format()` on an `array{startedAt: \DateTimeImmutable}`, `foreach ($rows as [$a, $b])` and `[$a, $b] = $pair` give nothing. Found in step 5c.
- **Invokable objects give no return type.** `($this->saveTrack)(new SaveTrack(...))->toString()` does not read the `__invoke()` return type. Found in step 5c.
- **Method-level templates are not bound.** `$em->getRepository(Concept::class)->findBy([])` gives nothing, because `@template T` with `@param class-string<T>` on a method is not resolved. Found in step 5c.
- **Implementations do not inherit `@param` types.** A parameter typed `array` in an implementation gets no element type from the `@param list<X>` of the interface method. Found in step 5c.
- **Callbacks of generic methods get no element type.** `$items->filter(fn ($i) => $i->label())` on a `Collection<int, Item>` leaves `$i` unknown; only built-in array functions type their callbacks. Found in step 5c.
- **`static` returns on a generic receiver lose its arguments.** A method returning `static` on an `ArrayCollection<int, Item>` gives a plain `ArrayCollection`. Found in step 5c.
- **Templates are matched by name.** A method-level `@template T` that shadows the class's `T`, or a conditional return type whose subject is a class template, takes the receiver's argument. Found in the step 5c review.
- **Hover shows `mixed` for scalar generic arguments.** `Collection<int, Item>` displays as `\Lib\Collection<mixed, \App\Item>`, because types keep only class names. Found in the step 5c review.
- **Generic arguments are lost in three places.** `parent::method()` reads the parent without the subclass's `@extends` arguments, generic traits (`@use Trait<X>`) are not read, and a class with no templates written `Foo<int, Item>` gives nothing. Found in the step 5c review.
- **Spread arguments give nothing.** `array_map(fn ($x) => ..., ...$lists)` leaves `$x` unknown instead of typing each spread array's element. Found in the step 5c review.
- **`instanceof` inside `match (true)` arms does not narrow.** `$message instanceof RunEvidence => $message->ownerId` gives nothing. Found in step 5c.

### Framework projects

- **Modules linked in from outside a nested project are lost.** With the Drupal `composer.lock` below the root and a module symlinked from a sibling folder, as a Composer path repository does, the module's files lie outside the Drupal project, so its services, routes and plugins are not read. Found in the step 6a review.
- **Annotation navigation is gated by the class's indexed path.** In a file the index does not hold yet, or one `phplumb.exclude` leaves out, Drupal is not asked about `@Block`-style annotations. Pass the file's path to `annotation_classes`. Found in the step 6a review.
- **Hook invocation search reads outside the Drupal project.** `calls_named` runs inference on every candidate file, Symfony files included; the results are right but the work is wasted. Found in the step 6a review.
- **Detection walks the tree once per module.** Each module reads every `composer.lock` up to depth 4, before checking `off`, and detection runs two to three times per start: about 20 ms where it took 6 ms. Found in the step 6a review.
- **Method calls build framework arguments before checking that a module applies.** On a Symfony-only project the empty module makes every call look up its targets twice. Check `frameworks_for(path)` first. Found in the step 6a review.
- **The server log does not name module projects.** Logging `drupal (codebase/drupal), symfony` would show users which folder each module reads. Found in the step 6a review.
- **Some gating has no test.** YAML file targets, declaration targets and keyed strings outside a module's project, and the Drupal update filter on its own, are untested; `Framework::set_project` has no doc comment. Found in the step 6a review.

### Symfony

- **Route names given by class constants are not read.** `#[Route('/dev/login', name: DevLoginAuthenticator::ROUTE)]` makes no route, so 2 of 383 route names in the Symfony reference project resolve to nothing. Found in step 6b.
- **Route imports with `name_prefix` are not read.** Attribute routes under a YAML import that sets `name_prefix` keep their plain names. Found in step 6b.
- **Some route definitions are not read.** The `alias` argument of `#[Route]` and YAML `alias:` routes, localized routes such as `loc.en`, `config/routes.php` and `config/routes/*.php`, and YAML files imported from outside `config/routes` define no route. A localized method also gets a `Class::method` alias that Symfony does not create. Found in the step 6b review.
- **A class-level `#[Route]` with `name: null` drops the whole class.** Symfony reads it as no prefix. Found in the step 6b review.
- **Named arguments give no route.** `redirectToRoute(route: 'home')`, `generate(name: 'x')`, Twig `path(name: 'home')` and names interpolated in Twig strings, such as `"#{path('home')}"`, give nothing. Found in the step 6b review.
- **Route files are read more widely than Symfony loads them.** Nested directories and `.yml` files below `config/routes` count, a symlink loop there repeats targets, and updates match the `.yaml` extension case-sensitively while reading does not. Found in the step 6b review.
- **Some step 6b behaviour has no test.** `Symfony::update` is untested, and the Twig guard tests pass without the guards; a Twig fixture holding `<?php ... ?>` would make the hover test meaningful. Found in the step 6b review.
- **Routes from bundles are not read.** Routes that bundles define, such as `_wdt` or `_security_logout`, have no target. Found in step 6b.
- **Templates from bundles and bridges are not found.** Bundle namespaces such as `@Twig` or `@WebProfiler`, their overrides in `templates/bundles`, and TwigBridge's form themes such as `form_div_layout.html.twig` give nothing. Found in step 6c.
- **Template names in YAML are not read.** `form_themes` in `twig.yaml` and other configuration that names templates give nothing. Found in step 6c.
- **Some template names are not read.** Named arguments such as `render(view: 'x')`, a class-level `#[Template]`, `{{ include(['a', 'b']) }}`, `{% extends cond ? 'a' : 'b' %}`, `{% extends('a') %}`, names Twig normalizes such as `admin/../base.html.twig`, tags closed with `~%}`, `TemplatedEmail::htmlTemplate()`, `block()` with a template, `{% form_theme %}` and `config/packages/<env>/twig.yaml` give nothing. Found in the step 6c review.
- **Some step 6c behaviour has no test.** `renderBlock()`, `renderBlockView()`, `stream()`, `Environment::display()` and `load()`, list-form and relative `paths`, a `default_path` override, and a Twig file of a nested Drupal project are untested. Found in the step 6c review.

- **Some message flows are not followed.** Messages wrapped in `new Envelope(...)`, messages the Scheduler sends with `RecurringMessage`, handlers configured in YAML or service tags, handler methods a handler class inherits or takes from a trait, `handles: '*'` and namespace wildcards, `HandleTrait::handle()`, and named arguments such as `dispatch(message: new X())` are not read. Found in step 6d.
- **Handlers of another bus are listed.** The `bus` and `fromTransport` options of `#[AsMessageHandler]` are not read, so a dispatch lists the handlers of every bus. Found in step 6d.
- **An interface-typed message lists only that interface's handlers.** `dispatch($notify)` with `Notify $notify` misses the handlers of implementing classes, and those handlers do not list that dispatch. Found in the step 6d review.
- **Every method-name definition pays for the dispatch check.** On a project with an active framework, `call_at` parses the file again for every definition on a method name; the Drupal coverage sweep went from about 11.5 to 13.6 s. Check the method name against the names frameworks answer before inference. Found in the step 6d review.
- **Handlers under a symlinked root do not list their dispatches.** `handled_by` compares the request path with canonical handler paths. Found in the step 6d review.
- **Some step 6d behaviour has no test.** A `dispatch()` on something other than a message bus, and multi-line and null-safe dispatch calls, are untested. Found in the step 6d review.
- **A bare class-level `#[AsMessageHandler]` cannot lead to dispatches.** The attribute is only its name, which opens the attribute class; the handler method does lead to them. Found in step 6d.

- **Some configuration is not read.** Services and parameters defined by bundles, in XML or PHP configuration, in `config/packages/*.yml` or `config/packages/<env>/`, and parameters set from PHP code give nothing; `_instanceof` and `bind` are not followed. Found in step 6e.
- **`@id` on an alias opens the alias, not the service it points to.** Following aliases is one more click away. Found in step 6e.
- **Some service and parameter forms give nothing.** `@!id` and `@>id` references, `#[Autowire('@id')]` and the positional `#[Autowire(null, 'id')]`, files loaded through `imports:`, and a `%name%` after a lone `%` in the same string (`'50% of %app.name%'`) give nothing. Found in the step 6e review.
- **Some step 6e behaviour has no test.** Another argument of `#[Template]` giving nothing, and aliased or fully qualified `#[Autowire]`, are untested. Found in the step 6e review.

## Accepted for now

- **Implementations miss project classes below a built-in class.** On `$c->count()` with `$c` a `\Countable`, a project `Bag extends \ArrayObject` is missing, because subtypes come from the project table only.
- **Property types given only by `LanguageLevelTypeAware` are not read.** Summaries keep attributes on functions and methods only, so properties such as `Exception::$file` lose that type. Most such properties also carry `@var`.
- **Stub files are read again after every update.** Property-assignment inference on a class that extends a built-in class re-reads the stub file once the per-update cache is cleared. The files are at most 300 KB.
- **Stub duplicates keep the first file in path order.** `ds` wins over `ds_v2` and `couchbase` over `couchbase_v2`, so users of Ds 2.x or the Couchbase 2 SDK see the other version.
- **Members of built-in classes are never certain for diagnostics.** A typo such as `$date->whatever()` on a `DateTime` stays unflagged, because the stubs give one PHP version and the project may run another.
- **`from()` and `tryFrom()` on a pure enum are not flagged.** Diagnostics count `cases`, `from` and `tryFrom` on every enum. Navigation gives no target for them on a pure enum.
- **Typed class constants get no type.** `const Severity DEFAULT = Severity::Low` gives nothing after `Severity::DEFAULT->`, because only enum cases get a type.
- **Hover on `->value` shows `int|string`.** The type comes from the `BackedEnum` stub, not from the enum's backing type.
- **Arrays built by assignment have no element type.** `$teams[$id] = $team` in a loop gives `$teams` no element type, as the design takes element types only from declarations.
- **Template bounds are not used.** A template with no argument gives nothing, even with `@template T of Item`.
- **Projects are fixed when the index is built.** A `composer.lock` added later takes effect only after a rebuild or a restart.
