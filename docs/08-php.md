# PHP, composer, Drupal and the stub set

> Part of the record §8 of `CLAUDE.md` began. Each note is a bug that was paid for once; the point
> of writing it down is that the _general_ shape at the end of each one recurs. Where a note says
> "§8", it means the record as a whole — `CLAUDE.md` §8 plus this family of documents. **Read this
> file before working in the area it names.** Add new notes here rather than to `CLAUDE.md`, and
> keep them in the shape the existing ones have: what looked true, what was true, what the tell was,
> and what generalises.

Facts about the language and the ecosystem that no amount of care in Rust recovers: what composer's
dumped files contain, what Drupal registers where, how a docblock is delimited, and what the
project's own coding standard does to the formatter's output.

**The notes, by their tell:**

- Never `read_to_string` a composer-generated file.
- Composer autoload paths are anchored per-token.
- Drupal module namespaces are not in composer at all.
- Drupal's own documentation contains a working example of the syntax you are scanning for.
- A docblock's prose is the run before the FIRST tag, and a multi-line conditional type is
  indistinguishable from prose to anything that scans the whole block.
- PHP comments do not nest, so a `*/` anywhere in generated docblock text closes the docblock and
  turns the rest of it into code.
- Documenting _methods_ needed two scanner rules the function pass never exercised, and both failed
  silently in the safe direction.
- Lifting one part of a description into the tag stream breaks the parts after it.
- `Drupal.Arrays.Array.ArrayIndentation` measures from the first token of the line the `[` is on, so
  an _indented_ operator continuation inside an array can never satisfy it.
- The container parameters a project reads are not in the YAML, and that is a _source_ problem
  rather than a reader problem.
- A callable written as a two-element array is a call, and one of its two spellings is also plain
  data.

---

- **Never `read_to_string` a composer-generated file.** `vendor/composer/autoload_classmap.php` can
  contain non-UTF-8 bytes (a Latin-1 `©` in a comment). `read_to_string` returns `Err`, a
  `let Ok(..) else` guard silently drops the **entire** classmap, and root-level classes like
  `\Drupal` become unresolvable. Read bytes + `from_utf8_lossy`; the parsers only need ASCII.

- **Composer autoload paths are anchored per-token.** `$baseDir` and `$vendorDir` must be resolved
  by the **nearest preceding** token, not by a `/vendor/` prefix heuristic. Drupal core registers
  `Drupal\Core\ => $baseDir . '/web/core/lib/Drupal/Core'`, which is _not_ under `vendor/`. Getting
  this wrong silently kills all of Drupal core resolution.

- **Drupal module namespaces are not in composer at all.** `Drupal\node\`, `Drupal\field\` etc. are
  registered at runtime from `*.info.yml`. They must come from a framework provider feeding generic
  autoload rules — with no Drupal name anywhere in the generic layer.

- **Drupal's own documentation contains a working example of the syntax you are scanning for.**
  Reading annotation-declared entity types by scanning text for `id = "..."` looked equivalent to
  asking mago for the tag, and is not: `core.api.php` teaches the annotation syntax with a real
  `id = "comment"` inside an `@code` block. A text scan reads that as a second class claiming
  `comment`, and a catalog refuses to rank two claimants -- so the id **stops** resolving, and the
  feature that was supposed to add four ids silently removes one that worked. mago folds `@code`
  into a single tag. Whenever a scan is about to run over documentation, remember that documentation
  is where the syntax is demonstrated.

- **A docblock's prose is the run before the FIRST tag, and a multi-line conditional type is
  indistinguishable from prose to anything that scans the whole block.**
  `@return ($array is list<V>` continues on lines opening with `?` and `:`, which carry no `@` -- so
  a script placing summaries read them as an existing description and skipped the function. Nine
  were skipped, `array_map` (178 call sites, eighth most-called in the project) and `array_splice`
  among them, and the skip logic read as _safety_. Same shape twice in one piece of work, because
  the Rust tripwire test written to catch a re-vendor had the identical predicate and would have
  over-counted. Related and cheaper to get wrong the other way: a scanner telling a top-level
  function from a method must test the column of the `function` **keyword**, not of the docblock
  above it -- indentation strips to empty either way, so every method landed in the function pool
  and `sort` came back documented as `Collator::sort`. The tell was reading the answer's _text_, not
  comparing counts: "Sort array using specified collator" is not a thing `sort` does.

- **PHP comments do not nest, so a `*/` anywhere in generated docblock text closes the docblock and
  turns the rest of it into code.** Two agent-written `@code` examples carried `/* ... */`, and that
  one construct cost **24 `syntax` findings** across the stub set -- the remaining tags of each
  docblock, `@return` included, became statements. Nothing in the writing pipeline noticed: the JSON
  was valid, the applier placed it, the test suite stayed green, and hover on the two functions
  still rendered because the prose came before the break. The only instrument that saw it was
  `phrost check third_party/php-prelude`, which is §8's note about the stub set being a corpus,
  arriving from a direction I did not expect -- a corpus is also a _canary_ for text this project
  writes into it, not only for defects it was vendored with. Whenever text from anywhere is inserted
  into a comment, the delimiter is the thing to refuse before anything else; the placing script now
  does, and 3 515 lines of prose otherwise cost the index **0 bytes**, because a docblock is read on
  demand from a range in a file rather than held.

- **Documenting _methods_ needed two scanner rules the function pass never exercised, and both
  failed silently in the safe direction.** The anchor: several stub files wrap everything in a
  braced `namespace { ... }` block, which indents every class inside it -- a class scanner anchored
  at column zero found no class in `dom.php` at all and left 26 `DOMElement` methods unplaced. What
  made it survivable is that **no** stub file mixes the two forms, so the owner was `None` rather
  than stale and nothing was written to the wrong class; a file that did mix them would have
  attached DOM descriptions to whatever the last column-zero class was. The key: a method has to be
  `Class::method`, because `count`, `current`, `next`, `key` and `offsetGet` are declared by dozens
  of classes -- keying by bare name collapsed 1 253 methods into 648 distinct names, which read as
  the work having half failed, and it is also what caught **six invented methods**
  (`SplDoublyLinkedList::enqueue`, `IteratorAggregate::current`, `PDORow::openBlob`) that a
  bare-name match would have placed on whichever class happened to declare something of that name.
  The general shape: when a pass moves from a flat namespace to a nested one, the scanner's _anchor_
  and its _key_ both change, and neither change announces itself.

- **Lifting one part of a description into the tag stream breaks the parts after it.** `@code` was
  made a tag so its line breaks would survive, and that worked for the docblock shape I tested --
  prose, then an example, then the tags. Drupal's real shape is prose, example, _more prose_,
  another example, and there the sentence after `@endcode` continued the `code` tag, because a
  non-tag line continues the last tag and the tag was still the last one. It rendered glued to the
  closing `];` and inside the fence. The tell is that the bug needs **two** of the construct to
  appear at all, so one probe of one example says the feature works. A synthetic tag for "prose
  between tags" was the obvious repair and is wrong: four places read `Docblock::tags` and each
  would have to remember to skip it. The description is markdown that this layer passes through
  unescaped, so the fence belongs _in the description_ -- no tag, no consumer changes, and the
  example stays where its author put it rather than moving to the end with the tags. Only in the
  description: a `@code` written after the tags have started still has nowhere else to go.

- **`Drupal.Arrays.Array.ArrayIndentation` measures from the first token of the line the `[` is on,
  so an _indented_ operator continuation inside an array can never satisfy it.** mago breaks before
  an operator whose array operand spans lines -- `$options\n  + [` -- and there is no setting for
  it: `should_inline_binary_rhs_expression` hugs an array operand for `??`, `||` and the comparisons
  and not for arithmetic, which is the only kind an array reaches. So a defaults merge, which 105
  files of Drupal core 11 write as `X + [` at the end of a line, came back from format-on-save as a
  phpcs error. Respelled in `format/drupal.rs` by reading spans off a re-parse of the formatter's
  output; two kinds of line in the region that comes back are _not_ layout and need opposite
  treatment, which the lexer's string ranges decide -- a string written over several lines keeps its
  own bytes because they are the value, and a heredoc moves whole, body and closing marker together,
  because PHP reads its body relative to that marker. What the measurement also found: the same
  formatter was **not a fixpoint** on 26 of the reference project's 1 669 files, on no line the join
  touches. `preserve_breaking_condition_expression` reads the breaks the _first_ pass introduced as
  author intent, and the second pass then put every operand of the condition on its own line at one
  indentation, losing the nesting. Iterating `format` to a fixpoint was measured (all 1 669 settle
  within two passes) and refused, because the second pass is the _worse_ output; turning the flag
  off collapses the hand-broken conditions core is full of. **What fixed it was the width**, and
  that is the note to keep: Drupal's own sniffs _error_ on a break inside an array and only _warn_
  above 80 columns, so at PER-CS's inherited 120 the formatter was choosing the error to avoid the
  warning. Applying `ArrayIndentation`'s own rule to the formatter's output over 1 673 files: 36
  violations introduced at 120, 6 at 160, 1 at 200 and 1 at every width past it -- and the
  instability went with them, 25 files to 1, because a break that is never introduced is never
  re-read as intent. Before respelling a shape the formatter produces, price the width. What settled
  the rest was **running the standard rather than reasoning about it** -- see
  `tools/drupal-style.sh` in §9, which measured 15 introduced errors and now measures 1 (a phpcs bug
  on `match` inside `switch`, which core contradicts and the project excludes). Two rules about
  _where_ a respelling goes came out of it and are the transferable half. A rule the formatter would
  undo has to run **after** it, every time: the one-line `match` break was first written as a hint
  -- a newline inside the arm, which is the fact mago reads -- and the next pass reprinted the arm
  on one line and read that as inline again. And a rule that changes the **breaks** the formatter
  reads has to be followed by reprinting until the text settles, because several settings read a
  break as author intent; with that loop the files that are not a fixpoint went 25 -> 0.

- **The container parameters a project reads are not in the YAML, and that is a _source_ problem
  rather than a reader problem.** `getParameter()` declares
  `array|bool|string|int|float|\UnitEnum|null`, so every one handed to a typed parameter reads as
  possibly-null; the shape is in the configuration, and `services::parameters_in` now reads it as a
  third kind of catalog value beside a class and a place. It answers **zero** of the reference
  project's own calls: both are `serializer.formats`, which Drupal's `serialization` module sets
  from a **compiler pass** over tagged services, as `ListCacheBinsPass` sets others. Sixteen
  parameters are declared in YAML across that checkout and its own code reads none of them. Same
  shape as `Drupal\node\` not being in composer -- the configuration a framework _ships_ and the
  configuration it _computes_ are two sources and only the first is a file -- so before reading a
  file for a fact, check that the fact is in a file at all. What the reader does buy is any declared
  parameter, and the fixture pair says so; what it cannot buy needs the compiler passes.

- **A callable written as a two-element array is a call, and one of its two spellings is also plain
  data.** `[$this, 'refresh']` is how PHP wrote a first-class callable before `(...)`, and it is
  still how a framework is handed one -- a Twig filter, an event subscriber, a `usort` comparator.
  Left as an array literal the method name is a _string_: `references` on the method found only its
  own declaration, `rename` rewrote the declaration and left the string spelling a method that no
  longer existed, and goto on it went nowhere. 245 sites written with `$this` and 9 with
  `Foo::class` on one real checkout, none of them reachable by any of the three. It needs no new
  shape -- `ExprData::CallableMember` is what PHP 8.1's `(...)` already lowers to and says exactly
  the same thing, and every reader of a member site has understood it since it existed. What took
  the measuring is telling the construct from an array that looks like one. `$this` beside a string
  has one reading. `[Foo::class, 'bar']` looks equally unmistakable and is **how a list of class
  names is written** when one is spelled with `::class` and the next is not: Symfony's
  `setAllowedTypes('active_at', [\DateTimeInterface::class, 'null'])`, its
  `in_array($name, [TranslatableMessage::class, 'TranslatableMessage'])`, its
  `'required_badges' => [CsrfTokenBadge::class, 'RememberMeBadge']`. Read as callables those were
  **17 undefined methods on Symfony and 10 on Laravel**, every one on a class that never claimed one
  -- and the counts alone read like a feature finding real mistakes. Reading the findings is what
  settled it, which is §8.2's rule about an instrument that flatters. The fix is not to drop that
  form but to mark it: `presumed` says the syntax only _suggests_ a member here, and the view keeps
  such a site only where it resolves. That works because the readers split cleanly -- references and
  rename match what a name **resolves to**, so a list of class names contributes nothing to them,
  and the check that fires precisely _because_ nothing resolved is the only one a false site could
  make lie. Applying the test once, where the site is built, is what keeps it from being a flag
  every reader has to remember. Final tally across five corpora: **two false positives gone and one
  finding added**. The two were `Cannot call method unresolvable() on string` in Laravel's container
  tests, where `bindMethod([Stub::class, 'unresolvable'], fn ($stub) => ...)` had the callback's
  parameter typed from the _elements_ of what looked like an array of strings -- the pair stops
  being an array, and the nonsense stops with it. The one added is `Seeder::run`, already reported
  one line below at the `$this->run(...)` spelling of the same claim.

- **The vendored stub set describes its exceptions in prose and declares almost none of them.** 428
  `@throws` against 8 258 functions, where the phpstorm-stubs set bundled inside intelephense has 2
  244 -- and `date.php` had exactly one, on `DateTimeZone::__construct`, while
  `DateTime::__construct` beside it had none. The information was mostly _written_: 49 docblocks say
  "Throws X if ..." in the summary and carry no tag, which is a form nothing can read. Adding the
  tag where the set's own prose already names the class is therefore sourced rather than invented,
  and is recorded in that README under "Local edits" like the `RedisCluster` declaration before it
  -- a local edit is silently reverted by a re-vendor. `DateTime::__construct` is the one addition
  the prose did not source, and it is a genuine asymmetry rather than a judgement:
  `DateTimeImmutable::__construct` two hundred lines down documents the same
  `DateMalformedStringException` for the same argument. **The trap is `@code`.** A first pass read
  `throw new ErrorException($message, 0, E_WARNING, ...)` out of the example in
  `ErrorException::__construct`'s own docblock and declared the constructor as throwing itself.
  Exactly what `Lowerer::doc_is_ignored` already records for `@ignore` -- a docblock is where
  somebody demonstrates a tag -- met one tag over, and the fix is the same: cut the
  `@code`/`@endcode` span out before reading anything. Worth knowing what this buys and does not: on
  the reference project the 49 tags are +29 findings, of which 22 are `AssertionError` from
  `assert()` and 8 are the `DateMalformedStringException` that started it. The standard library is
  not where a Drupal project's throws live -- its own modules carry 2 171 tags, core 1 113 and
  vendor 6 185.

## The stubs declare `DateInterval`'s properties `readonly`, and PHP does not

`$interval->invert = 1;` is how a negative interval is built by hand, and Symfony writes it in eight
files of its own -- a transformer, two normalizers and their tests. mago's prelude declares all
eleven of `DateInterval`'s properties `public readonly`, so the `readonly-write` check reported
every one of them on code that runs.

Unmarked locally and recorded in `third_party/php-prelude/README.md`, since a re-vendor reverts it.
The general point is the one that section already makes about the 46 other defects in that set: a
stub is a _claim_ about the standard library, and a new check reads claims nobody has had reason to
verify until now. Running `check` over a corpus after adding one is how the wrong claims surface.

## Forty declarations in the stub set carry two docblocks, and the first one is thrown away

```php
/** @return DOMNode|false */
/**
 * Adds node as the last child of this node and returns it. ...
 */
public function appendChild(DOMNode $node) {}
```

PHP attaches the nearer block and so does every other tool, so the tags in the first one reach
nothing. `DOMNode::appendChild()` has no native return type either, which left it **completely
untyped** -- and with it every chain built through it, which is most of how anyone builds a
document. Thirty-three of the forty are `dom.php`, three each `date.php` and `xmlreader.php`, one
`spl.php`.

The tell was a `compare-lsp` run: PHPStan reports `Cannot call method removeChild() on DOMNode|null`
on the reference project and phrost said nothing, on a method whose stub plainly declares the union.
Nothing about the reading was wrong -- reading the second block is what PHP does -- so the defect is
the generator's, and the merge is a local edit with a tripwire counting them back to zero.

Beside it, four DOM methods that **return the node they were given** and declare `DOMNode|false`,
which is true and useless: `$parent->appendChild($doc->createElement('p'))->setAttribute(...)` then
reads as a `setAttribute` on a bare `DOMNode`, reported 29 times on Symfony. `appendChild`,
`insertBefore`, `removeChild` and `replaceChild` now carry `@template TNode of DOMNode`, which is
what the manual says and what PHPStan's own stubs declare.

**Measured.** Symfony coverage 318 768 -> **319 030** resolved receivers and 11 067 -> **10 805**
untyped; the reference project, WordPress, Laravel and Slim unmoved. What Symfony gains in findings
is 65 `possibly-false-receiver` and 24 `possibly-false-argument` on DOM code, every one a `|false`
PHP really can return and PHPStan reports too.

And the relief that makes it liveable, which is a rule and not a suppression: `createElement('p')`
cannot fail, because the failure branch is what the method answers for a tag name XML would not
accept. `ruled_out_failures` reads the literal the way `literal_returns` reads a date -- **166 of
the 236 new DOM reports were exactly that shape** -- and a tag name read from a variable keeps the
union.

## A Twig template calls a method and no PHP file names it

`unused-declaration`'s own doc lists three sources of _reached_ and says "leaving any of them out
turns this into a report nobody trusts twice". There were four. A Drupal project renders
`{{ trade.status.getDescription }}` and nothing in PHP writes `getDescription` anywhere, so the
method read as reached from nowhere -- **25 of the reference project's 124 findings**, every one an
accessor a template renders.

Twig's shorthand is the half that makes this more than a name search. `{{ node.title }}` is
`$node->title`, `$node->title()`, `$node->getTitle()`, `$node->isTitle()` or `$node->hasTitle()`,
whichever the object has, so the name a template writes is frequently not the name the method
carries. The getter spellings are recorded beside every word.

**Words rather than parsed calls**, which is coarse and is the direction this has to be coarse in:
the set is only ever consulted to _withhold_ a finding, exactly as the member half of the rule
already is -- "one `->run()` anywhere keeps every `run` alive". What it costs is stated rather than
hidden: `variables` written in a theme's `menu--toolbar.html.twig` is Twig's own context variable
and keeps `Report::getVariables` alive too. A missed finding, which is what a report about absence
has to prefer.

`.twig` is a _language_ and not a framework, the way `.php` is, so it is read wherever it is found
rather than behind a provider. Folded into the discovery walk that already happens -- one more
extension in a list beside `yml` -- and installed beside the configuration sites, since both are
read from files the index never holds and neither is in a stored skeleton. Measured: discovery 346
ms and indexing 951 ms on 28 851 files, the same band as before; `check --jobs 1` 1.19 s.

## `defined()` is the third existence test, and the only one that names a class constant

`function_exists`, `class_exists` and their siblings were read; `defined` was not. Symfony's
`AbstractUnicodeString` writes

```php
if (!$compat || !\defined('Normalizer::NFKC_CF')) { ... }
$str->string = normalizer_normalize($str->string, \Normalizer::NFKC_CF);
```

and phrost reported the second line. It was **right** about the constant -- the checkout installs
`symfony/polyfill-intl-normalizer`, whose global `Normalizer` stub outranks the bundled one and does
not declare `NFKC_CF`, which is exactly what the guard is about -- and wrong to say so, because the
file had already said it.

`defined` is the one test whose string holds _both halves_ of the name, so the class is compared
too: a guard about another class licenses nothing, which is the same care `method_exists` takes by
requiring its subject to be the receiver. Per file rather than per branch, the trade
`existence_tests` already states.

Found while grouping Symfony's 27 `undefined-constant` findings, which also turned up a stub of our
own left half-finished: `RedisCluster` was added here as a local edit without its four `FAILOVER_*`
constants, and `RedisTrait` writes all four in one `match`. Symfony **27 -> 18**, Laravel 2 -> 1,
and nothing else moved. The rest of that group is real: `LazyObjectState::STATUS_*` is referenced by
`SymfonyCaster` and declared nowhere in the monorepo, and the `ProjectServiceContainer` constants
belong to a container generated at runtime.

## `@global` is the one binding PHP has no syntax to type, and it was unread

`global $wpdb;` names a variable the whole program shares and says nothing about what it holds.
phpDocumentor's tag for exactly that is `@global wpdb $wpdb`, and WordPress writes it **599 times**
directly above such a statement -- because `$wpdb`, `$post` and `$comment` are how that codebase is
written. They are the three commonest untyped receivers in the entire checkout: `$wpdb->prepare`
alone is 333 accesses, `$post->ID` is 385.

mago's docblock parser has no `@global`. The tag is phpDocumentor's rather than one of the
static-analysis vocabularies it models, so it arrived as text and nothing read it -- while the
`@var wpdb $wpdb` that says the identical thing in the identical place worked.

Rewritten into the `@var` it means, and **padded to the same length rather than replaced**: every
range downstream is a byte offset into the docblock's text, so a name reference in the type gets its
goto range from it and `undefined-type` reports at it. `@global` is seven bytes and `@var` four, so
the three the substitution gives back are spaces, which the tag grammar already allows. Only where a
separator follows, so prose saying `@globally` is left alone.

**Measured on WordPress: resolved receivers 51 348 -> 53 510** (84.8 % -> 88.3 %) and untyped 7 259
-> **5 070** (12.0 % -> 8.4 %) -- the largest single coverage move of the session, and the corpus
that needed it most. Typed assignments 87.2 % -> 87.9 %.

What it costs is 89 new findings there, every one read and every one sound: `wpdb::prepare()` is
declared `string|null` and `WP_User_Search::$query_limit` is a `string`; `$wpdb->get_col()` gives
`array<int, string|null>` and `wxr_authors_list()` takes `int[]`. WordPress's own annotations say so
and the code does not check -- the precision gain arriving as reports, which §8.2 records as the
feature working. Nothing moved on the other four corpora, and the reference project writes no
`@global` at all, so its `check` pays one extra `contains` on a docblock that already held `var`.

## A docblock the coding standard put a comment under

```php
/**
 * The entity for this result.
 *
 * @var \Drupal\Core\Entity\EntityInterface
 */
// phpcs:ignore Drupal.Classes.PropertyDeclaration, Drupal.NamingConventions.ValidVariableName
public $_entity = NULL;
```

That is `Drupal\views\ResultRow`, and the comment is there **because** the property is documented:
the coding standard objects to the leading underscore and the author silenced it. The gap test was
`gap.chars().all(char::is_whitespace)`, so the comment read as a separator, the `@var` reached
nothing, and every `$row->_entity` in a views field plugin had no type — with everything through it
dead behind that.

PHP's own rule is the other way round: the compiler keeps the last doc comment it saw and an
ordinary comment does not clear it, which is why `getDocComment()` answers across one and why
PhpStorm, PHPStan and Psalm all read it. **215 properties in 82 files** of one real checkout are
written this way, core and contrib together, because `phpcs:ignore` is how Drupal silences a sniff.

Two things the gap test now keeps saying no to. An attribute: `#[` is not a comment, and where a
declaration's own range does not already cover its attributes the answer stays the one it has always
been. And a block comment, left out deliberately rather than by oversight — it is the same argument
and no measured case, and a rule with no case to exercise it is the kind that drifts.

Methods were never affected, which is worth knowing because it is what made the bug look
unreproducible: the same `// phpcs:ignore` above a method keeps its `@return`. Found the other way
round — `compare-lsp` against intelephense named `$row->_entity` as a site where only they answer,
and it was one of three left on the reference project.

Measured: reference project resolved receivers 49 715 -> 49 722 and untyped receivers 359 -> 352,
psl `unused-import` 67 -> 65 (an import a newly-read `@var` now uses). Nothing rose on five corpora,
index memory unchanged at 103.7 MB, 17/17 budgets.

- **A get-only property hook is read-only only when it is _virtual_, and PHP 8.5.10 is on the
  machine to ask.** The rule as first written -- hooks with a `get` and no `set` refuse every write
  -- is what the RFC reads like, and it reported four writes in Symfony that all run. The first is
  `ViewEvent::$controllerArgumentsEvent`, a deprecation shim that caches into its own get-only
  property from the constructor and again inside the hook. What PHP actually decides on is _backed
  or virtual_, and that is a **syntactic** test on the hooks' own text: a property is backed if any
  hook uses `$this->propName`, and a backed property holds a slot that takes a write like any other.
  Only a virtual `get` with no `set` gives `Error: Property C::$name is read-only` -- from inside
  the declaring class as much as from outside, which is the one write rule in PHP with no scope in
  it. Three more things the oracle settled that reasoning would have got wrong. A **`set`-only**
  hook still leaves the property readable, because it is backed. An **abstract** hook (`{ get; }` in
  an interface or an abstract class) says nothing about backing at all -- it is a requirement, and
  whoever implements it decides -- so it cannot carry the rule. And a **subclass may override a
  hooked property and add a `set`**, after which a write through the parent's type runs; that is
  what the by-name gate in the index is for, narrowed to declarations that could be the override (a
  class-like that extends something, or a trait) so that a common name does not switch the rule off
  everywhere. The general point is the one §8.2 keeps making about instruments: `php -v` said
  8.5.10, and four `php` one-liners settled in two minutes what a week of argument from the RFC
  would not have. For a language rule, run the language.

- **A static property cannot be `readonly` and _can_ be `private(set)`, and a comment that said
  otherwise hid the second half for as long as it existed.** `refused_writes` returned early for
  every static target on the grounds that "a static property is never `readonly` -- PHP forbids the
  combination -- and carries no set visibility of its own". The first clause is right, and PHP says
  so outright: `Static property R::$x cannot be readonly`. The second is wrong. PHP 8.4 allows
  `public static private(set) int $n` and enforces it, and the table is exactly the one instance
  properties follow -- asked of 8.5.10: the declaring class may write a `private(set)` static, a
  subclass gets `Cannot modify private(set) property Tally::$n from scope Kid`, a subclass may write
  a `protected(set)` one, and nothing outside may write either. Two lessons rather than one. A
  comment that states two facts and is checked for one is a comment that will be believed for both.
  And the way it was found was writing the _syntax_ down and asking `php -l` whether each line is
  legal at all -- of six PHP 8.5 constructs probed that way, two were not what the RFCs read like:
  closures in constant expressions are refused (`Constant expression contains invalid operations`),
  and `public static private(set)` is allowed.

- **`#[\Override]` counts four things as "something above", and two of them are not what the RFC
  reads like.** PHP 8.3's attribute is a _fatal at class load_ --
  `C::m() has #[\Override] attribute, but no matching parent method exists` -- so it is the same
  kind of claim as `bad-override` and answerable in the same place. What satisfies it, asked of
  8.5.10 one line at a time rather than reasoned about: a parent class's method, transitively, and
  **not** if it is `private`; an interface's method, transitively; an **abstract** method in a used
  **trait** -- while a _concrete_ one in a used trait is the fatal, because the class's own method
  silently replaces it and there is no relation to override; and `__toString`, always, because PHP 8
  makes every class that declares it implement `Stringable` whether it says so or not. A
  **constructor** never counts, whatever the parent declares. The last two are what a first
  implementation gets wrong, and the corpora said so immediately: psl reported four, all false --
  two `__toString` and two `twist()` fulfilling an abstract trait method. After the rules were run
  rather than argued: **3 541 uses across psl, Symfony and Laravel, nothing reported.** A check on a
  construct that popular is worth building precisely because a zero on it is evidence. A
  `#[\Override]` written _inside_ a trait is not judged at all: whether it has something above
  depends on the class using the trait, and the declaration alone cannot say.

- **Twenty of PHP's load-time fatals, asked one line at a time, and phrost was watching for three.**
  Written as a table of one-liners and run: seventeen of the twenty are fatals, and the two that are
  not are worth knowing -- a **duplicate backed enum value** loads fine (only a static analyser
  objects), and `abstract` beside `static` on one method is legal. Of the seventeen, phrost already
  reported the abstract-method-in-a-concrete-class and the two instantiations; the rest were silent.
  Two were then built, chosen by how easy they are to write _by accident across files_: a class
  extending a `final` one, and a name a class declares twice. What the corpora said about the second
  is the note worth keeping. Its first run reported 20 on WordPress, Laravel and Symfony, and
  **every one was a `@property` or `@method` tag beside a real declaration** -- `Iri` documents
  `@property string $scheme` above `protected $scheme = null;` eleven times, which is how a class
  says what its magic accessor answers. A docblock member is not a declaration and PHP never sees
  one; `MemberRow::from_docblock` is the field that says so. After the filter: 0 on all five
  projects, and **9 on the vendored stubs** -- an extension's _overloads_ written out as PHP, which
  PHP itself refuses (`Parle\RLexer::push` three times, `Imagick::count` twice). Recorded in that
  set's README with the rest. The general point is about where to look for a check's false
  positives: a mistake that stops a file loading has a population of zero in code that ships, so a
  corpus cannot tell you the check is _useful_ -- only that it is _quiet_. The stub set, which is
  declaration-only and was written by a generator, is the one corpus where these do occur.

- **Every one of PHP's four "no context" errors is decided at _bind_ time inside a closure, and the
  error messages say nothing about that.** `Using $this when not in object context`,
  `Cannot use "self" when no class scope is active` and their two neighbours read like compile-time
  rules and are not. Run against 8.5.10 one line at a time: `function () { return $this; }` written
  in a **static** method compiles; `static fn () => $this` compiles; `fn () => parent::class` in a
  class that extends nothing compiles; a `self::` closure at the top level compiles. `Closure::bind`
  settles the scope and the object afterwards, and Symfony has **79 `$this` and 24 `parent::`** of
  exactly that shape that all run. A check that walks into closures reports every one of them. Three
  more that the first spelling got wrong, each found by running it over a corpus rather than by
  thinking: a **trait**'s `parent::` is resolved against the _using_ class, so a trait can never be
  judged (`MicroKernelTrait`, 22); a `.php` file's **top level** is an object context whenever it is
  `include`d from a method, which is what every PHP template is (`error.html.php` opens with
  `<?= $this->charset ?>`); and `self::` in a class **constant's initialiser** is inside the class
  though no body holds it, which is why the class test has to be over _ranges_ rather than over the
  walk -- Symfony writes 628. What survives is small and certain: a name written directly in a body
  nothing can rebind. 0 on all five projects and on the stubs, where the first spelling
  reported 913.

- **PHP's subscript and class-name rules split by _kind_, and the split is not where a reader would
  put it.** Run as a table: subscripting an `int`, `float`, `bool` or `null` is a **Warning** and
  yields `null` -- `Trying to access array offset on int` -- while subscripting a **string** with a
  non-numeric literal is a **TypeError** that stops the line. A numeric string is a position, so
  `$s['1']` and `$s[1]` both read a character and only a _literal_ key can be judged: a key written
  as a variable may be either at runtime. And `$s[1.0]`, `$s[true]` and `$s[null]` are warnings
  again (`String offset cast occurred`), not errors. The same shape decides a class reached through
  a value. `$x::CONST` and `new $x` take an object **or the class's name as a string** --
  `$c = 'DateTime'; $c::createFromFormat(...)` runs -- so the error names both:
  `Class name must be a valid object or a string`. What that leaves out matters: a `callable` is a
  string, an object _or_ an array depending on the value, and Symfony reaches `$callable::class`
  three times. An `iterable` is an array or a `Traversable`. Neither can be judged, and a first
  spelling that wrote the rule as "not an object or a string" reported all three. The lesson is the
  one `holds_no_elements` already carries and this repeated: with `BuiltinTy` closed by convention
  rather than by the type system, **name the refused kinds**, never negate the accepted ones -- a
  variant added later must not fall into a refused list by default.

- **A generator's `return` is not the function's, and an arrow function's is not written at all.**
  Four compile-time fatals about a declared return type -- a value from `: void`, a bare `return;`
  under any type, any `return` in a `: never`, and a `yield` the type cannot be -- and three of the
  four needed a corpus to find the boundary. In a **generator** a bare `return;` ends the iteration
  and `return 5;` is what `getReturn()` hands back, so neither return rule applies however the
  function is typed. Symfony writes 56 of those and psl 23, all `yield ...; return;` inside an
  `: iterable`. An **arrow function**'s `return` is synthesized by lowering, so what looks like a
  return is the expression: psl writes `static fn (Throwable $t): never => throw $t` sixteen times
  and PHP accepts it, because a `throw` never returns. `fn (): void => 1` stays reported, since an
  arrow's body always has a value and PHP refuses that one whatever the value is. And the **yield**
  rule is narrower than the message suggests.
  `Generator return type must be a supertype of Generator` refuses `void`, `never` and the scalars
  -- but `object` and `mixed` both accept a generator, and so does every iterable-shaped class name.
  `array` is the one in the refused list a reader expects to work.

- **mago parses `$x |> fn ($v) => f($v)` and PHP refuses the file, and its own formatter writing the
  parentheses in is the tell.** PHP 8.5 binds an arrow function's body across the pipe, which is
  never what the writer meant, so rather than guess it refuses to compile:
  `Arrow functions on the right hand side of |> must be parenthesized`. mago binds it the way a
  reader expects and says nothing -- and `phrost reprint` emits `1 |> (fn($x) => $x + 1)`, which is
  the same reading stated out loud. So the parser and the language disagree, and a green editor sat
  over a dead file. `phrost_syntax`'s `refused` is where that now lives: shapes mago accepts and PHP
  does not, reported as the syntax errors they are. Each rule is gated on a substring the shape
  cannot be written without -- `|>` here -- so a file that cannot hold it costs one scan of its text
  and no walk, which is what makes the rule affordable on the indexing path. Asked of 8.5.10 rather
  than read, and the matrix matters: a **closure** on the right needs no parentheses because it has
  a brace to end on, and a first-class callable, a callable string, a variable, an array callable
  and a parenthesised conditional all parse. Only the arrow function, with or without `static`. The
  one neighbouring refusal, `new E(...)`, is about first-class callable syntax and not about the
  pipe at all. Found while probing what 8.5 syntax the index understands, and the first thing it
  caught was **our own fixture**:
  `tests::pipe_operator::a_closure_on_the_right_is_called_as_a_value` was written without the
  parentheses, so it pinned a spelling no runtime accepts -- §8.4's note about a test whose fixture
  does not match its own name, arriving from a new direction.

- **A matrix of seventy-nine shapes against `php -l` found twenty-one fatals nothing reported, and
  three of the answers refute the obvious reading.** The method is the cheap one §9 records: write
  each candidate as one line, lint it, run it, and read what PHP says -- then run the same file
  through `phrost check` and keep the cells where PHP refuses and phrost is silent. What came out of
  it, all in `refused-declaration`: `abstract final` and `final` on an interface's method (one rule,
  since an interface's method is abstract without the word); a **backed** enum's case with no value
  and a plain enum's case with one; and six about a parameter list -- a repeated name, `$this`, a
  variadic that is not last, a variadic with a default, a variadic that promotes, and promotion
  outside a constructor or in one with no body. The three that are not what the wording suggests: a
  variable is **case-sensitive**, so `f($a, $A)` compiles and only an exact repeat is the fatal; a
  **by-reference** promoted property is legal (`private array &$x`) where a variadic one is not; and
  `static` on an interface method, a constant in a trait and a static property in a trait all parse,
  though each reads like something PHP would refuse. A parameter list is checked wherever one is
  written -- a function, a method, a closure, an arrow function -- because PHP refuses all four
  identically, which is what makes one walk the right shape. Measured after: **nothing** on five
  corpora or on the stub set, which is what a load-time fatal should score on code that ships, and
  is why the matrix and not a corpus is the instrument for this family. Still open from the same
  matrix, each a fatal and none of them implemented: **re-assigning `$this`** -- refused by
  `$this = 1`, `$this = &$x` and `foreach (... as $this)`, and _not_ by `$this .= 'x'` or `$this++`,
  which compile and fail at runtime instead, so the rule is about a plain assignment and a foreach
  binding and nothing else -- a reference in a list destructuring, a duplicate `goto` label, `goto`
  into a loop, `break` past the loop count, and `yield` outside a function. The `$this` one is held
  back for want of a check to put it in rather than for want of the rule: it is a statement and
  every neighbour here is a declaration, so `refused-declaration` would be the wrong name on it.

- **The native-type refusals are the half of that matrix a docblock can ruin, and it did.** `void`,
  `never` and `mixed` may only be written standalone; `void` and `never` may only be a _return_; an
  intersection holds classes and not `callable`, `null` or a scalar; a union may not repeat a type
  (case-insensitively) nor hold both `true` and `false`. Every one is a fatal at compile time, and
  every one is about a type **PHP reads** -- so the first run reported thirty-three findings on
  Laravel, all of them `@method static object|mixed make(...)` on its facades. A tag declares a
  member that does not exist, and a member that does not exist cannot fail to compile; skipping
  `MemberData::from_docblock` whole is the rule, not just for the type half. Two answers to keep in
  view, both run: `?int` and `int|null` are the same type written twice and neither is a duplicate,
  so the `null` an operand list holds must not be compared against the `?`; and `self`, `static` and
  `parent` name a class, so `static|int` is a fine union where `Reader&callable` is not a fine
  intersection. Two are deliberately left out because they are about what a name _unfolds to_ rather
  than what was written: `iterable|array` is `Duplicate type array` because `iterable` means
  `array|Traversable`, and `bool|true` is `Duplicate type true` for the same reason. Reporting
  either needs the expansion table, and a rule that knows what was written is the honest one to have
  first. The set found one real defect nobody had noticed: the vendored prelude declares `exec()`
  with `int|int &$result_code`, a generator artifact and a file PHP would refuse. Nothing on five
  corpora, on Drupal core's 11 000 files, or anywhere else.

- **The formatter handed back a file PHP will not parse, and mago's parser agreed with it.** mago
  drops the parentheses of `new (self::SERVICERS[$id])($config)` -- which are not redundant: they
  are how PHP names the class of a `new` with an _expression_, and `new self::SERVICERS[$id](...)`
  is `syntax error, unexpected identifier, expecting variable or "$"`. One file in 120 of the
  reference project, on a formatter that runs on save over files nobody re-reads. What PHP accepts
  there, every row run against 8.5.10: a name, a variable, a property chain (`new $h->m[0]()`), a
  static property (`new H::$s[0]()`), a call (`new f()()`), and a parenthesised expression. A class
  **constant** is none of those, with or without a subscript on it -- so `new H::C()` and
  `new H::C[0]()` are both refused and `new (H::C[0])()` parses. Two fixes, and the second is the
  one that matters. `phrost_syntax`'s `refused` learned the rule, which is where it belongs -- PHP
  refuses the shape whoever wrote it. And `settled` now asks whether its own output parses before
  handing it over, which is the general guard: a formatter that declines has done nothing, and one
  that corrupts has cost somebody their work. The guard needed the rule, because mago's parser
  accepts what its formatter wrote.

- **A reprint can parse, pass `phpcs`, settle, and still mean something else.** Comparing what each
  reprinted file computes with what it computed -- nikic/php-parser over both trees, positions and
  comments dropped, the deliberate respellings normalised (`tools/reprint-means-the-same.php`) --
  found five shapes over 14 816 reprinted files of five corpora, none of which any earlier
  instrument had reported: an `instanceof` operand mago printed without the parentheses PHP needs (a
  cast, a call, a literal), a heredoc closing marker indented with a tab and spaces, a template's
  trailing blank line, a closing tag whose removal turned output into whitespace, and an empty file
  given a newline. Two of those PHP refuses and mago's parser accepts, so they are rules in
  `phrost_syntax`'s `refused` -- the guard declines them and the editor reports them in code a
  person wrote; the `instanceof` parentheses are also put back from that rule's own list, so the
  file still formats. The other three are output: text outside the tags is printed, and every byte
  of it stays. The lesson for the next formatter change is the instrument, not the five: each check
  before it answered a question adjacent to the one format-on-save depends on.

- **The settling loop was Drupal's and the instability is the formatter's.** `settled` looped only
  for the Drupal style, where an array-break _hint_ changes the input. A pass that does not reach a
  fixpoint is a different thing and belongs to every style: WordPress's `deprecated.php` gains a
  blank line on the **second** reprint and none after -- a docblock sitting on the body of a
  braceless `if` -- so a person saving it twice watched it change twice. The loop runs for every
  style now, and is skipped where the first pass changed nothing, which is what a save over an
  already-formatted file does.

- **A token stream is the wrong instrument for a formatter, and finding that out took six
  normalisations.** `tools/formatting-preserves-the-code.py` compares what PHP's own tokeniser says
  a file is made of, before and after a reprint, which should say whether anything but the layout
  moved. What it says instead is that a formatter rewrites a great deal that PHP does not care
  about: a keyword's case, a string's quotes, a parenthesis precedence makes redundant, a trailing
  comma before a closer, a comment's indentation and its `*` markers, the order of an import block,
  a comma-separated `use` split in two, a type respelled, and the indentation a heredoc's closing
  marker strips. Each one read as a finding on real code until it was named. So the default is the
  two questions that need no list at all -- **it parses** and **it settles** -- and the token
  comparison is behind `--tokens`, where the list it needs is written down. The lesson generalises
  past the formatter: a comparison that has to enumerate what is _allowed_ to differ is only as good
  as the enumeration, and every gap in it looks exactly like a bug.

## What `#[\Deprecated]` may be written on, asked of 8.5.10

PHP 8.4 gave deprecation an attribute and the runtime reports reaching one. Which declarations may
carry it is not guessable, and the error message names the list from the wrong side --
`allowed targets: class, function, method, class constant, constant` -- so two of the refusals are
not in it. Asked directly, four lines at a time:

| written on                          | 8.5.10 says                                  |
| ----------------------------------- | -------------------------------------------- |
| class                               | ok                                           |
| trait                               | ok                                           |
| interface                           | `Cannot apply #[\Deprecated] to interface I` |
| enum                                | `Cannot apply #[\Deprecated] to enum E`      |
| function, method                    | ok                                           |
| class constant, enum case           | ok                                           |
| global `const`                      | ok -- attributes on constants are 8.5        |
| property                            | `cannot target property`                     |
| parameter, including a promoted one | `cannot target parameter`                    |

So a property and a promoted property can only be deprecated by a docblock, which is what the
reading does. The two class-like refusals are worth a check of their own: both are fatal at load
time, and neither is in the message's own list.

Two more the same session settled. An arrow function on the right of the pipe operator **must be
parenthesised** -- `$x |> static fn ($it) => $it` is a parse error, `$x |> (static fn ($it) => $it)`
is not -- which is a rule no RFC summary states and `php -l` gives in one run. And a pipe supplies
exactly one argument, so `'x' |> two(...)` is an `ArgumentCountError`: phrost reports it as
`missing-argument`, along with `incompatible-argument` for a piped value the callee refuses and
`undefined-function` for a name that is not there.

## mago 1.50.0's crates buy phrost nothing, and cost the `X + [` join

Asked properly, because "there is a new release" is not a reason and the set and the crates are
independent by design -- the prelude README says a reader who finds the two version numbers
different has found the normal state.

The **prelude** at 1.50.0 was worth taking and was taken: two of its three changed declarations
moved no count on any of the five corpora, and the third is recorded as declined with its number.

The **crates** were bumped, built with no API breakage at all, and passed 2 875 of 2 876 tests. The
one failure is `the_join_is_measured_from_the_line_the_left_operand_ends_on`, and it is not a stale
expectation: mago 1.50's formatter now breaks after the `=` or `=>` when the value is
`[...] + $x + [...]`, pushing the whole merge in one level, where 1.48 kept it on the assignment's
line. That is the idiom a user reported and this project fixed -- `hugging_array_operands` exists
for its other half -- and `Drupal.Arrays.Array.ArrayIndentation` is what rejects the result.
Reprinting `app_core` with both binaries puts the real scope at **one file of 1 051**, twice in it,
the same shape both times.

Against that, what 1.50 gives phrost. The first pass answered this from the release notes and that
was the wrong instrument twice over: the notes describe one release where the step spans two, and a
note is written by someone deciding what is worth mentioning. **Diff the crates instead** -- they
are on crates.io as `.crate` tarballs, so it costs one `curl` each. Changed lines from 1.48.0 to
1.50.0, over the eleven this workspace pins:

| crate                                                                  |                                               lines |
| ---------------------------------------------------------------------- | --------------------------------------------------: |
| mago-syntax, mago-syntax-core, mago-phpdoc-syntax                      |                                               **0** |
| mago-names, mago-span, mago-database, mago-php-version, mago-allocator |                                               **0** |
| mago-reporting                                                         | 14, and reading them: a block moved, byte-identical |
| mago-linter                                                            |                                                  92 |
| mago-formatter                                                         |                                                 244 |

So there is no undescribed anything in the parser, the lexer, the docblock parser, names or spans --
not because the notes are silent about them but because the code is identical. The linter's 92 are
`context.rs`, `import_tracker.rs` and two rules: `no_fully_qualified_global_class_like`, which stops
_offering a fix_ that would shadow a name and says so in its help, and `kan_defect`, which rewrites
one polynomial as `mul_add` and can therefore move a score in its last bits. phrost surfaces a mago
issue's message and `help` and drops its `edits`, so the first is visible only as a help string --
and neither rule fires once on the reference project, Symfony or WordPress, both living in
categories those projects do not switch on.

Which leaves a formatter regression on one side and an empty column on the other, and the pin stays
at `=1.48.0`.

What would change the answer: a release phrost actually uses something from, or a Drupal pass that
pulls a value back onto its `=` line the way `hugging_array_operands` pulls an operator back onto
its operand's. The second is bounded work in a place that already does exactly this kind of
respelling, and the failing test is its specification.

## mago 1.58.2 is taken, and one of its settings is the first memory win a bump has given phrost

Three things in it are phrost's. The parser can leave out the whitespace trivia between two tokens
that no comment precedes (`ParserSettings::elide_detached_whitespace`), and nothing in phrost reads
one: a docblock is found through the whitespace _after_ a comment, which is kept, folding reads only
comments, and the formatter and the linter parse for themselves. Switched on in `ParseSession`, the
reference project's `rss after bootstrap` is 234.2 MB against 247-254 before, three `index --memory`
pairs agree at ~10 MB lower, the index heap and the cold index time are unchanged, and findings and
coverage dumps are byte-identical on five projects. Unlike 1.56's release-note memory work, which
was in crates phrost does not link, this one is in the parse every file goes through.

The docblock parser now refuses a type with a closing bracket glued to it --
`@var array<string, string>)` -- as the typo it is; SimplePie's one such tag stopped being read and
two WordPress assignments took their declared type instead. Counted over six projects' docblocks
with mago's own parser, a check reporting what does not parse would find one real typo on BookStack
(`array{0: string: 1: array}`), and beside it six HTMLPurifier `@type string` tags mago refuses,
intentional Symfony fixtures, and WordPress's hash notation -- which phrost rewrites before it
parses. Not built: the data says it would mostly report mago's grammar, not the author's mistakes.

The formatter keeps the parentheses of `new (self::S[$id])(...)`, which 1.56 dropped and phrost's
reparse guard refused; the guard stays. It reprints five of the reference project's files
differently, and the project's own phpcs scores both versions 0 introduced, 57 removed. The prelude
took two of `third_party/php-prelude`'s edits upstream (`base64_decode()`, the `class_*` maps) and
dropped `@param object|string` from `is_a()`/`is_subclass_of()`, whose native parameter is `mixed`
-- 17 nullable-argument findings on Symfony and Laravel went with it.

## mago 1.56.1 is taken: the memory work is in crates phrost does not use, and one formatter fix is

1.56's release notes lead with memory, and none of it is phrost's to have: the allocations it cut
are the analyzer's, the codex's and the pipeline's, and phrost links none of the three. Measured
rather than assumed, on the reference project and against a 1.55 build of the same commit: the index
heap is 110.4 MB on both, `rss after bootstrap` 254.0 against 254.1 MB, and seven alternating
`index --memory` pairs differ by 2-3 MB with no stable sign -- the noise band section 8.2 of
CLAUDE.md describes. Every p95 the perf sweep reports is within its own run-to-run spread.

The parser's only change is opt-in: `ParserSettings::recover_incomplete` keeps an unfinished body or
list at the end of the file for an editor to query, and is off unless asked for. phrost already
answers that buffer -- `Session::parse` appends the tokens mago reports missing at the end and
parses again, up to four times -- and a probe of mago's own test shape
(`class C { function work($v) { take([1, 2], $v, $this->ot`) completes `other` and goes to it. So
taking the setting would be a second constructor of a rule phrost has; the only thing it could buy
is the re-parses, which is a cost question for `tools/mangled-buffers-answer-without-failing.py` and
not a recovery one. The prelude is byte-identical to the 1.52.0 set `third_party/php-prelude`
vendors, so there is nothing to re-vendor. The suite passed without an edit, and findings and
`coverage --dump` are byte-identical on the reference project and six corpora, BookStack included.

The formatter fix is the one that matters. 1.55 dropped the parentheses of `(new Collection($x))`
before a _property_ access -- `new Collection($x)->filter->running()` -- even when told the
project's floor is 8.3, where that is a parse error; the 1.54 note above made the formatter take the
floor for exactly this rule, and the property form slipped past it in mago. A reprint of Laravel
(`^8.3`) with both binaries differs in five files and seven lines, every one of them 1.56 putting
back the parentheses Laravel's own source writes. The reference project, WordPress, Symfony's
components, psl and BookStack reprint identically.

## mago 1.55.0 is taken: one type changed under the linter, and two lines of formatting

Priced from the crates, as the 1.50 note says: the parser, the lexer, the docblock parser, spans and
the prelude are **byte-identical** to 1.54's, so nothing lowering reads moved. What did:

- `mago-reporting` hands an issue's `message` and `help` over as `Cow<'static, str>` rather than
  `String` -- the one edit phrost needed, in the lint pass that rewords mago's suppression advice.
- `mago-names` resolves `clone`, `exit` and `die` in a namespace to the global function, which PHP
  compiles them to. phrost's own def-map is untouched; mago's resolver feeds only its linter.
- `mago-linter` reworks a dozen rules (`ambiguous-function-call`, `tagged-todo`, `valid-docblock`,
  `cyclomatic-complexity` among them), none of which fires differently on any project measured here.
- `mago-formatter` keeps a cast with the operator before it -- `6 === (int) f(...)` breaks inside
  the call rather than before the `===` -- and a condition whose binary expression was broken across
  lines stays broken.

Findings and `coverage --dump` are byte-identical on the reference project and five corpora. A
reprint with both binaries differs in one file of WordPress's `wp-includes` and one of the reference
project's custom modules, both the cast rule, and the Drupal `phpcs` the reference project vendors
reports nothing on either version of that file. Laravel, Symfony's components and psl reprint
identically.

## mago 1.54.0 is taken, and the bump found that the formatter had been writing PHP 8.4

1.53 and 1.54 change no API phrost uses and no prelude file -- the published set is byte-identical
to 1.52's, so `third_party/php-prelude` stays where it is. Findings and `coverage --dump` are
byte-identical on the reference project and five corpora, and the whole test suite passed without an
edit. The parser's changes are allocations.

The formatter's are not, and one of them showed an old fault. 1.53 drops the parentheses of
`(new Foo())()` "on PHP 8.4+", and the reprint over Laravel came back with `new Foo()()` -- and,
diffed against the 1.52 reprint, with **1 591 lines** where 1.52 had already been writing
`new Collection($x)->every(...)`, the 8.4 form for a member access, into a project whose
`require.php` is `^8.3`. `reprinted` handed mago `PHPVersion::LATEST` on the reasoning in its own
comment, that "a formatter reprints and never introduces syntax: the version only decides what
parses". It was never true: the version decides what the formatter _writes_ -- the 8.4 `new`, and
before it the trailing comma 8.0 allows after a parameter list. On PHP 8.3 every one of those files
is a parse error, written on save.

No instrument here could see it, which is the part worth keeping. `php -l` on this machine is 8.5;
`reprint-means-the-same.php` parses with nikic/php-parser, which reads 8.4 syntax; the formatter's
own parse-after-print guard uses mago's parser, which reads every version alike. Three checks of
"does it still parse", all asked of a parser newer than the project. The question has a version in
it, and none of them was given one.

So the formatter takes the project's floor now -- `FormatTarget { style, php }`, `php` from
composer's `require.php` through `ComposerJson::php_version`, the server's and `reprint`'s alike --
and where nothing says, 8.3: what an unstated project got before minus the one rewrite that breaks
working code. On the reference project (`>=8.4.0`) and Symfony (`>=8.4.1`) the 8.4 form is legal and
stays; on Laravel the parentheses come back. The other 1.53 change, a broken logical group laid out
as a block with its parentheses on their own lines, adds no `phpcs` error under the Drupal standard
on the six reference files it touches.

## mago 1.52.0 is taken: both things the 1.50 note asked for arrived

The note above named what would change its answer, and both did. The Drupal pass now pulls a value
back onto its `=` -- `drupal/joins.rs`, which also joins a whole operator chain where it used to
join only the operator beside the array, and which on its own takes the errors a Drupal-style
reprint of core's modules introduces from 196 to 162 on 1.48. And this release changes the parser,
which 1.50 did not. Changed lines from 1.48.0, measured the same way:

| crate                                                           | lines |
| --------------------------------------------------------------- | ----: |
| mago-phpdoc-syntax, mago-span, mago-php-version, mago-allocator | **0** |
| mago-syntax-core                                                |    46 |
| mago-database                                                   |    79 |
| mago-names                                                      |   214 |
| mago-syntax                                                     |   299 |
| mago-reporting                                                  |   318 |
| mago-formatter                                                  |   473 |
| mago-linter                                                     |   592 |

What the parser's lines are: one fix and a set of allocations removed. The fix is a braced
interpolation holding a string that holds a `}` -- `"{$values["{$keys['}']}"]}"` -- which 1.48
closed at the inner `}` and reported as seven syntax errors on a file `php -l` accepts, and a syntax
error costs everything after it in that file: declarations the index never sees, a formatter that
refuses the file. The rest, here and in `mago-names`, removes allocations without changing an
answer.

What it costs, measured rather than assumed: **nothing that moved**. `check` over the reference
project and five corpora prints byte-identical findings; `coverage --dump` is byte-identical over
409 000 answers on three of them; the index's heap breakdown is the same to the byte and indexing
time is within noise (1.11-1.13 s against 1.13-1.15 s on 28 971 files); every perf budget holds with
the same p95s; reprinting takes the same time. The formatter's own changes are other presets'
business and read as improvements -- WordPress's 289 changed files are nearly all a lone `;` after
`echo` coming back onto its line, Laravel's and Symfony's a `??` chain given a line per operand.

It needs Rust 1.99, which 1.51.1 began requiring for a source build, and `rust-toolchain.toml` moves
with it.

Two instruments misled on the way and are worth knowing before the next bump. `reprint` on
`docroot/core/modules` printed core in the **default** style -- the style comes from the nearest
`phrost.toml`, and core is not under the project's custom code -- so `tools/drupal-style.sh`
reported 567 000 introduced errors that were a preset nobody asked for; pass `--style drupal` to
reprint core. And `phpcs` over core dies on PHP's default 128 MB, writing a fatal error where its
CSV report should be, which the diff script read as a missing column.

## WordPress's hash notation, and a rewrite that keeps every span

`@return array { Description. @type string|false $ext ... @type string|false $type ... }` is the
draft PSR-5 spelling of a shape, and WordPress documents 134 of its returns that way. mago reads
`array {` as `array{` with a space, fails on the prose inside, and the tag said nothing -- not even
`array` -- so `wp_check_filetype()`, `wp_get_current_commenter()` and `get_plugin_data()` had no
return type at all. A `@param array $args {` was never affected: the name between the type and the
brace ends the type before mago reaches it.

The docblock is rewritten before mago reads it, **to the same length**: the braces of a hash become
spaces and `@type` inside one becomes `@var ` -- five bytes for five -- and a positional key `$0`
becomes `$n`, since mago takes no variable starting with a digit; the key itself is read back out of
the text as written, at the span the variable came back with. Every span mago returns is therefore a
span of the file, which is the half that matters beyond the type: a class a key names keeps its goto
and its references. The fields become a shape of required keys, replacing the array operand of
whatever preceded the brace -- `array|false {` keeps its `false`, `int[] { @type int $0 ... }` is
`array{0: int, 1: int}`, and a nested `@type array $x {` is a nested shape.

**What tells a hash from a shape written across lines took two tries.** The first version opened a
hash at any `@return` line ending in `{`, and PHPStan's multi-line `@return array{` above `key: T,`
lines is exactly that -- 34 assignments on the reference project went from a shape to a bare
`array`, and psl and Symfony moved too. Now both are required: a space before the brace, and a
`@type` line directly inside. The coverage dump caught it before any finding moved, which is the
instrument doing the job the finding list cannot.

Measured: WordPress 191 assignments typed that had none and 106 refined from `array` to a shape,
none lost; 26 findings in and 12 out, most of both the same finding with a more specific type in its
message, and the new ones real -- `strtolower($type['ext'])` on the `string|false` the hash
declares, `wp_install()` returning the `int|WP_Error` of `wp_create_user()` where its hash says
`int`, an `fread()` whose `false` now survives the join with a typed body. The other five corpora
unchanged. `check` and `index` unchanged on WordPress and the reference project. No stored shape
changed, so no cache version: the build stamp already retires rows another build derived.

## Laravel's helpers and relations are rules the framework states, not declarations

`app('router')` and `resolve('cache')` are `make` with the container left out, and the container's
aliases were bound for `make`, `offsetGet` and `get` only -- a binding names a method on a class,
and a helper is a function. `FunctionBinding` is its half at the seam, read both by the parsed path
and by the chain scanner a half-typed `app('router')->` goes through. Writing it found a bug of its
own shape: the `FunctionReturn` arm handed its answer back with `return`, which skipped every step
written after the call, so `app('redirect')->back()` read as the `Redirector`. An arm's answer is
its value.

`$post->author` is `author()->getResults()`: the model's `__get` finds the method, sees a relation
and loads it. `MethodBackedProperty` states that once for `Model` and `Relation`, and the property
is answered, navigated and completed through the project's own method -- and counted among that
method's references, or a rename leaves every property spelling reading nothing. Single relations
are `?TRelated` because Laravel declares them so, and `$post->author->name` is reported under the
same null rule as anything else: 46 such findings in Laravel's own tests.

The attributes a model states in its body -- `$casts`, `casts()`, `$fillable` -- are read now
(`StatedProperties`), and the decision they waited on was taken the strict way: every cast is
nullable. Not a guess -- `castAttribute` returns a null column before any cast runs, so `?Carbon` is
what `published_at` _is_ without a migration in sight, and the null-safety rule says which way to
err. A listed name with no cast is `mixed`: there, and nothing more. On Laravel's own tests that is
about 620 accesses newly resolved and 125 `possibly-null` findings on `->format()` and friends,
every one a column the code assumes is set.

Two PHP rules shape the reader, and both were asked of the language rather than of Laravel. A
property's default is not merged with the parent's when a subclass redeclares it, so a subclass's
`$casts` replaces the parent's array whole and a cast only the parent listed is gone there -- while
the parent's `casts()` method still runs unless overridden. So each row carries what wrote it, and a
nearer class writing the same source shadows the parent's.

Lowering keeps no property defaults, and still keeps none but these: the names a rule reads are
handed down through `KeyedAttributes::keeping_defaults_of`, derived from the installed rules so the
gate and the reader cannot name different properties. The rows live beside the file table rather
than on it -- a field on every file row was a measured megabyte on a Drupal checkout for a fact only
models have. Laravel 12's skeleton writes `#[Fillable(['name', 'email'])]` on `User` where
`$fillable` used to be, so that spelling is read too -- kept by lowering as the same list, matched
by the attribute's last segment, which is how lowering keeps it and a rule must compare it. A cast
class of the project's own is typed by its `get()` as declared, null or not: the framework hands it
the null column instead of answering for it.

The key is `?int` too -- `getCasts()` adds `[getKeyName() => getKeyType()]` while the model is
incrementing, and a model not yet saved has none. Only while the three defaults hold: a class, or a
trait it takes, declaring `$primaryKey`, `$keyType`, `$incrementing` or the methods reading them
(`HasUuids` does, through `HasUniqueStringIds`) has a key this cannot name, so its `id` is left
alone rather than guessed.

Dynamic wheres -- `User::whereEmail($email)->first()` -- had no declaration and answered `mixed`, so
the chain after one answered nothing. The query builder's `__call` reads the column off any name
beginning `where`, and `FluentPrefix` states only that: the class, the prefix, and that the answer
is the receiver. Everything between a model and that builder was already stated, and is reused
rather than restated -- the Eloquent builder reaches it through its `@mixin`, so the receiver it
hands back is itself, and a model through the implied mixin, so `Post::whereTitle()` is
`Builder<Post>`. Which mixin decides is asked first, because a model's member table reaches the
query builder too and `$this` there would be the model. A list offers `where` and each column the
model states -- the words the project wrote down, since any word is a call `__call` takes.

A real application found what the framework repository and the skeleton could not: BookStack writes
`use DB;` in 68 places, and every one was an `undefined-type`. Laravel's `AliasLoader`
`class_alias`es the facades to global names at start -- `config/app.php`'s `'aliases'`, which since
Laravel 11 a project may leave out, leaving the framework's own `Facade::defaultAliases()`. The list
is read from the installed `Facade.php` rather than restated, because it grows each release, and the
project's own list is read beside it -- merged when it calls `defaultAliases()`, a replacement when
it does not. `ClassAlias` is the seam and `ProjectSkeleton::class_declarers` the one lookup that
honours it, so goto, hover, member tables and the type checks all see the facade. A name a file
declares stays the declaration's, as PHP's `class_alias` refuses a taken name. Packages'
`extra.laravel.aliases` are not read; neither is a config directory an application moves, which
BookStack does (`app/Config`), so its own two aliases are not seen -- it imports them by full name.

The same application then reported `trans('auth.mfa_setup')` passed where a string is wanted 191
times: the helper declares `array|string`, and larastan answers with a benevolent union that no
level of PHPStan complains about. The question is decidable instead of waivable. The translator
hands back the line a key names, the array a group or a nested list is, and the key itself where it
names nothing -- so a literal is an array only when the language files say so, and those keys are
few (28 on BookStack, the framework's own four files included, which an application that never
published them still reads). They are filed as the catalog's `Shape(Array)` entries, and
`UnlistedKey` says what every other literal holds; a key with `::` names a package's files and is
left to the declaration, as is a key that is not a literal. The function half of an argument binding
now reads shapes as the method half always did -- it read only classes, which is the
two-constructors form again. The lexer is the reader's own and not the config reader's comment
stripper, because a translation is prose with `//` and `#` in it; its first run panicked on a byte
slice in the middle of `العربية`, at server start.

The model's implied mixin took in the builder's _properties_ along with its methods, because a
written `@mixin` does and the implied one is read as if written. But what Laravel implies is a
forwarded call: `Model::__call` hands it to a builder, and `Model::__get` reads an attribute or a
relation, never a builder's property. So BookStack's `$reference->from` -- a `from()` morph relation
-- went to `Query\Builder::$from` and read as `Expression|string`, and any model with a relation
called `orders`, `columns`, `joins` or `groups` had the same trap waiting. An implied mixin now
brings methods only; a written one keeps PHPStan's reading, properties included.

## A Blade template is read as PHP of the same length

Every site where only phpantom answered on BookStack was in a `*.blade.php`. The templates were
already indexed -- the extension is `php` -- as one long run of inline HTML, which declares nothing
and answers nothing. They are now read through `TemplateDialect`: the framework names a file ending
and a function that turns the template into PHP **as long as the template, with every newline where
it was**. Expressions keep their bytes; markup and Blade's delimiters are overwritten with blanks,
or with the few characters that make the expressions a program -- `;` after `{{ }}`, `if` over `@if`
and a `{` owed to the next blank byte, `}` over `@endif`, a directive's arguments as a list. So an
offset in the buffer is the same offset in what is analysed, and no request translates a position;
goto, hover, completion and rename work inside a template without knowing they are in one.

Three constraints shaped the writer, each found by running it over BookStack's 325 templates and
parsing the result. A template that begins with its first expression has nowhere for the opening
tag: the short `<?` takes two blank bytes, a leading `{{-- @var ... --}}` carries it in its own
delimiter (the text inside moves one byte, paid for by `*/` being two shorter than `--}}`), and a
directive with no room for both its keyword and the tag keeps its condition as an expression and
loses only its block -- 26 templates began that way and every one read as markup before. Markup
holding text outside ASCII is kept verbatim in a `/* */` comment, because a client counts columns in
UTF-16 and two spaces for one Cyrillic letter would move every position after it on the line. And
only the directives Blade ships are read: `@media` in a stylesheet and Alpine's `@click` are markup.
318 of 325 parse clean; the other seven end on a directive whose `;` has no byte left, which the
session's own repair appends.

Not answered for a template: findings, formatting, code actions, tokens and folds. A finding would
be about markup turned into PHP, and the others would rewrite or paint a buffer that is markup --
`Workspace::editable_text` is the one door those requests use. What a template _reaches_ is still
recorded, so a method only a view calls is not reported as unused. Variables are typed by a
`{{-- @var --}}` or a `@php /** @var */ @endphp`; BookStack writes neither, so 240 of 1 400 member
accesses in its templates resolve -- what types the rest is the controller's
`view('name', ['post' => $post])`, which is the next step. Index time and heap are unchanged on
BookStack (560 ms, 55.5 MB).

A template's variables are then what the calls rendering it hand it.
`view('posts.show', ['post' => $post])` is recorded where it is written (`RenderSite`: the
template's name, where the array begins, the keys it holds -- carried by the codec, since only a
body says it), and whatever analyses the template has the rendering files lowered beside it: the
open set for a buffer, the scratch set for a file read from disk -- one helper for both, so an open
template and a closed one see the same variables. The value is resolved in the rendering file's own
view, with its imports and its locals, and carried back with its written types spelled portably.
`@include('row', ['p' => $p])` is written by the masker as the `view()` call it is, so a partial
reads its variables from the template including it, and the renderers are followed three levels up.
And `$errors`, which Laravel shares with every view, is the dialect's to state. On BookStack: 240 of
1 400 template member accesses resolved before this, 916 after; a serial `check` 2.40 s against 2.50
s, every finding unchanged. What is left is mostly data passed as something other than an array
literal (`$editorData->getViewData()`), and `$loop`, which is a `stdClass`.

One recurrence to note: the question "what binds a variable nothing in the file binds" was asked in
two places -- the variable's card and a path through it -- and only one of them learnt templates, so
`$post->title` went to its declaration while hovering `$post` said nothing. `unbound_held` is now
the one place both ask.

Components are templates too, rendered by a tag rather than a call: `<x-avatar :user="$user" />`
hands `components/avatar.blade.php` its `$user`. Same-length masking cannot write a `view()` call in
the bytes a tag has, but it can write the call the tag _is_: `<x-` is three bytes, `x(\` is three,
and every attribute becomes a named argument in the room its own name and `="` leave --
`x(\avatar,user:  $user  );` -- since a camel-cased variable is never longer than the kebab-cased
attribute it comes from. The index reads the name back as text (`RenderKind::Named`, template
`components.` + name, `index` and repeated-name directories as aliases). The name is written as one
fully qualified name, `.` as `\` and `-` as `_`, because `<x-empty-state>` read as constants is
`empty(` -- and PHP reads a keyword inside a qualified name as part of it.

Measuring it found a bug of the old kind: a variable passed to a call is bound by a
`PathRoot::ByReference` root when nothing else binds it, for `preg_match($p, $s, $m)` -- and when no
call wrote it, that root answered nothing, so a template's `$post` stopped resolving on the line
after `<x-card :post="$post" />`. A call that wrote nothing leaves the name as unbound as before,
and the root now says so by asking `unbound_held`. The same correction typed seven `$_REQUEST['x']`
arguments on WordPress that a call earlier in the file had hidden, every one a real `string|array`.
Blade's own directives are matched in any case (`@endGuest`), because Blade finds them with
`method_exists`. Laravel.io: template accesses resolved 272 -> 408 of 697, components' own 49
-> 173.

A Livewire component's template is the one whose `$this` is real: Livewire includes it inside
`Closure::bind($fn, $component, $component)`, so `$this` is the instance _and_ the scope is its
class -- a protected member is as reachable as in a method -- and every public property declared
below `Livewire\Component` is a variable (`getPublicPropertiesDefinedOnSubclass`). So the seam is
`enclosing_class` itself, which answers a template with the class that renders it, rather than
`this_held` learning one more case and `self::`, visibility and the variable card each needing it
again. Which class is said in one of two places: a `view('livewire.x')` in a class's body, which the
index now records with the class it is written in even when it hands nothing over
(`RenderSite::class`), or -- for the four of Laravel.io's nine components with no `render()` -- the
template the class's name spells under `App\Livewire`, as `ComponentRegistry` derives it. A key-less
site is no reason to lower its file beside the template; only a site with keys is.
`<livewire:likes :subject="$x" />` is a component tag under another name and takes the same masking
(`livewire(\likes,subject:$x)` fits the ten bytes `<livewire:` is), so a dialect's tags are now a
list, each with the variables its templates share. A property's declared type answers before the
tag's value -- `mount()` may transform what it was handed -- and a type read off a default only
after it. Laravel.io: template accesses resolved 450 -> 483.

The same corpus showed Laravel 11's slim config: a `config/auth.php` that writes no `'providers'`
has the framework's own, merged beneath it by `LoadConfiguration`, so the user model is read from
`vendor/laravel/framework/config/auth.php` when the project's file names none. Laravel.io's member
accesses with no such member 755 -> 703, and every other corpus unchanged.

## A template is checked, and what the reader wrote is not

Templates were first read and not judged, on the grounds that a finding in one would be about markup
turned into PHP. Measured over BookStack and Laravel.io with every check on, that was true of three
things and of nothing else: `undefined-function` on the calls the masker writes for a tag and an
include (`x()`, `livewire()`, `_inc()` -- 408 on Laravel.io), `syntax` where a statement ends the
file with its `;` owed and no byte to hold it, and the raw-text lint, which parses markup. The
dialect now names the calls it writes (`TemplateDialect::calls`) and the check passes over them; a
template's syntax complaints are dropped while where it broke still bounds what the rest is believed
about; and the lint does not run over a template. What is left is the same judgement PHP code gets
-- 66 findings on Laravel.io's templates and 49 on BookStack's, nearly all under the strict-null
position (`$articles->get(2)` handed to a component is `?Article` at every use), and the rest
genuine: a contract read for members it does not declare, a deprecated method. Not every Blade
structure error is caught this way -- an unclosed `@error` makes PHP that real Blade also compiles
broken (`php -l` on its output says so, on Laravel.io's `forum/threads/show`), and that is exactly
the complaint now dropped; pairing directives is a check of its own.

`unbalanced-directive` is that check. The masker records every directive it reads -- it already
passes over comments, echoes, `@php` and `@verbatim` correctly -- and the dialect pairs them
(`TemplateDialect::problems`), by the closer a block is named for first and only then by the
structure it compiles to: `@auth ... @endif` is valid PHP and is not reported, and an `@endif` past
an `@error` nobody closed is read as ending the `@if` around it, so the `@error` is named and not
the `@can` three blocks out, which is where pairing by structure alone put the blame. It reads the
template as written, which the index does not hold for an open buffer -- that is the PHP made of it
-- so the workspace keeps an open template's text beside it. Laravel.io: three, each an
`@error('body')` written as if it printed the message, each a template real Blade compiles to PHP
that does not parse; BookStack and Monica: none.

Turning the checks on also priced `@use('Illuminate\...\Renderer')`, Blade's import, which the
masker wrote as a list: three `undefined-type` findings in Laravel's own exception pages, every one
a name the import resolves. It is now the `use` statement Blade compiles it to, in the same bytes --
`use ` takes the directive's name, its parenthesis and the quote, which leaves room for the opening
tag at the very start of a template -- and with an alias where the bytes between the two strings
hold `as`.

## An include hands over every variable, and a component only its array

Blade compiles `@include('pages.form', ['model' => $page])` to a render handed the array _and_
`get_defined_vars()`, so a partial reads `$page` that nothing names to it: whatever the including
template holds there, bound in it or handed to it a level up. `@component` is handed its array and a
slot and nothing else, so the two can no longer be the same `view()` call: the masker writes an
include as `_inc(` -- four of `@include`'s eight bytes, as `view` was -- and the dialect states it
(`TemplateDialect::includes`), so the index marks such a site `inherits` and records it even with no
array. Reading a variable is then a walk up the includes, breadth first and each file climbed past
once, stopping where a site names the variable or the including template binds it -- a by-reference
argument excepted, since `userCan($x, $chapter)` reads what it was handed. A recursion through
`resolve` would have been shorter and is exponential in a partial forty templates include. Four
levels, and the files lowered beside a template follow: on BookStack the most for one template went
46 -> 67, the mean 2.7 -> 3.7. Template accesses resolved: BookStack 1007 -> 1035, Laravel.io 522 ->
528; the largest remaining group, `pages.edit`, is rendered with `$editorData->getViewData()`, which
no reading of the call can name.

## A Pest test's `$this` is configuration, and `@param-closure-this` cannot say it

`test('...', function () { $this->login(); })` binds the closure to a test case Pest generates per
file, extending the class `uses(Tests\TestCase::class)` names -- in the file, or with
`->in('Feature')` in `tests/Pest.php` for a directory, `pest()->extend(...)->use(...)` since Pest 3.
`test()` itself declares `@param-closure-this TestCase`, PHPUnit's, which is every declaration can
say. Read that way, 467 of Laravel.io's 686 `undefined-method` findings were the project's own test
helpers and Laravel's HTTP testing methods on PHPUnit's class.

`ConfiguredClosureThis` is the rule (Pest's provider is a directory of its own, detected by the
installed package): the index records each such call where it is written -- the classes along the
chain and the directories an `in()` applies it to (`ConfiguredThis`) -- and `closure_this` replaces
a declared class with the configured one that descends from it. Every call configuring a file is
merged, as Pest merges them: `uses(TestCase::class); uses(WithFaker::class);` is one test case using
one trait. The trait is part of `$this` (an intersection) and part of the scope, since its body is
copied into the generated class -- `$this->faker` is protected and reachable, and without the scope
half it was 7 `inaccessible-member` findings. The cost is the quiet direction: an intersection with
a trait excuses a member neither operand declares, so a misspelt helper in a test file that `uses()`
a trait goes unreported, where in one that does not it is reported on the configured class.
Laravel.io 877 -> 228 together with the stub ranking in `docs/08-index.md`; what is new is
`assertSeeText('Login', '<a>')` passing a string as `bool $escape` eight times, and `fresh()` and
`first()` read as the nullable values Laravel declares them.

## A command's `$signature` says what each input holds, and the reads are beside it

`$this->argument('user')` and `$this->option('queue')` declare `array|string|float|int|bool|null`,
because the method cannot say which input it is asked for; the command's
`protected $signature = 'mail:send {user} {--queue=}'` can. `DeclaredInputs` is the rule, and the
grammar travels with it as the Laravel provider's own function (`console.rs`), the way Blade's
`to_php` does -- the language of a signature is the framework's and nothing below the seam should
know it. What each spelling holds was read off the runtime, Laravel's parser building a definition
and Symfony's input reading it back on PHP 8.5.10, and matches Larastan's `getArgumentType` and
`getOptionType`: `{x}` and `{x=d}` a string, `{x?}` a nullable one, any `*` a list, `{--x}` a flag,
and every option taking a value **nullable whatever its default** -- `--queue` written with no value
hands back null even beside `{--queue=default}`, and `--ids` alone appends a null to a list.

Nothing is stored per file. A command's `handle()` sits beside its signature, so the view reads the
signature out of the file being analysed, where lowering keeps a string default under a name a rule
hands down (`KeptTextData`, the sibling of the array defaults Eloquent's `$casts` uses). A command
declared in another file keeps the method's own answer -- missing, not wrong -- and so do commands
that declare their inputs in `getArguments()`/`getOptions()`, which this does not read.

Laravel -67 (69 gone, 2 new and both real: a `{--environment=*}` handed on as `array<int, string>`
when its elements may be null, and `RetryBatchCommand::isolatableId()` declaring `string` and
returning the `{id?*}` list), BookStack -17, nothing else moved.

**The test harness lowered open buffers without the list a framework hands down.** `World::open`
built its `OpenSet` with no keyed attributes, where the server gives both halves of the index the
same list -- so a reader of the _open_ file's HIR saw no kept literal at all, and the first view
test of this rule read every input as the method's declaration. Every earlier reader of a kept
default went through the skeleton, which had the list, and that is why it never showed. Fixed in the
harness; no other test moved. Same family as the CLAUDE.md note that a `check` harness must mirror
the server's setup -- a harness is a second server, and every list the real one installs is one it
has to install too.

### And a read of a name the signature does not write is an error

`$this->option('queu')` is Symfony's `The "queu" option does not exist.` the moment the line runs,
so `undefined-input` reports it as an error, with a declared name one edit away offered the way a
catalog key's is. The signature is the whole list once the application's own inputs are added --
`command`, `--help`, `--silent`, `--quiet`, `--verbose`, `--version`, `--ansi`/`--no-ansi`,
`--no-interaction`, `--env`, read off `Illuminate\Console\Application::getDefinition()` on Laravel
12 and Symfony 7.4, and `--isolated` for an `Isolatable` command -- and where it may not be the
whole list nothing is judged. That turned out to need two gates and not one. The first run reported
seven reads of `--test`, `--pest` and `--phpunit` in Laravel's own `make:` commands: their
signatures write none of them, and `GeneratorCommand`'s constructor adds all three to every command
using `CreatesMatchingTest`. So only a class extending `Command` directly is judged, and not one
declaring `configure()` or calling `addOption()`/`addArgument()` itself.

Zero findings on Laravel, BookStack, Laravel.io, Monica and the skeleton app -- maintained code, and
Larastan has the rule -- so the check was proved on a clone of BookStack with two typos planted:
both reported, both suggestions right (`oldUr` -> `oldUrl`, `forse` -> `force`). No slower:
`check --jobs 1` on BookStack reads 2.86-3.02 s on both builds.

### And the signature is where a read of a name goes

The signature is the only place an input is declared, so goto on `'queue'` in
`$this->option('queue')` lands on the `queue` of `{--queue=}`, a hover says what it holds and what
the signature's ` : description` says it is for, completion inside the quotes offers the inputs that
method reads -- an argument's names to `argument()` and an option's to `option()` -- and a search
lists the declaration with every read. A rename is refused: whoever runs the command names the input
too, in a crontab, a scheduler's `command('mail:send --queue=high')`, `Artisan::call()` or a test,
and none of those is a read the index holds. Two details decide whether it works on real names.
Completion filters by everything typed after the opening quote and replaces the whole literal, not
the word before the cursor -- `dry-run` is one name and a word ends at its dash. And `$this` is the
innermost class around the read, so a read inside an anonymous class inside `handle()` names nothing
of the command's.

For that the parser hands back where each name is written, which is an offset into the literal's
_value_; it is an offset into the file only where the value is the text between the quotes. A
signature with an escape in it -- `"mail:send\t{--queue=}"` -- is still read for types and still
described, and goes nowhere, rather than to a column one off. On BookStack goto from
`$this->option('users-without-avatars')` lands on line 22 of a five-line signature, and no finding,
hover or completion p95 moved on a build that answers it.

## A model's attributes are its table's columns, and the migrations write the table

`$goal->name` on an Eloquent model declares nothing anywhere in PHP: `__get` reads the row, and the
only statement of what the row holds is the migration that created the table. phrost read `$casts`
and `$fillable` and nothing else, so a model that writes neither was a class of `mixed` properties,
and every cast was `?T` because whether the column can be null was nowhere it looked. PHPantom and
Larastan both read the migrations; on Monica, whose models carry two `@property` tags between them,
14 % of every member access in the project was a read of a column nothing typed.

The split follows the seam. Finding `Schema::create('goals', function (Blueprint $table) { ... })`
in a body and the chain of calls on `$table` is the index's (`crates/index/src/schema.rs`); what
`$table->foreignIdFor(Contact::class)->nullable()` _does_ -- adds a `contact_id` that may be null --
is the framework's, a function over the call as written (`laravel/schema.rs`), as a signature's
grammar is. Read at ingest, stored with the file, carried by the codec, and composed into tables on
the first question after any migration changes, by file name -- which is the order the migrator runs
them, and the order a timestamp in the name exists to give. Only `up()`, since `down()` undoes it,
and only under a `migrations` directory, since a test that builds a table in `setUp()` writes the
same calls and is not the project's schema.

What a column holds is the **raw** value the driver hands back, not the one a name suggests: a
`decimal` is a string so as not to lose digits, a `json` column the text it stores, a `date` the
text it was written in -- Laravel casts `created_at` and its siblings itself and anything else only
where `$casts` says. `boolean` is the one that differs by driver, `1` from MySQL and `true` from
PostgreSQL, so it holds `mixed`. A cast keeps its type and takes the column's null: `datetime` on a
column that cannot be null is a `Carbon`, which is the half of the answer the model could never say.

The table a model's rows are in is `getTable()`: the nearest class naming one in `$table`, else
`Str::snake(Str::pluralStudly(class_basename($this)))` with Doctrine's English rules for the plural
-- ported rule for rule and checked against Laravel itself on all 650 model names in four Laravel
projects' code and tests, every one the same (`Bookshelf` is `bookshelves`, `Person` is `people`,
`APIToken` is `a_p_i_tokens`).

Monica: member accesses resolved **83.0 -> 90.3 %** (+3 427), the "dynamic" share 14.0 -> 6.7 %.
Findings -27 / +22. The 27 are a cast's null on a column that cannot be null -- `written_at`,
`happened_at`, a `boolean` returned as `bool`. The 22 are columns that are nullable in the migration
and read as if not: `->integer('position')->nullable()` assigned to an `int` property seven times,
`->string('password')->nullable()` handed to `Hash::check()`, and `Tag::find($tag->tag_id)`, whose
argument is now an `int` and whose answer is therefore the `?Tag` it declares. And a key a class
changes -- `HasUuids`, a `$keyType` -- reads its column rather than nothing, which took five more:
`$vault->id` is the UUID string its migration writes, so `Vault::findOrFail($vault->id)` is a
`Vault` and not the `Collection<int, Vault>|Vault` an array argument would make it. BookStack +29
accesses and no finding moved: its models declare their columns with `@property` already. Laravel.io
moved nothing, because its schema is a squashed `database/schema/mysql-schema.sql` that this does
not read.

One thing found while measuring and left: Laravel's `getAttribute()` asks for a column before a
relation of the same name, where phrost's chain asks the relation first; no corpus has the
collision, so it is a note rather than a change.

### And what a list offers on a model beyond its table

Two more names PHPantom offered on BookStack and phrost did not, each with a line drawn where it
over-offers. A `where` for every column a model declares with `@property` rather than in a migration
or a cast -- but only for a tag no method backs, since a tag a method backs is a relation: PHPantom
offers `whereChapter` and `whereRevisions`, which ask for columns that do not exist. And each
relation's count, `comments_count` beside `comments()`, which a query's `withCount()` fills: typed
`?int`, because it is nothing until a query counted it, and goto lands on the relation. A column
spelled the same way is the table's and not a count -- a denormalised counter is the one thing named
like this on purpose. No finding moved on four Laravel projects.

### And a facade's tag is a summary of its root's method

Laravel writes its facades' `@method static` tags with a script that reads each facade's `@see`
classes, and the script drops every method template. `Cache::remember('k', 60, fn () => $users)` is
tagged `mixed` where `Repository::remember()` says `@template TCacheValue`,
`\Closure(): TCacheValue $callback`, `@return TCacheValue` -- so on Laravel.io the `$topAuthors` a
controller caches and hands its view was nothing, and `$author->username()` in the template was 831
completion items short of PHPantom's list. PHPantom patches the `Cache` facade by name.

The seam's `MethodsTaggedFrom` names the source instead, and the flattener puts the source's public
declaration in place of a tag it declares (`merge_tag_sources`), forwarded, since the call runs
through `__callStatic`; a tag the source does not declare stays as written. Three facades drop a
template their root declares, by a scan of every facade's `@see` against its targets: `Cache` (four
closure-taking methods), `App` (`make()`'s `class-string<T>`) and `DB` (the connection's
`transaction()`, written in a trait the scan has to follow). Laravel.io: 24 unknown receivers
resolved; Laravel: 25. One finding, `Str::plural('Article', $author->articles_count)`, where the
count is `?int` by the rule above and a `selectRaw` put it there.

### And an application's facade names its root in code

`BookStack\Facades\Theme` writes no `@method` tag; `getFacadeAccessor()` returns
`ThemeService::class`, and that body is the only statement of what `Theme::dispatch()` reaches. 49
of BookStack's dynamic reads were calls through it. The seam's `MixinReturnedBy` names the method,
lowering reads a body that is exactly `return X::class;` as a `@mixin X` on the class -- reusing the
name reference the body's lowering already made, so the name is not in the file twice -- and the
flattener and the static completion through `__callStatic` do the rest, as for any written mixin. An
accessor returning a string names a container binding and stays unread. BookStack resolved 53 more
member accesses, Laravel 18; no finding moved anywhere.

### And a protected method `__call` hands back

`Model::increment()` and `decrement()` are protected, and `Model::__call` opens with
`if (in_array($method, ['increment', 'decrement', 'incrementQuietly', 'decrementQuietly'])) return $this->$method(...$parameters);`.
So `$post->increment('views')` from a controller is refused by PHP, sent to `__call`, and runs the
declaration after all; `Post::increment(...)` does the same through `__callStatic`, which makes an
instance inside the model's own scope. Checked against PHP 8.5 with a four-line class -- and the
converse with it: a _public_ instance method through `::` is an `Error`, because `__callStatic` only
runs for a name the caller cannot reach.

The list is in a method body, so the seam says it (`OpenedByCall`) and the flattener marks the
member (`ResolvedMember::opened`, carried onto an override, which `__call` reaches the same way).
The one question both readers ask is `ResolvedMember::reached_with`: what completion offers and what
`reach_of_member` resolves cannot disagree about a reopened name. Before, the call read as one
`__call` answers -- no type -- and completion left it out. Monica's 79 sites where PHPantom offered
both and phrost neither are gone; on Laravel's own tests three findings appear, all a castable
object handed to `increment()` against its `@param float|int $amount`, which is what the docblock
says and what every other docblock is held to.

### And a model's factory is the one named after it

`Contact::factory()` declares `@return TFactory`, a template of `HasFactory` that nothing fills
unless the model writes `@use HasFactory<ContactFactory>` -- which none of 97 models in three
Laravel applications does, against 3 815 `::factory(` calls in their tests and seeders. Laravel
finds the class by name: `Factory::resolveFactoryName()` is `Database\Factories\` and the model's
name without the application's namespace and its `Models\`, then `Factory`; the application's
namespace is the autoload prefix of `app/`. That is the seam's `NamedAfterCaller`, a static method
handing back the class named after the one it was called on, read only where nothing written fills
the template, where nothing below the trait (`newFactory()`, a static `$factory`) names the class
itself, and where the class exists -- otherwise the declaration stands. Checked against Laravel's
own resolver.

The factory then says the rest: each one writes `@extends Factory<Contact>`, so `->create()` is a
`Contact` and `->count(3)->create()` a collection of them. Monica resolved **90.3 -> 97.6 %** of its
member accesses, and the "dynamic" share 6.7 -> 0.7 %. Against PHPantom on the same sample, the
sites only it answered went from 16 definitions and 27 hovers to one of each. And 185 findings more,
all in `tests/`, all the strictness the application's own code already had: `$journal->vault->id` on
a `BelongsTo` that answers `?Vault`, `fresh()`'s `?static`, Sanctum's `currentAccessToken()`
declared as an interface with no properties.

And the other direction, where a factory says nothing about its model. Laravel.io's seven factories
write neither `@extends Factory<User>` nor `$model`, and `Factory::modelName()` finds the model by
the factory's name: `UserFactory` makes `App\Models\User`, or `App\User` where there is no such
model. That is a template filled by a subclass's own name -- the seam's `NamedTemplateArgument`,
answered where a member's `TModel` is substituted (`supplied_argument_at`, so a bare `TModel` and a
`Collection<int, TModel>` alike), only where nothing up the chain wrote the argument and no class
below `Factory` declares `$model`, and only for a class the index holds. Both candidates checked
against Laravel's own resolver. Laravel.io resolved **88.4 -> 93.1 %** of its member accesses, 404
dynamic reads of a bare `Model` down to 88; 54 findings re-named from `?Model` to the model they are
about and ten more, three of which are an `id` this cannot yet read as non-null because the
project's schema is a squashed SQL dump.

The case it stepped aside for is the commoner one. BookStack's factories write
`protected $model = Book::class;` and nothing else, and a declared `$model` was read as "the name is
not the answer" -- so `TModel` stayed unfilled and 367 reads were of a bare `Model`. A `::class`
default is a fact the declaration states, so lowering keeps it the way it keeps a `$table` string:
only for a property some contract asked for (`set_kept_defaults`), resolved where it is written, and
stored per file (`class_defaults`) so it survives the cache. `NamedTemplateArgument::from_property`
reads it first, up the chain below `Factory`, and the name convention only where no class declares
the property. A `$model = 'App\Models\Book'` string is not read: it says the same thing, and nothing
here guesses from a string that a class constant would have stated. BookStack: **367 -> 87** dynamic
`Model` reads, the 87 left in Blade views; findings re-named to the model they are about, and four
more, every one `attachRole(Role::getRole(...))` where `getRole()` answers `?self`.

## A macro is a method its class is given by a call, and `__call` must not answer first

`Collection::macro('sortByCollator', function (...) { ... })` in a provider's `boot()` gives every
collection a method nothing declares. Monica calls it 41 times, and every chain after it was a
receiver nobody could name. What is kept is what a closed file can keep: per registering file, the
class the call names, the name, where it is written (inside the quotes, which is where goto lands),
and either the return the closure writes or the one chain its `return` builds off `$this`,
`static::` or a named class (`MacroSite`, cache v72). Which static method registers and which trait
takes registrations is the seam's `MacroRegistrar` -- `Macroable::macro` for Laravel -- and whether
a class takes them is asked where one is used, because that needs the hierarchy: a `macro()` on a
class that does not use the trait is somebody else's method of that name.

The first build resolved the call and typed nothing after it, and the fixture said it worked. The
fixture's `__call` had no return type; Laravel's `Macroable::__call` is declared `mixed`, and
`magic_accessor_held` answered that before the macro was asked -- the _catch-all answering a
question its inputs could answer better_ shape, found only because coverage counted the call as
resolved and the next step as `UnknownReceiver`. A registration is more specific than the method
that dispatches it, so it is asked first, and the fixture now declares `__call` the way the
framework does; a sabotage of the order reads `mixed` there.

Measured on Monica: 41 calls `Dynamic` -> resolved, the 41 steps after them resolved, 36 assignments
typed; index memory unchanged at 47.0 MB and every request p95 within noise. Not kept: the closure's
parameters (so no signature help on a macro), `::mixin()`, and references or rename from a call to
its registration.

## A Twig namespace in Drupal is two directories, and the docroot walk skips names a template keeps

`{% include '@my_module/parts/thumb.html.twig' %}` is answered from a catalog the Drupal provider
fills with every `.twig` file under every extension's `templates/`, by the name Twig loads it by
(`phrost-twig` reads the names, the view judges them, and a `.twig` name under a namespace that was
walked and is not in the catalog is reported, because the loader throws). Two things were wrong with
the first version, and both were found by opening the text of all 1 309 templates on the reference
checkout -- core and contributed included -- at stand-in paths in the project's own code, which is
the only way to make the server check a file it would otherwise not.

**A namespace is registered twice.** Drupal's `FilesystemLoader::__construct` adds `<ext>/templates`
_and_ `<ext>` under the same name -- the second, its own comment says, so a name can reach the
extension's root without climbing out of `templates/` with `..`. So
`@announcements_feed/templates/announcements-feed-admin.html.twig`, which both `gin` and core's
`default_admin` write, is a correct include of a file the catalog files as
`@announcements_feed/announcements-feed-admin.html.twig`, and the first version reported it as
missing. The catalog of names a template is written with stays the one that is offered; a second
catalog holds every `.twig` file by its path from each extension root it is under (1 512 on that
checkout), answered and never listed. Read the framework's loader before claiming a key set is
complete -- the claim is what turns a miss into an error.

**The docroot walk prunes `misc`, `js`, `css`, `assets`, `images` and `src` by name, and a theme
keeps `templates/misc/status-messages.html.twig`.** Moving the template list into that walk, to drop
a second walk of every `templates/` directory, lost fifteen templates -- and the theme hook count
did not move, because each of those hooks is also implemented somewhere else, so the instrument that
should have shown it read the same. The key count did: 1 594 before, 1 579 after. The walk now
enters those directories inside `templates/` and `components/`, and the loader says which
directories it does not enter, so a name passing through one -- `@mine/src/Generated/x.html.twig` --
is one nothing can rule out. A test holds that list to the walk's own two.

Measured: 0 findings over all 1 309 templates, every literal name in the project's own 131 resolved
by goto (35 of 35, p95 0.15 ms), setup unchanged (median 146 -> 145 ms), RSS +1 MB inside noise.

## A Twig template's variables are read where the array rendering it is written

`{{ data.institution_name }}` in the template a
`['#theme' => 'pdf__ofac_certificate', '#data' => [...]]` renders is `$data['institution_name']` at
that array, and the PHP engine already knows what that holds. So the seam states the array's shape
as data -- `RenderArray { key: "#theme", catalog, prefix: "#", own }` -- lowering files each such
literal as a `RenderSite` beside the calls Blade's `view()` makes, keys kept as written (`#data`) so
the existing `rendered_value` reads the entry unchanged, and the template finds its sites through
the theme catalog's places: the hooks whose template is this file. The file rendering it is lowered
beside the open buffer, as a Blade template's renderers are, and every step after the root is Twig's
own attribute rule applied with the PHP engine from outside any class: a key of an array, else a
public property, else the method of that name or its `get`, `is` or `has` getter.

Three things the first run found, each worth knowing before the next source of variables is added:

- **`owner_of_member` names the class that would own a member, not one that has it.** For a single
  class it answers for any name, so a walk trying a property, then `label()`, then `getLabel()`
  stopped at the property that does not exist. Ask `member_location` -- which also knows `@property`
  and `@method` -- whether the member is there.
- **`reaches_through_magic` is false for a refused member.** A private property outside its class is
  `MemberReach::Refused`, not `Magic`, and asking only for the second read it as visible. A template
  can read what PHP reaches from nowhere, which is `reach_of_member == Declared`.
- **`portable` reduced every written generic and shape to the class it is not.**
  `@param list<Node> $nodes` handed over as `nodes` arrived as nothing, so `{% for item in nodes %}`
  typed no `item` -- and the same was true of every Blade variable a docblock wrote that way. A
  builtin base with one argument is now a list, with two a map, a class with arguments `Applied`,
  and a shape a record, whose keys the file already interned.

Measured over the reference project's 131 templates, hovering every segment of every chain: 279
roots typed, every typed attribute checked by hand against the PHP it reads; hover p95 0.38 ms;
indexing, `check --jobs 1` and index memory unchanged.

**The 607 reads the first count blamed on element writes were blamed on the wrong thing.** Reading
`$build['#theme'] = 'x'; $build['#notice'] = ...` (`RenderKind::Writes`: the keys written on that
local in the same body -- not in a closure inside it, which has a local of its own -- each value its
last whole write) moved the count by five. The templates were under `src/Generated/.../twig/`, where
a hook's `path` put them, and the theme catalog is read from `templates/` directories -- so no key
placed them and no site was found for any of them. Drupal's registry names a template by its file
wherever it is, and `RenderArray::file_name` says so as data (`.html.twig`, `-` for `_`): with it,
**842** roots and **472** attributes typed, from 279 and 92. What is left is mostly what no render
site reaches: 1 045 reads in sections a page includes by a computed name (`{% include section %}`),
and about 250 handed over by an `include ... with {...}`.

**A computed include is said by the project, not guessed.** The page there loops over a list a PHP
builder returns from two class constants of template names, and nothing short of tracking string
values through a return and a Twig loop connects a section to the page -- so `[[twig.context]]` in
`phrost.toml` (or the editor's settings) says it: `templates = "@my_module/order/**"`,
`from = "@my_module/view--order.html.twig"`. `render_arrays_of` then adds the page's render sites to
the section's, a few pages up, so the workspace lowers them beside the section and the typer reads
them with no code of its own. `**` and not `*` there on purpose: a part included from a section with
`with {...}` and no `only` gets the section's whole context too. On the reference project, passed
through `initializationOptions` so its committed file was not touched: **1 634** roots and **672**
attributes typed (from 842 and 472), hover p95 0.63 ms; the unbound left in the sections are the
parts' own `total`, `customer`, `line_item` -- an `include ... with` hand-over.

**An include hands over its `with` and, without `only`, everything where it is.** The project's
templates are read once at start for template words already; the same read records what each
includes, embeds and extends by a literal name (`set_twig_includes`), and a template finds the ones
including it by resolving those names through the loader's catalogs. They are not indexed: the
workspace holds their text in the open set beside an open template, four levels up, with what
renders each lowered too -- the shape renderers already had -- and the typer reads each including
template as a template of its own. `extends` counts, because a parent renders with its child's
variables. Reading the text again on every keystroke would be a disk read per buffer per keystroke,
so a held template is read once and the watcher refreshes it. 1 741 roots and 740 attributes typed
(from 1 634 and 672), 400 roots left unbound of 3 113; hover p95 0.83 ms, the slowest 9 ms on a part
four includes deep.

## A Twig filter is declared by a construction in a dependency's body

`new TwigFilter('t', 't')` in an extension's `getFilters()` is the only statement anywhere that the
filter `t` exists and runs `t()`, and Twig's own sixty are in `CoreExtension` under `vendor/`, whose
bodies are never lowered. So lowering reads the syntax tree of the methods a rule names -- the
`twig` provider's `ConstructedEntry`: `getFilters`, `getFunctions`, `getTests` -- in every file,
keeping each construction of the class named (`KeptConstructionData`: the key, its range, and the
callable as written); the index resolves the classes against the file's imports and stores the
entries with the file, as it stores a property default naming a class. One string comparison per
method elsewhere, so a dependency costs what it did.

The callable forms that occur: `[self::class, 'm']` (Twig's own, 39 of them), `[$this, 'm']`,
`[Other::class, 'm']`, a function's name, `'Class::method'`, `null` for one the compiler writes in
place, and a closure. The last two run nothing a reader can be sent to, and goto lands on the key in
the extension instead.

And the table of filters the typer knew by heart went with it: what a filter returns is what the PHP
running it declares. What is left is Twig's rule for the ones whose result is the value or part of
it -- `sort`, `filter`, `default`, `first` -- because their PHP says `array` and only the rule says
it keeps the elements. On the reference project every one of the 1 388 filters and functions its
templates call is answered, and 15 more variables are typed through them.
