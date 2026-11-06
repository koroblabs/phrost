# Roadmap

The previous roadmap was eight phases with week estimates. It was written on day one and was stale
by day six, because the estimates were the only ordering it had and they were guesses.

This one is ordered by **evidence**. Every item names the measurement that put it where it is, and
the measurement can be taken again. Nothing here is scheduled; the next thing to build is the top of
the list, and the list is re-derived by running the four instruments.

---

## Where this stands

Measured on a production Drupal 10 checkout: 28 853 PHP files discovered, 1 751 of them the
project's own.

**What it answers.** Every capability the protocol has that this project has a use for, and four of
them arrived after the measurement above: `references` and `rename` reach into a framework's
configuration, a file rename takes the class with it, `textDocument/codeLens` puts the number of
uses over a declaration, and `textDocument/rangeFormatting` formats a selection. What is
deliberately absent is listed in `docs/comparison.md`.

**What it understands.** Counting everything, generated code included: 50 554 member accesses, **50
040 resolved (99.0 %)**, 331 receivers with no type, 111 declared `object` by the source itself, **0
unknown classes** and 0 incomplete hierarchies. Assignments 93.9 % typed; of 3 041 catalog keys,
none is unregistered and none looks misspelt. The 328 untyped receivers are a long tail nobody can
narrow -- `$row->id` off a `fetchObject()`, which is a `stdClass` at runtime -- and the 111 are what
a parameter written `object` says. **Assignments are 94.8 % typed** (re-taken 2026-09-24, after
compound assignments, `throw` operands, constant tables and callbacks were typed), and the shapes
left are element reads off untyped arrays, calls on `object` parameters and dynamic calls -- read
out one by one in `docs/08-index.md` rather than left as a number.

**What it costs.** A cold server is ready in ~1.6-2.3 s at ~265-280 MB; a warm one -- every start
after the first -- in ~0.7 s at ~143 MB, keeping all 30 965 stored rows without opening a file. The
cold figure is ~90 MB of freed parse arenas the system allocator keeps rather than returns; the two
obvious remedies were measured worse and the public relief API is inert on macOS 26 -- see
`docs/08-performance.md`. Budgets:

| request                  |      p95 | budget |
| ------------------------ | -------: | -----: |
| completion               |  0.06 ms |  12 ms |
| signature help           |  0.07 ms |  12 ms |
| goto definition          |  0.18 ms |  12 ms |
| hover                    |  0.25 ms |  12 ms |
| document symbols         |  0.04 ms |  12 ms |
| folding ranges           |  0.74 ms |  12 ms |
| semantic tokens          |  1.11 ms |  12 ms |
| inlay hints              |  1.65 ms |  12 ms |
| workspace symbols        |  2.73 ms |  12 ms |
| code actions at a cursor |  3.13 ms |  12 ms |
| code actions, every kind |  5.74 ms |  12 ms |
| diagnostics              | 10.66 ms |  30 ms |

`references` is on that table because it used to be the one row nobody looked at: 59.6 ms at the
median against everything else under 1.3, and 96 % of it a byte filter that read every file the
project owns to find which mention a name. The index had read them all already, so it keeps
[`WordSet`](../crates/index/src/words.rs) -- every identifier-shaped word a file holds, hashed and
sorted, **1.7 MB for 1.9 % of the index** -- and the sweep asks that instead. What is left is
analysing the candidates that genuinely mention it, spread over cores. 59.6 -> 2.8 ms at the median
and 80.9 -> 15.4 at p95, which is what made a code lens affordable at all.

Diagnostics is the one with room used, and its _max_ is now 10.8 ms, on the 3 011-line `Order.php`;
the 1 500-line generated file that was the worst case at 47 ms is under 10. The body walks that ran
once per position resolved are gone: what the tree fixes about writes, branches and guards is
derived once per file revision (`phrost-body`'s `facts.rs`) and a lookup is a few range tests per
candidate, with the old walks kept as the oracle a test compares every write at every position
against. What is left is the argument and member checks resolving every call, and the linter pass
the project's `phrost.toml` asks for.

**What it reports**, on the same checkout -- and reports for the whole project, not only the open
tabs: once the index is ready every file the project owns is checked in the background on four
threads, 1 711 files in 0.32 s here, with its own progress bar, and again whenever a file changes on
disk. `[diagnostics] project = false` in `phrost.toml` keeps it to the open buffers. `phrost check`
does the same sweep on every core -- 0.2 s on eight threads, 0.9 s with `--jobs 1` -- and prints the
same lines in the same order whichever it used.

| code                     | count |
| ------------------------ | ----: |
| `possibly-null-argument` |    67 |
| `possibly-null-return`   |    36 |
| `possibly-null-receiver` |    23 |
| `incompatible-argument`  |     2 |
| `psr4-mismatch`          |     2 |

Measured 2026-09-02, and smaller than the table it replaces for reasons on both sides: the earlier
stretch took `doc-missing-tags` 1 279 -> 837 and `incompatible-argument` 99 -> 81 by finding causes
rather than loosening rules, and the project has since fixed most of what it was told about, which
is what the instrument is for.

**`possibly-null-return` is new**, and it is the return's half of `?T where T expected` beside the
argument and receiver checks -- `function get(): string { return $this->maybe(); }` was reported
nowhere, because the type check sets nullable values aside for a null vocabulary that had no entry
for a return. Read one by one, the 36 are `preg_replace`, `array_shift`, `EntityInterface::id()`,
`batch_process()` and the project's own `?T` finders returned where `T` is declared: what PHPStan
reports at level 9. Three are worth knowing as the check's edges rather than its findings. A
property filled through a _setter_ under a null guard
(`if ($this->x === NULL) { $this->setX(...); } return $this->x;`) is one a reader of one file cannot
see through, and PHPStan reports it too. A `mixed ?? []` written into a typed property keeps the
declaration, because the vocabulary has no non-null `mixed` and a write that does not refine the
declaration is not read; PHPStan is silent there below level 9. And `reset($rows)` under
`if ($rows !== [])` is reported in null's words for a `false` the guard rules out, because `!== []`
is not yet read as a non-empty proof. Fifteen more were removed before the check landed, every one
the commonest way a property is filled in PHP --
`if ($this->cache === NULL) { $this->cache = $this->build(); } return $this->cache;` -- which a
slot's search withdrew from wherever a local's joined; the two are one root now.

---

## What the reference project is told, read for truth -- 2026-09-24

The other instruments ask what phrost fails to say. This pass asked whether what it _does_ say is
true, finding by finding, on the 538 the reference project gets: 369 `doc-missing-throws`, 147
`unused-declaration`, 20 `assumed-non-null` disclosures and one type finding (`DeleteForm`'s `?int`
into `load()`, which PHPStan reports too).

- **`unused-declaration`**, each checked against every file that writes the name: one false. A
  method a trait calls through `$this` was recorded on the trait alone, and the interface the using
  classes implement read as a contract nothing calls. Fixed; the other 146 name a same-named member
  of another class, or nothing.
- **`doc-missing-throws`**, each checked for a surrounding `try` whose `catch` names the class or an
  ancestor: one false. A `catch (\Throwable)` did not cover a class whose ancestry the index cannot
  walk up to `Throwable`. Fixed; the two others are right, since `catch (\Exception)` does not catch
  a `\Throwable`. None sits in an `{@inheritdoc}` method.
- **`assumed-non-null`** are disclosures of a written position, not claims about the code.

## Landed 2026-09-24: a matrix of PHP's magic

The instruments had run out of ranked work, so the next list came from asking one question per kind
of magic PHP has -- `__get`/`__set`/`__call`/`__callStatic`/`__invoke`/`__toString`, `@property`,
`@method`, `@mixin`, `ArrayAccess`, `IteratorAggregate`, generators, enums, `static`, `clone`,
class-strings, callable arrays and strings, closures bound to an object, destructuring, `throw` as
an expression -- in a scratch project of its own, at goto, hover, completion and the checks. 139
positions over three passes; **79 of 117** answered on the first, and **108 of 139** answer now.
`tools/php-magic-matrix.py` re-takes it, and what still fails there is the table below.

Eight commits, each found by one row of the matrix and priced by an A/B over five projects:

- **An accessor's declared return types the names it answers.** `__get(): Item` said nothing about
  `$bag->anything`. Only what it _declares_ -- a dispatcher's body is one branch's claim -- and not
  for a name the file asks `method_exists` about.
- **A subscript on an object reads `offsetGet`**, and only a subscript: iterating runs
  `getIterator`, and WordPress's cookie `Jar` answers differently from each.
- **`throw` where a value goes is `never`**, so `$repo->find($id) ?? throw new NotFound()` is the
  row and not nothing.
- **A destructuring target under the cursor** is the slot it takes.
- **A union holding `mixed` is `mixed`** to every sentinel question -- 136 WordPress findings that
  the subscript would otherwise have added. `docs/08-types.md`.
- **A class nothing can extend is refused what it does not declare**, so a `final class` with no
  `__toString` is the `TypeError` it is -- except a stub's, since `GMP` converts itself in C. And
  that exposed **every enum being refused as a `UnitEnum`**, fixed in the same commit.
- **A class answers a kind guard** (`is_callable` on a final class without `__invoke`), and
  `is_scalar` rules objects and arrays out.
- **`@property-read`** is what a refused write's message names.

Net on the corpora against the tree before them: reference project **0 new, 0 gone**; WordPress 2
new and 19 gone; Laravel 12 and 15; Symfony 4 and 4 -- every new one read, and all but WordPress's
two true by the declarations (the two are a guard written on another variable, below). Among them a
real Symfony bug, `array_map('strval', $note->attributes() ?? [])`, which PHP 8.5 refuses because
`attributes()` is an object -- visible because `SimpleXMLElement::__get` is read now.
`check --jobs 1` and every perf budget unchanged.

### What the matrix left, and why

Not built, with how often the shape is written on the reference project where that was counted:

| shape                                                        | state                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `[$var, 'method']`                                           | **refused by design** -- a pair of a variable and a string is data as often as a callable; `[$this, 'm']` (246 sites) and `[Foo::class, 'm']` (9) are read                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `'Foo::bar'` and a project's `'App\fn'` as callables         | **refused for types** -- a string reaching into the project is not the author choosing a declaration, and WordPress's `esc_sql` says `string                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | array` for every argument at once. A runtime function's name (`'trim'`, `'strlen'`) is read since 2026-09-24. Goto on either still answers nothing; 0 sites on the reference project |
| `Closure::fromCallable([...])`                               | **read** since 2026-09-25 as the callable it wraps, so invoking it gives the wrapped return rather than `Closure::__invoke(): mixed`. A pair over a variable (`[$o, 'm']`) stays refused, as it is everywhere                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `compact('x')`                                               | **read** since 2026-09-24 as an occurrence of `$x`, so references, highlight and rename include the string and inline-variable is refused; the string itself is a cursor on the local (hover, goto, rename). A `compact()` that spells every name no longer silences `unused-variable`/`unbound-variable` for its whole scope, and `compact('typo')` is reported as PHP warns about it: WordPress +1 true `unused-variable`. 1 site                                                                                                                                                                                                                                                                                                                                                                 |
| `require __DIR__ . '/x.php'` goto                            | open                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `@param-closure-this`, `@mixin T`, a generator's send type   | open, not written in the project's own code; `@mixin T` is written once in four corpora (Laravel's `HigherOrderCollectionProxy`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `array_key_first`/`array_key_last`                           | **read** since 2026-09-25 through a new `ArgumentKey` branch (cache 28): a bare `@return K` bound by a parameter's _keys_ had no branch, fell back to `string                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | int                                                                                                                                                                                  | null`, and now answers `?string`over`array<string, Row>`and`?int`over a list. The prelude's`K : null` conditional is left as vendored -- the null is set aside as absence, so the branches agree |
| a bare `: array` refined by what the body returns            | open, and the design is the work: `Reason::rejectionCases(): array` (with `@return array`) returns `array_filter(self::cases(), ...)`, so a caller's `foreach` over it is untyped -- the reference project's `$case->name` in `NoteController`. `ReturnedRow` stores only member chains, and a closed file's body is not read at request time (§1.2), so this needs a stored description of a returned _call_ with its container argument, measured for rows and memory before it lands. **Priced 2026-09-25 and not worth it yet**: of 1 056 bare-`array` methods in the reference project's own code, 436 return a local (render arrays, mostly), 346 a literal, 126 a member chain, and only **25** a call this would describe (`array_map` 12, `array_filter` 6, `cases()` 4, `array_values` 3) |
| several tags on one docblock line                            | **not a gap**: three matrix cases wrote `/** @template T @param class-string<T> $id @return T */`, and PHPStan reads only the first tag there too (`method.templateTypeNotInParameter`, dumped `object`). The cases were fixed, not phrost; two of the three then passed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `(object) [...]` as `stdClass`                               | **read** since 2026-09-25: an array, a scalar or a null becomes a `stdClass` (PHP 8.5.10 builds one around each), an object nothing can make null stays itself, anything else is `object`. WordPress +1 true `incompatible-return` -- `convert_to_screen()` documents `WP_Screen` and returns `(object) array(...)`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| a global constant's type (`const X = 5`, `define('X', 's')`) | **read** since 2026-09-25: the value's type is stored per file (`ConstantRow`, cache 29, in the dump so `cold == warm` covers it) and a `ConstantAccess` is a path root. WordPress lost 8 false `incompatible-argument` on `str_replace(..., ABSPATH)` and gained 2 true `possibly-false-argument` in `pclzip`. A value that names nothing falls back to a `@var` above the declaration, and the 33 engine-filled constants the stubs write as `const PHP_EOL = PHP_EOL;` carry one (types taken from PHP 8.5.10): Symfony +1 true -- `$input ?? (\defined('STDIN') ? \STDIN : fopen(...))` into a `resource` property, where `fopen` can be `false`                                                                                                                                                |
| `-1`, `-$x`, `~$x`, `$i++` as values                         | **read** since 2026-09-25: `-`/`+` are `0 - $x` (a new `Signed` node, since PHP has no negative literal and `$i = -1;` was untyped), `~` an int, a postfix increment the value before and a prefix one `$i + 1`. No finding moved on five projects                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `array_combine`'s keys                                       | **refused** since 2026-09-24 rather than read wrong: a branch cannot yet say that a key comes from one argument and a value from another                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `SplObjectStorage<O, V>` iterated                            | **read** since 2026-09-25: a value's arguments are read at the slot its class's tags name (`@template-implements SeekableIterator<int, O>`), so the loop yields `O` keyed by `int`, and a subscript with an expression key runs `offsetGet` and gives `V`. Symfony -2, both `TraceableEventDispatcher` reading the stored array as the listener                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `Foo::nope()` with only a `__call`                           | **reported** since 2026-09-24 -- outside the class's hierarchy only, where PHP 8.5.10 refuses it; inside, `__call` runs                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| a guard written on a _second_ variable                       | **refused**: `if (is_object($m)) { $o = $m; } if ($m && !$o)` needs what one local's truthiness proves about another, which is a relation between two names and not a type either of them holds -- a path-sensitive analysis, priced at two WordPress findings                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |

Closed on the second day, each with a test and an A/B over five projects: a template nothing
supplied no longer renders as a class called `T`; `new Coll([new Item()])` binds from the literal;
an element written into a parameter declared as a class keeps the class (19 `WP_REST_Request`
findings); an object of a closed class invoked, subscripted or made a string is `impossible-use`; a
constant table states its element and key (125 `self::X[...]` reads on the reference project); a
callback is read by what invoking it gives, a reduce's accumulator keeps its own hint, and a spread
passes its elements (7 Symfony findings). The matrix reads **180 of 222** after its fourth pass.

The sixth pass came from PHPStan's level 8 findings on the reference project read site by site, and
closed: compound assignments typed by their operator; a table of `Foo::class` values, with `new`
over it an instance of the one class or a union of several; `method_exists` read as a check that
leaves when it fails and on a class named by a value (4 findings in Laravel and Symfony); a runtime
function's output parameter left unjudged on the way in (3 in Symfony); and a plain
`@return list<U>` bound the way the stubs' conditional spelling is.

The fifth pass (generators, invokables, narrowing on properties, `never`, `catch` unions, iterable
keys, the array functions) found three things and closed them: an invokable object handed to
`array_map` is read through its `__invoke`; `array_column` is a contract branch of its own,
`ArgumentColumn`, reading the field or property the call names (32 calls on the reference project);
and `Held::one_of` takes a nullable operand apart, so `Other|?Doc` -- the degenerate form no guard
can narrow -- is `Other|Doc|null`. **221 of 263.**

Positions the matrix confirmed rather than gaps: `get_class($x)` and `$x::class` render `string`
rather than `class-string<X>`; a first-class callable renders `callable(...): T` rather than
`Closure`; `json_decode` and `unserialize` are `mixed`; a generator's `current()` is `?TValue` by
its stub.

## Landed 2026-09-22: what a wrong answer costs a layer away

Ten commits, and the shape they share is worth more than any of them: **every one was found by an
instrument rather than by reading code**, and in four of them the finding was nowhere near the fix.

- **A join reduced `?Row` beside `Row` to `Row`**, on the grounds that both name one class -- and
  the reduction had been covering for four rules about what the path that _skipped_ a write proved,
  each of which could answer "nothing" and never be noticed. Fixing it exposed all four at once.
  `docs/08-types.md`.
- **`possibly-absent-key`**, the read PHPStan reports and phrost had nowhere to say: twenty of
  twenty-eight `offsetAccess` findings on the reference project are keys read straight into an
  `array<string, mixed>` argument, where there is no _use_ to report. Asked for rather than always
  on.
- **Go-to-type-definition on `Rule[]`** answered nothing, because the rule was written as "empty
  where the index cannot name a class" and a list is not that. Found by `capability-audit`, which
  also turned out to be counting a class named in a docblock sentence as a reference.
- **Extract-variable wrote an unparseable `elseif` chain.** One of 257 edits, found by applying
  every action a real project offers and parsing the result.
- **A local is a different value in every scope, and one set of names was not** -- so a widget that
  writes `$element['#prefix']` in `formElement` silenced every subscript check on `$element` in
  every other method of the file. Twice in one afternoon, in `write_targets` and in `tested_keys`.
- **A guard on a property reached the call and not the local the call filled**, because the chain
  walk asks `slot_value` about the receiver and `resolve_path` asks about the path as a whole.
- **Three shapes lowering said nothing about**, found by a sweep that assigns one of every
  expression PHP has to a local and passes each to an incompatible parameter: an interpolated
  string, the error-control `@`, and the magic constants. The last removed **157 false positives**
  from an eight-line arm, because `str_replace('\\', '.', __CLASS__)` could not pick the branch of
  its conditional return -- which is the lesson: a shape lowering is silent about costs what
  everything downstream of it would have decided, not what the shape itself is worth.

What the same sweep _refused_, with the measurement for each, is in `docs/08-index.md`: a global
`const`'s type, unary `-`, `~`, `$x++` as a value. And what is left of the PHPStan level 8 gap is
below.

## Every instrument reads green, 2026-09-22, and what that leaves

A full sweep, because the PHPStan queue had come down to positions and the way to re-rank is to ask
the others. Each of these is reproducible by the command beside it.

| instrument                              | reading                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `phrost coverage`                       | **99.0 %** of member accesses resolved; 0 unknown classes, 0 unregistered catalog keys                                                                                                                                                                                                                                                                                                  |
| `phrost check --ratchet`                | held on all five corpora                                                                                                                                                                                                                                                                                                                                                                |
| `just perf`                             | 17 of 17 budgets, most at single-digit percent                                                                                                                                                                                                                                                                                                                                          |
| `tools/capability-audit.py`             | **20/20 answered and 20/20 useful** on every row but a bare-cursor `codeAction`, whose provoked twin is 20/20 -- on two projects                                                                                                                                                                                                                                                        |
| `just compare --reference intelephense` | definition **627 agree, 0 reference-only**; hover **646, 0**; completion 200 sites, both non-empty at 200                                                                                                                                                                                                                                                                               |
| `tools/drupal-style.sh`                 | 247 files reprinted, **0 phpcs errors introduced**, 5 removed                                                                                                                                                                                                                                                                                                                           |
| every invariant probe in `tools/`       | passes: hint == hover, reference list == highlights, completion card == hover card, signature == argument, hierarchy both ways, selection growth, rename round trip, navigation round trip, checker == server, edited == fresh, formatter through two front doors, disk changes, file moves. **Re-taken after the ten commits above**, because lowering is exactly what could break one |
| `tools/code-actions-apply-cleanly.py`   | **256 of 256** -- the one failure it found is the `elseif` refusal, fixed                                                                                                                                                                                                                                                                                                               |
| `just corpus`                           | still unavailable; no sibling checkout on this machine                                                                                                                                                                                                                                                                                                                                  |

What the sweep found was one real defect (extract-variable writing an unparseable `elseif` chain)
and one real gap (`typeDefinition` answering nothing for `Rule[]`), both fixed; and one instrument
correction, the eleventh to `capability-audit`, which was counting a class named in a docblock
sentence as a reference.

### What is left of the PHPStan level 8 gap, and why

382 findings "of the rest", and it is four things rather than a queue:

- **96** `Cannot call method fetchAssoc() on StatementInterface|null` -- Drupal's `->query()` and
  `->execute()`, which a framework contract says never hand back null. A position, and the one
  `assumed-non-null` discloses at the sites where it could matter.
- **73** `Call to an undefined method object::getAddress()` -- the project's own `object $order`
  parameters, 32 of them in one _generated_ file. Nothing can infer a class the source declines to
  name, and `phrost coverage --without` exists because this is the corpus rather than the server.
  What would close it is the author typing the parameter, which is also what PHPStan is asking for.
- **~25** `numeric-string`, and a handful of integer ranges and `non-empty-array`. Refinement types
  phrost does not model; `BuiltinTy` is closed by convention and adding to it is a semantics change,
  so this is a piece of work rather than a fix.
- **10** other `offsetAccess` shapes -- a positional `array{}|array{0: string, ...}`, an
  `array|null` container which is `possibly-null-subscript`'s question, a computed key typed as a
  union of literal strings. The other 18 are `possibly-absent-key`, which a project opts into.

**Re-read 2026-09-24**, site by site rather than by count, over the 381: two were phrost's and are
fixed -- a name inside its own `/** @var T $x */ $x = f($x);` read as the annotation, which hid the
`?int` `DeleteForm` passes to `load()`; and a compound assignment (`$ok &= $step`) invisible to
typing, which hid a `bool` method returning an `int`. The rest divide as before: `object` parameters
(every `method.notFound` and `property.notFound`), refinement types (`numeric-string` 25,
`int<1, max>`, `non-empty-array`), an entity query's `execute()` as `int|array`, Drupal's
`getFileUri()`/`uuid()` contract, a shape's required key against `array<string, mixed>` (a claim
`possibly-absent-key` makes opt-in), and one PHPStan false positive --
`!$uid && ($uid = $this->uid())` leaves an `int`. Open from the same read: `exec($cmd, $output)`
refuses the string `$output` held, which PHP 8.5 overwrites. A table of `Foo::class` values is typed
since, and `new` on it answers when the table names one class -- `Order::fromNode()` names ten, and
no one of them is the class every use means, so it still answers nothing there.

## Two instruments have run out, and that decides what comes next

**The corpus stopped ranking work.** 342/360 on PHPantom's, 364/379 on the borrowed set, 247/253 on
the third. Of the 40 failures left, **the great majority are positions this project has taken in
writing** -- union leniency above all, return type from a method body, static-vs-instance. Reading
the failures rather than the score is the only way to see that, and it is now the first rule under
_How to pick a piece of work_.

The last pass over them (2026-09-02) read all 53 that were failing and closed 13 -- a call declared
`never` ending a branch, wherever in the branch it sits; PHP 8.5's `|>`, lowered as the call it
makes; a constructor's by-reference parameter; completion at the end of an unfinished
self-assignment; a `@method` tag's conditional return and its `class-string<T>` template; the
nullable return check above; and a harness that scored a correct silence as a miss. What is left
divides three ways. **Positions**, twelve: union leniency and De Morgan (4), static-vs-instance (2),
`array<T>` keys (2), an unresolvable branch rendered as `mixed` rather than as nothing (2), a
Laravel-specific patch (1), a `new Money()` inlay the value already states (1). **Wording or a
fixture that is wrong**, nine: a completion label spelled with its namespace, an old message text, a
`use` keyword offered after `new`, a relative class name the fixture meant as absolute, hover on a
method declaration, a hover section layout, and two weak-mode coercions phrost is right to accept.
**Designs**, under _Blocked_ below and not to be started blind: the travelling array shape (2), an
iteration that yields a class rather than a type (2), a conditional assert about an argument, a
callback parameter narrowed from its call site, `@implements` binding an implementer's parameters, a
conditional return tested against an argument's _contents_ (`is array<Foo>`), and two small ones --
`in_array($x, $list, true)` narrowing and superglobal key completion.

Two of them turned out not to be positions, and both were read out of the failure list rather than
the score. `signature_help` was 15/17 because mago drops the last statement of a file, so the
feature was dead for exactly as long as somebody was typing the call -- fixed, and the category is
17/17. `narrowing` was 56/66 for a position that turned out to be held only half way -- fixed, and
the category is 61/66; `catch_clause` and `assignment` came with it and are 100%. `type` went 23/33
to 26/33 and `generics` 47/53 to 49/53 once the generics lever was re-priced -- see below, where two
of its three causes turned out to be small. `type/*` and `narrowing/*` were then probed shape by
shape, and the two gaps found there were both refused on a count: see _Typing `->value`_ and
_Narrowing a negated `&&`_ below. The `narrowing` failures were then read the same way, and _that_
was not a position either -- see below.

**Coverage is nearly out too, and this is the newer finding.** Tracing all 383 distinct untyped
receivers back to what produced them:

| producer                                     | occurrences |
| -------------------------------------------- | ----------: |
| a project call with no declared return type  |         205 |
| `foreach` over an array with no element type |         191 |
| other                                        |         135 |
| a closure parameter over an untyped array    |          38 |
| a cache item                                 |          22 |
| an array element                             |          20 |
| a database row                               |          16 |
| `json_decode`                                |          12 |

**About two thirds is the project's own missing declarations**, not a gap here -- probed rather than
assumed, since `@return X[]`, `array<int, X>`, `list<X>`, `@param X[]`, `@var X[]`, closure
parameters over typed arrays and `$arr[0]` all resolve today. What misses is a bare `@return array`
and `json_decode`.

So the ranking now comes from **counting shapes in real code**, which is what produced the render
array item below.

---

## Against intelephense

Same project, 40 files, 530 positions, licence key installed. Full method and result in
`docs/comparison.md`; the short version:

|                              |     phrost | intelephense |
| ---------------------------- | ---------: | -----------: |
| RSS once ready               | **270 MB** |     1 720 MB |
| hover p95                    | **0.9 ms** |       5.5 ms |
| completion p95               | **3.8 ms** |      10.3 ms |
| documentSymbol p95           | **0.2 ms** |       6.5 ms |
| definition p95               |     1.1 ms |   **0.7 ms** |
| goto: they answer, we do not |      **7** |              |
| outline entries              |      1 207 |    **1 646** |

Memory is the decisive one and the gap widens as files open. The three places we lose are named and
are work items, not opinions. The bootstrap row is **disqualified**: the harness starts intelephense
from a persistent cache and phrost's is stamped with the build that wrote it, so a fresh binary
starts cold. Comparing the two conditions is not comparing the two servers.

---

- **A free function's result as a receiver.** `helper()->` is marked "not read yet" in the
  completion scanner, and it stays that way: **3 sites** in the reference project's own modules
  (`reset()` twice, `finfo()` once), 60 in all of Drupal core -- 25 of them `reset()` and 6 `end()`,
  which hand back an array element and name no class worth completing. Symfony has 439, but 331 of
  those are its own configuration DSL (`service()`, `inline_service()`, `events()`) written in
  `config/*.php`. Measured 2026-09-12 and refused.

### Where this stands after the 2026-09-11 pass, measured

Every number here was taken again at the end of it, not carried forward.

|                                      |                                                                                        |
| ------------------------------------ | -------------------------------------------------------------------------------------- |
| reference project                    | **971** findings: 840 `doc-missing-throws`, 131 `unused-declaration`, no type findings |
| WordPress / Laravel / Symfony / Slim | 3 945 / 6 352 / 8 505 / 14                                                             |
| the vendored stub set                | 46, unchanged by a day of type-layer work                                              |
| coverage                             | 98.8 % of member accesses resolved, 93.2 % of assignments typed                        |
| index                                | 93.8 MB                                                                                |
| budgets                              | 17 of 17 met                                                                           |
| fixture corpora                      | 343/360, 361/379, 247/253                                                              |

**What a whole day of it produced, and where each piece came from.** Two bugs came from the
reference comparison below; three came from a person reading output and pushing back on it. The
second source has the better hit rate and is the one that found the _class_ of bug worth naming:

> A rule that hides or declines something, justified by an argument nobody measured.

Four were audited once that was named. Magic methods in completion: the argument was that PHP calls
them rather than a person, and `->__toString()` is written by hand 51 times in Drupal core and 59 in
Symfony -- **wrong, fixed**. A parameter's default rendered `= ...` because the index kept only that
there was one, which also made "implement the required methods" stop short on 21 % of the reference
project's interfaces -- **wrong, fixed**. Static methods offered after `->`: 0 sites in their own
modules and 34 in all of Drupal core -- **right, kept**. Promoted parameters, which declare a
property as well -- **right, kept**. Two of four held, which is about the rate worth expecting and
is why each has to be counted rather than argued.

### The third instrument ran out the same day, and it is the one that had been working

`tools/compare-lsp.py` against intelephense is what the corpus section above calls for -- somebody
else's answers to the same questions -- and on 2026-09-11 it was the only instrument still ranking
work. It found two real bugs in one afternoon, and then it ran out too.

Over 40 files and **2 836 sites**, goto where intelephense answers and phrost does not went **18 ->
7 -> 3** and hover **17 -> 6 -> 2** across three changes:

- a call whose receiver is a **union of classes** carried no return into the next call, so every
  PHPUnit mock chain stopped after one hop -- 11 of the original 17;
- a **wrapper's body** was read for `new Foo()` and not for `return $this->a()->b()`, so Drupal's
  query builders died at the second hop -- 4 more;
- and the _outline_ row, which read 263 against 655 and looked like the worst number in the report,
  is 14 local variables and a namespace per file. intelephense lists locals in a document's symbols
  and phrost does not. A policy, and the same shape §8.2 records for that document's other rows.

What is left is three sites of 2 836, and completion's remaining 156 labels are `__construct` (822
offers), `__sleep` (451), `__wakeup`, `__get`/`__set`/`__isset`/`__unset`/`__clone`/`__toString` --
magic methods, which §8.2 already calls a position rather than a gap -- and `create`/`load`/
`loadMultiple`/`preCreate`/`postLoad`, which are **static** methods offered after `->`. Both are
things phrost declines on purpose.

So all four instruments named in CLAUDE.md §9 are now saturated on this project, and the next piece
of work has to come from somewhere else: a report from somebody using it, a second reference server,
or a dialect no corpus here speaks.

### Re-measured 2026-09-11, and both are still out

Taken again because a claim that an instrument has run out is worth nothing if the instrument is not
re-run. Coverage on the reference project now reads **98.8 %** of member accesses resolved and
**93.2 %** of assignments typed. What is left of the assignments is reading elements out of untyped
arrays -- Drupal render arrays (`$element['#value']`, 31 sites) and decoded JSON
(`$data['email'] ?? NULL`) -- which are `mixed` in fact and not for want of inference.

That settles one item that had been sitting here on interest rather than on evidence: **deriving a
return type from a method body would not close this gap**, because the gap is array shapes nobody
declares rather than returns nobody wrote.

The corpora were re-baselined the same day -- WordPress 3 920, Laravel 6 906, Symfony 8 439, Slim 14
-- and the **four largest numbers in them were read site by site**. Every one is a true positive on
the corpus's own loose code: Symfony passing `str_replace`'s `string|array` into `substr` because
the enclosing function declares no parameter type, `createFromFormat` used with no guard at all,
WordPress dereferencing `get_userdata()` and `_get_list_table()` unchecked. Which is the same
finding as the corpus section above, one level up: reading the failures rather than the score is
what says whether a number is work.

## The largest single gap was not in PHP at all

`phrost coverage` reads 98.6 % of member accesses resolved, and the remainder is a long tail: 499
untyped receivers over **312 distinct expressions**, no cluster bigger than eight. That instrument
has run out the way the corpus did.

What it cannot see is the half of a framework project that is not PHP. Counted on the reference
checkout, in the project's **own** modules alone: **1 414 references in 72 YAML files**, every one
naming something already in a catalog or the def-map --

|                                                 |     |
| ----------------------------------------------- | --- |
| `'@service.id'` in `arguments`, `calls`, `tags` | 745 |
| `class:` / `factory:`                           | 280 |
| `_controller:` / `_form:`                       | 144 |
| `route_name:` / `base_route:` / `appears_on:`   | 101 |
| `_permission:`                                  | 81  |
| library dependencies                            | 37  |

None of them navigated, and the seat had been reserved since the framework seam went in: the Zed
extension routes YAML here on purpose, and `flow::foreign_languages` pinned that holding one must
cost nothing until something read it.

**Built, and measured the same way: 1 338 of 1 351 answer, with 0 false "unregistered".** The
remaining thirteen are the census regex rather than the server -- `factory: ['@id', 'method']`
counted twice, a controller's method captured as its own name.

Three things about the shape are worth keeping. Nothing new is indexed: the catalogs were already
filled at bootstrap and a class name is a class name, so what was missing was the _statement_ of
which key means which table -- which arrives from the seam as data, so Symfony is a file rather than
a rewrite. The buffer is held as **text and nothing else**, never registered in the VFS, because the
two index tiers are updated from the one string a document event carries and a `services.yml`
declares no PHP. And a reference is one scalar on one line under one key, which is what a line
scanner gives -- so a configuration buffer costs a scan of the line the cursor is on rather than a
parse of the file, and a PHP keystroke costs nothing at all.

Index memory 89.0 MB either way, `check` unchanged at 94 findings, all seventeen budgets green.

Completion followed on the same scanner, and the ids are the point of it: nobody remembers which of
1 856 service ids builds the manager they want. `arguments: ['@order']` offers 13,
`route_name: app_core.` offers 59, `- core/` offers 79, `_permission: 'access '` offers 76. A class
is not offered -- it needs a qualified-prefix search of the def-map, which the PHP path reaches
through the open file's imports and a configuration buffer has none of, and a person writing
`class:` copies the name rather than typing it.

**The report is built and reuses the PHP one's decision rather than restating it.** A key nothing
registers is the same mistake in either language, so `catalog::reported` is one function with two
callers: a closed catalog proves the line cannot run and is an error, an open one can only say a
name looks like a misspelling and is a warning, and a name merely absent from an open catalog is
silence -- which is what keeps 21 runtime-generated `entity.*` routes quiet.

Measured the way a report has to be: **0 findings across all 72 of the project's own YAML files**,
and sabotaged in memory to prove the silence is not the check failing to run -- `'@no.such.service'`
errors, `'@order.managr'` errors with the suggestion, `app_core.accountz` warns,
`totally.absent.thing` says nothing.

---

## Next, ranked by counted evidence

### 1. Render arrays -- the element id is done, and so are its documented keys

7 633 `#`-prefixed keys in the project. `#type` 1 676, `#required` 1 027, `#title` 684.

**The id resolves.** `'#type' => 'textfield'` goes to `Drupal\Core\Render\Element\Textfield`, hovers
as that class, and completes from the catalog -- 82 ids, read from the positional
`#[FormElement('...')]` attribute. Both things this entry once called blockers are gone: an array
element _is_ a catalog site (`ReturnContracts::bind_array_key`, `phrost_body::keyed_element_at`),
and positional attribute literals _are_ lowered, gated on a name set derived from the providers' own
rules.

**The keys beside it complete.** `'#ma|'` inside that array offers `#maxlength`, `#size`, `#title`,
`#required` and forty more, each with what it holds -- `bool`, `string[], read-only` -- and the
sentence beside it on resolve. Nothing declares any of them in PHP: `getInfo()` returns defaults and
most properties have none, so the only statement they exist is a bullet in the class's own docblock,
and the walk goes up the `extends` chain because a leaf documents five and inherits the sixty
everybody writes. See `elements::DOCUMENTED_KEYS` for why the reader anchors on the bullet rather
than on a heading -- `Textfield` writes `Properties:` and `FormElementBase` writes a sentence, so a
heading would have kept the rare keys and lost every common one.

The cautions this entry recorded all held, and none needed a special case:

- **671 of the 1 676 `'#type'` sites are not render arrays.** A project-local DSL writes
  `'#type' => 'Float'` and `'#Object'`. Measured after the fact: those arrays offer _nothing_,
  because `Float` is not an element id -- the catalog rules them out with no rule of its own.
- **The catalog is declared open**, so completion, goto and hover work and only the diagnostic is
  withheld -- exactly as route names already do.

`#theme` is still refused: 53 of its 63 values are template suggestions that deliberately name a
hook which does not exist.

What is left here is the _report_ -- `'#titel' => ...` in an array whose element is known is a
misspelling nothing catches. Held back for the same reason the id's report is: the key set is a
superset of what any one element accepts (a render array legitimately carries `#prefix` alongside
whatever the element adds), and a false positive on a form is worse than a missing one.

**Measured 2026-10-09, and the answer is still no.** Of the 167 distinct `#` keys the reference
project's own modules write, 75 appear nowhere in core or contrib -- and **none of them is a
misspelling**. They are theme variables handed to a template through the render array (`'#order'`,
`'#line_item_id'`, `'#customer_panels'`), a schema DSL's own vocabulary (`'#not_empty'`,
`'#min_items'`, `'#on_inactive'`) and `'#path'` 481 times. Within an edit distance of two of a known
key sit `'#nid'` (`#id`), `'#date'` (`#data`), `'#lang'` (`#long`), `'#mail'` (`#max`), `'#notes'`
(`#node`) -- every one a deliberate name that a nearness rule would report. A report built on
nearness would be wrong at every site it spoke on this project, and a report built on the element's
declared key set cannot see that a `#theme`'s variables are legitimate. Refused until a render
array's hook can be known to accept a variable, which is the template's `{{ }}` and not PHP.

### 2. `$container->get(SomeClass::class)`, and the completion positions

This entry used to read "seven goto positions, four of them hover too, three of them plain local
variables". Re-run against intelephense with the licence key installed (39 files, 682 sites) it was
**six**, three of them still those locals -- so the doc that called them closed was reading an older
sample, and the ranking above it was reading the doc.

The three locals were **one bug**: a destructuring target writes through no variable, so every name
`['id' => $id, ...] = $row->getSource()` binds had no declaration at all. Fixed; the same run now
reads **var 266 agree, 0 reference-only**, and goto's total loss is 6 -> 3.

`$this->container->get(X::class)` was the other shape, and it is now stated as a contract -- 6 -> 1,
the one left being a site the harness renders as `?->fields`, where `?` is its own placeholder for a
receiver its chain regex could not capture.

What made it worth doing is **not** visible to `phrost coverage`, which is the lesson to keep.
Symfony declares `get()` with a conditional return keyed on a type test over a template, nested two
deep, whose true branch is `C|object` -- so even read perfectly a member lookup would have to find
the method on `object` and decline. `ArgumentClassReturn` states the same fact in the form this
index holds, and the guard beside it had to widen from `object` to `?object`, which is the same
statement with the null the _second_ argument opts into. 295 of the 296 calls on the reference
checkout write one argument. The instrument says nothing about any of it: coverage counts the
project's **own** member accesses and the project writes the shape twice, so it reads 49 063 / 98.8
% either way. The 296 are in the core and contrib that a person opens all day, which is the mirror
of the note in `docs/08-types.md` about corpora being unable to price an application idiom -- here
it is an application's instrument unable to price a framework one.

The **27 % outline shortfall was never a shortfall**: 86 % of the difference is intelephense listing
every `$var` bound in every method as a child of that method -- seventeen in one `buildForm` -- plus
a `Namespace` node per file and an anonymous class's members. An outline drives the breadcrumb and
the symbol picker as well as the scroll, and flooding all three is not completeness. A kitchen-sink
probe found nothing genuinely missing. Refused, not owed.

The definition p95 is 1.1 ms against their 0.7 ms, both an order of magnitude under a 12 ms budget,
and chasing it would be tuning a row nobody waits on.

So what is left of this entry is **completion: 181 positions, both non-empty at 170**. The eleven
are small enough to open one at a time rather than rank -- and opening them needs the comparison run
again, which needs intelephense installed (`npm i -g intelephense`; the licence key is already in
`~/intelephense`, and without it eight paid capabilities read as `false` and the table lies).

**Re-run 2026-10-09, and the entry is closed.** 39 files, 636 sites: goto and hover **0** where
intelephense answers and phrost does not (the 11 goto "differ" are the two servers' own stub files
for `\InvalidArgumentException` and friends). Completion 173 sites, both non-empty at 172, and what
only intelephense lists is not owed: the eight items at `$this->orderManager->` are `protected`
methods of a trait the class uses, offered from outside the class where PHP refuses them, and the
one at a builder chain is three deprecated superglobals. The phpantom table's
`$this->select('…', '…')->fields` row is stale the same way -- 62 items there now.

### 3. The largest remaining coverage gap is not ours, and here is what it is worth

`phrost coverage` on the reference project reads **97.9 % of member accesses resolved**, with the
whole remainder in two rows: 637 `untyped receiver` (1.4 %) and 232 `declared object` (0.5 %). The
second is the cause of much of the first, and it is **one module**: 231 of the 275 `object` types in
the project's own code are in five files of `shop_order_documents`, four of them under
`src/Generated/`. A generated document builder declares `@param object $order`, so
`$order->getSupplier()` answers nothing, so `$supplier` is untyped, so all sixteen `$supplier->...`
sites in that file are untyped too.

Measured rather than argued, by annotating that one file in a clone and reverting:

|                  | `object`        | the real class      |
| ---------------- | --------------- | ------------------- |
| resolved         | 44 918 (97.9 %) | **45 149 (98.4 %)** |
| untyped receiver | 637             | 515                 |
| declared object  | 232             | 118                 |
| missing member   | 16              | 21                  |

**+231 resolutions from one annotation on one file**, and the five new findings are real: the
builder calls `isDiscountedPrice()` and `getPriceTerms()`, which live on `ClearanceProduct` and
`PerishableProduct`, on a value declared as the abstract `RefurbishedProduct`. No `__call` anywhere
in that hierarchy. So `object` was not only costing resolution, it was hiding five narrowing
assumptions nothing states.

Nothing here is phrost's to build. There is no docblock to read -- the tag agrees with the hint --
and recovering a parameter's class from what its callers pass is whole-program analysis, which this
project refuses for the reasons under **Deliberately refused**. What is worth keeping is the number,
so that the 1.4 % row is not mistaken for an inference gap: it is a fixable annotation in the
project under test, and the fix belongs in whatever emits that file.

The catalog row has the same shape. Of 712 keys an open catalog cannot judge, **670 are eleven
names** -- `'#type' => 'String'`, `'Object'`, `'Float'`, `'Integer'`, `'Boolean'`, `'Date'` -- a
schema DSL of the project's own that borrowed the `#` convention. `looks misspelt` is 0 across all 2
831 keys, so none of the eleven is near a registered element id and nothing false is reported.
Judging them would need to know that an array is not a render array, which is the "sometimes a key"
mistake `docs/08-index.md` records.

### 3b. What the corpus's 37 failures actually are

`just corpus ../phrost-old/fixtures` read **342/379 (90 %)** when the 37 were first read one by one.
Reading them found two real defects, both since fixed -- a receiver's leading separator and a
parenthesised `new` -- and the score is **344/379 (91 %)**. The rest do not describe 9 % of missing
work.

| group                                                                  | count | what they are                                                          |
| ---------------------------------------------------------------------- | ----- | ---------------------------------------------------------------------- |
| diagnostics, phrost said something else                                | 9     | a different _wording_ from phrost-old's, same finding                  |
| diagnostics, phrost silent                                             | 6     | two checked: one is phrost being right, one is a real narrow gap       |
| type: return inferred from a body                                      | 6     | the feature measured and declined below -- ceiling 1 of 508            |
| narrowing                                                              | 5     | **2 are unsound fixtures**, 2 have a population of 1, 1 is corpus-only |
| completion, foreach, pipe, member access, generics, conditional return | 11    | **2 were real and are fixed**; see below                               |

Four findings worth keeping, and the first two were the point of reading them.

**A receiver's leading separator was being thrown away, and `\Drupal::` completed nothing.** Found
from `member_access/new_no_parenthesis` and the two completion fixtures beside it, all of which are
top-level code whose marker is the last thing in the file -- which is what made them look like a
parser-recovery problem and is not what they were. `docs/08-server.md` has the whole of it; the
measured effect is `declared object` 232 -> 115 and resolved receivers 97.9 % -> 98.5 %, from
keeping one character.

**`(new Foo())->` was a deliberate miss with 42 sites in the project's own code.** goto and hover on
the same expression already worked, so only the incomplete-buffer scanner was blind.

**`diagnostics/function_argument_type_mismatch` is phrost being right.** The fixture is
`function needs_string(string $name) {}` and `needs_string(123)` with no `declare(strict_types=1)`,
and PHP coerces `123` to `"123"` there. phrost's weak-mode model is per-kind rather than
all-or-nothing, and probing it end to end shows it exactly: array->int, null->int, object->int,
string->array and array-return-from-int all report, `'42'`->int and `return '42'` correctly do not.
What it misses is a **non-numeric string literal** where an int or float is declared --
`return 'bad'` from `(): int` is a TypeError in either mode. Declined for now, and not on
population: no `Held` arm carries a literal's text, so this needs a new arm in a vocabulary
`docs/08-types.md` records as closed by convention, for an error that crashes on its first run.

**Two narrowing fixtures are unsound and phrost should keep failing them.** `union_and_instanceof`
expects `$x instanceof Foobar && $x instanceof Barfoo` to offer members of both, and declares both
as plain classes -- PHP has single inheritance, so no object is ever both and the branch cannot be
entered. `locals/narrowing.rs::also_of_class` says this in a comment and picks the guard.
`or_instanceof_with_noise` expects `if ($x instanceof Foobar || TRUE)` to narrow, and that branch is
entered whether or not the `instanceof` holds. Both are reasonable _completion_ heuristics in the
server they came from and neither is a claim phrost should make.

**The narrowing that does work, works in every shape that occurs.** Probed directly rather than
inferred: a class-and-interface conjunction composes, two interfaces compose, a declared union
filtered by an `||` guard reports the operand the guard ruled out, and the guard-clause idiom
(`if (!$x instanceof T) { throw; }`) is correct with a nullable, with a union, with a `return` and
with nothing declared at all. The one real gap is that an `||` of `instanceof`s **filters** a
declared union rather than **building** one, so with an untyped parameter it answers nothing.
Population on the reference checkout: **1** site in the project's own code, against **97** for the
negated guard -- and all 97 shapes already work. Not worth building for one site; recorded so the
next reader does not re-derive it.

### An assert on a plain function in a guard -- done

`@phpstan-assert-if-true` on a free function was the largest single lever measured on the corpus
this project had never read, and it is now read. WordPress **4192 -> 4048**: 145 false positives
gone, one true finding added that WordPress's own comment admits (`delete_old_theme` declares
`@param bool $removed` and returns a `WP_Error` "to pass errors through"). Symfony 19861 -> 19857.
Everything else unmoved.

What made it a piece of work rather than an arm is that the root resolves its subject at the
**guard**, so one built for a callee promising nothing answers with what the value held before the
branch. `AssertsArguments` is the gate, answered from a project-wide name set filled where the
skeleton already notes a `never` return -- the shape §8 records for `NeverReturns`. It cost 8
signatures of threading rather than the 24 that carry `never`, because `Flags` already reached every
recursion.

It also closed an older bug nobody had reported: `class_a_path_names` matched roots by shape, so any
guard between a `@var class-string<T>` and its `new $class` lost `T`, through a static method's
assertion as well -- shipped for months. `PathRoot::narrows` answers what a guard root narrows, and
is exhaustive so a new guard root cannot answer "not a guard" quietly.

**What is left of WordPress's 4048 is mostly not ours.** Of its 517 nullable- and false-receiver
findings, 491 have no test of the subject anywhere above them: WordPress relying on runtime
invariants its declarations do not state. That is a position rather than a gap, and it is what makes
the remaining count unrankable by size alone.

### 4. Two things measured and declined, so they are not re-opened

**A parameter's type inherited from the declaration it overrides: built 2026-09-20, after the
refusal below was shown to have been priced by the wrong instrument.** The entry counted `@param`
element types in the project's _own_ interfaces and abstract classes -- 18 -- when the parents that
matter are the framework's. Counted properly: the project's own modules hold **202** untyped
parameters and **188** of them sit under a `{@inheritdoc}`, which is a house style rather than a
gap. And the one the count missed entirely was the last definition gap left in a side-by-side run
against another server: `preRender(&$values)` over Drupal's `ResultRow[]`, whose `@param` is two
levels up because the class in between carries `{@inheritdoc}` too.

Built as a question `phrost-body` asks and `phrost-view` answers -- `PathRoot::InheritedParameter`,
the shape `Leaving::Ask` established -- because finding the overridden declaration needs a hierarchy
and nothing at the body layer has one. It climbs rather than taking one step, for the reason above.

Measured: 352 -> 347 unresolved receivers on the reference project, 5 findings' worth of new
precision across three corpora, and every one of the eleven read by hand. Four of them are
`SysLogger`, where the parent documents `string|\Stringable`, the file declares `strict_types=1` and
`strip_tags($message)` therefore fatals -- confirmed by running PHP 8.5.10 both ways. The gain is
smaller than 188 suggests because most untyped parameters are scalars (`$plugin_id`, `$permission`),
which is the honest answer rather than a disappointment.

**The original entry, whose reasoning about contravariance still stands:**

**A parameter's type inherited from the declaration it overrides: refused at 18 own sites,
2026-09-19.** PHP is contravariant on inputs, so an override may widen a parameter and the parent's
hint is a lower bound rather than an answer -- which is the §8.1 note about a declaration on the
input side, and is what makes this worth less than it looks. Counted on the reference project before
building: 1 963 methods carry a docblock that is only `{@inheritdoc}`, which is the number the case
for this reads from, but only **18** `@param` element types exist across the project's own
interfaces and abstract classes for any of them to inherit -- and 64 in a 400-file sample of Drupal
core. The 1 963 are overrides of methods whose parent says nothing more precise than the signature
already does, and the signature is already read. What would change the answer is a project that
documents its interfaces' array parameters; measure the second number, not the first.

**`@return Reseller[]|array`: refused as an inference rule and _reported_ instead, 2026-09-20.** The
refusal below stands and was priced by the wrong instrument. Reading the union as its left operand
is unsound -- Symfony's `AbstractString::unwrap()` writes `@return string[]|array` for a function
that returns _nested_ arrays, so the `|array` is a real weakening there. But the cost was counted as
"two occurrences of the annotation" when the number that mattered was what one occurrence costs:
`User::getResellers()` alone leaves **16 of the reference project's 352 unresolved receivers** with
nothing to resolve to, because every callback over what it returns takes its parameter from the
element type. So the answer is neither to guess nor to shrug: the union genuinely says only `array`,
which is a defect in the tag, and `doc-redundant-type` now says so. Measured: 17 on the reference
project (16 of them `array{...}|array` on one module's application classes), 4 Symfony, 2 Laravel, 1
WordPress. The original entry follows, because its reasoning about _inference_ is still right.

**`@return Reseller[]|array`: refused at 2 sites, 2026-09-19.** A union of a precise element type
with the bare `array` that contains it, written to keep a static analyser quiet. Rendering the
weaker claim is what §8.1 says to do when two spellings collapse, and that is what already happens
-- the union is `Reseller[]|array`, the array half says nothing about elements, and a member access
on an element resolves to nothing. Reading it as "the left operand, the right one is noise" is a
rule about how somebody writes docblocks rather than about PHP, and it would silently sharpen a
claim the author weakened on purpose. Two occurrences on the reference project and nine across the
four corpora. Left alone.

**Inferring a named function's return type from its body: refused at 1 of 508, and the gate was
re-applied on 2026-09-03 with the same answer.** The population this entry says to measure came back
**642** declaring neither -- **608 of them constructors**, 20 returning nothing, and **14
candidates**: eight `__get` reading an untyped array, two `NestedArray::getValue`, two that return
only `NULL`, one returning a render array, and one real (`ProductCoordinator::getCoordinator`,
returning a local assigned from a `match` of factory calls). The ceiling is therefore the same
single function this entry already measured by hand, on a project that has changed daily since.
Nothing to re-open; what follows is the original measurement.

**Inferring a named function's return type from its body: refused at 1 of 508.** The headline count
was real and its composition was not. Of 589 functions in the reference project's user code that
declare neither a return type nor `@return`, **549 are constructors** -- PHP forbids a return type
on one, so declaring nothing is the only legal thing to do -- 4 are abstract, and 21 return nothing
at all. **Fifteen actually return a value**, and thirteen of those would infer `mixed`: nine are a
`__get` reading an untyped array, three are `NestedArray::getValue`. Measured at the ceiling by
hand-annotating all 34 candidates in a clone -- more than any implementation could reach -- coverage
moves from 43 030 to 43 071 resolved, all of it from the one remaining function, and
`undefined-method` rises from 8 to 10 because typing it exposes two calls to members only half its
union declares. `@return static`, the type a human would write, produces the same two findings, so
the looseness is the code's and belongs in a baseline a person accepts, not in an inference over
fifteen functions. The gate for revisiting is the _population_ -- functions that return a value and
declare nothing -- measured **after** splitting out constructors, not the headline count.

**But closures were not in that population, and they were where it paid.** Built and measured at
**58 of 82** -- see _A block closure with no declared return is read from its body_ below, which is
the same question asked of a shape where the structural obstacle is absent.

**An open catalog reports a near miss, which opens all five without closing any.** Three of the
eight catalogs were reported against and five were silent, and the five hold most of what a project
writes: 1 073 routes, 763 extensions, 422 fields, 262 permissions, 82 render elements. Silence there
left the mistakes that hurt most unreported, and they hurt because they are _runtime_ mistakes -- a
misspelt route is a white screen, a misspelt permission is access silently refused, a misspelt
`#type` is an exception while a form is built.

Closing any of them was never available, and the reference project says why in one measurement.
`phrost coverage` now counts every catalog key written: **2 759 keys, 2 031 registered (73.6 %), 725
(26.3 %) an open catalog cannot judge** -- and the 725 is almost entirely two things. **668 of them
are the project's own DSL** borrowing the `#` convention (`element String` 245, `Object` 108,
`Float` 105, `static` 67, `Array` 50, `Date` 29, `Integer` 26, `Boolean` 22, `DateYM` 11, `Json` 5),
and 43 are `entity.node.canonical`, the route Drupal generates from an entity type's link templates.
Closing `element` or `route` would have lit up 711 correct lines.

What separates a mistake from an absence is that **a typo nearly hits**. So an open catalog reports
a key that is a plausible misspelling of one it holds -- Damerau-Levenshtein, one edit under twelve
characters and two at or above, nothing compared below five, and nothing said when two keys are
equally close. A closed catalog says what it always said and adds the suggestion.

Measured on 1 662 files: **0 findings**, and that is the result. None of the 668 DSL keys is near an
element id, none of the 43 routes is near a registered route, and no field a bundle attached is near
a configured one. The check is prospective by design: it fires on the typo while somebody is typing
it, which is the only moment a runtime-only mistake can still be caught. Diagnostics p95 unmoved at
19.5 ms.

**And the 43 were the catalog's fault, not the check's -- done.** `entity.node.canonical` is
generated by Drupal from an entity type's `links`, so no `*.routing.yml` declares it and the
near-miss reader, which reports a typo only near a _registered_ key, was silent on a typo of it
while reporting a typo of a YAML route. From outside, an asymmetry in the check; from inside, a gap
in the catalog.

The names are mechanical and the framework's own code says how: `EntityBase::toUrl()` spells
`"entity.{$this->entityTypeId}." . str_replace(['-', 'drupal:'], ['_', ''], $rel)`. So a rule states
the template and that `str_replace`, lowering keeps the _entries_ of the one array argument the rule
names -- gated hard, because entering an array keeps a string per element and `handlers` sits beside
`links` in every entity type Drupal ships -- and the index spells one key per entry.

The load-bearing decision is that a derived key **names nothing**. A route is not a class, and
recording the entity class as its value would have made `Url::fromRoute('entity.node.canonical')`
return a `Node` -- the trap `entities.rs` already documents for `getStorage`, arriving from a
direction nothing was watching. What travels instead is the entry's own value, which for a link
template is the path, and which is the same line a YAML route already shows.

Measured: route catalog **1 073 -> 1 350**, and `phrost coverage` catalog keys **2 031 registered
(73.6 %) -> 2 075 (75.2 %)**, unjudgeable 725 -> 681. Index heap +0.2 MB, bootstrap unmoved --
interleaved runs of both binaries came out 4.12/3.07/2.97 s against 3.10/2.98/2.93 s, bands
overlapping. Ratchet held, 17/17 budgets.

Hover over `entity.node.canonical` now says `/node/{node}`, and goto still says nothing: the class
carrying the link template is not the route, so `CursorTarget::CatalogPlace` carries the description
always and the place only when a file declares one.

**And for a fortnight none of it survived a restart.** `ProjectSkeleton::adopt`, which takes on a
stored index, re-claimed the keys an _attribute_ names and not the two kinds derived from a
declaration -- so the 277 routes above and all 1 038 hook names were present on a server's first
start of the day and absent on every one after it. What the report from outside looked like is
"reports once in five runs", and the two instruments anyone would reach for cannot see it at all:
`phrost check` and `phrost query` are both cold. See `docs/02-testing.md` under `cold == warm`,
which now says which instrument can.

### An argument that is a key with something taken off it -- done

`alter($type)` runs `hook_{$type}_alter`, so binding its argument as a whole hook name judged
`alter('form')` against a key called `form`. `ArgumentBinding` carries the suffix now, the way
`DispatchedName` always has. Two shapes changed on the reference checkout: `alter('frm')` suggests
`form`, and `alter('form_alter')` -- the suffix written twice, which people write while knowing the
hook's real name -- reports without needing any edit distance. The population is honest: **one
literal `->alter('...')` call in the whole 30 916-file checkout**, so this is a
correctness-of-reading fix rather than a catch, and the argument for it is the form -- the writing
half of one seam knew a transformation the reading half did not, for the third time this week.

**The generics infrastructure is done, and the four refusals that named one cause were priced on a
claim that is no longer true.** The recorded cause was "no type-ref variant keeps its arguments
through interning and the disk cache". `TypeRefData::Generic { base, args }` and its interned
counterpart both exist, and the cache is a hand-rolled tagged encoding where a variant is a tag
const and two arms -- twelve lines, not a format problem. Probed rather than assumed, across a
_closed_ file boundary so the interned-and-cached path is what answered:

- `@template T of object` on a class, `new Bag(new Item())`, `$bag->first()` -> `Item`. Works.
- `@param class-string<Rule> $name`, `new $name()` -> `Rule`. Works.
- `@return Bag<Item>` read from another file -> `Bag<Item>`, and its `@return T` -> `Item`. Works.
- `foreach` over `Iterator<string, Item>` -> the key is `string`. Works.

So the _Blocked_ entries below for `class-string<T>` and for `@template T of object` are stale, and
what was actually left was three separate causes with nothing structural in common. **Two are now
done**, and neither needed the cache format changed for the reason the entry gave:

- A container whose _arguments_ are templates the parameter binds. `iterator_to_array` reads the
  elements (`elements_branch`) and `array_keys` reads the **keys**, which is positional rather than
  structural: `@param array<K, V>` names both halves and only which argument it is tells them apart,
  so the reader that takes the last one answered about the values. `ArgumentKeys` is the one variant
  this whole lever needed -- one HIR variant, one sema variant, one cache tag, one resolver arm --
  and what a key _is_ comes from `iteration_key_of_held`, which `foreach ($x as $k)` already asked.
- A callable's return. Kept as a generic over the `callable` builtin, the way `class-string<T>` is
  kept over `string`, so no new node and no cache tag at all. The `\Closure` hint took a second rule
  in the _written_ refiner, which had no `object` arm either -- see that commit.

A third was reported from outside and turned out to be two, which is worth recording because the
report named one root and the probe found two:

- **A class-level `@template` filled from a `class-string<T>` constructor argument.** The factory
  shape, and the commonest generic in modern PHP -- a container, a repository, a `createMock`.
  `@param T $item` worked and this did not, because they are different questions about the same
  parameter: one asks what the argument _is_ and this asks what it _names_. Read as a container it
  asked for the element of a string, so the template fell back to its bound and every method on the
  instance answered `object`. Fixed; the class comes from the argument _expression_, because a `new`
  carries no literal list at all.
- **A container constructor argument written as an array literal.** Was the second root, and is now
  closed: a list literal keeps its entries and the view reads their union. See _A list literal holds
  what its entries hold_ below.
- **A free function's template, reported after the first fix.** `returned_by_function_condition` was
  named for what it did -- it read a conditional `@return` and nothing else -- so a free function's
  `@template T` + `@param class-string<T>` + `@return T` answered whatever its hint said. Reported
  with eight spellings, and the hint was not the variable: `object`, `mixed`, no hint, both
  namespace spellings and the global namespace all failed, and a method one line away answered. For
  a framework whose `.module`, `.inc` and `.install` files are free functions by definition that is
  not an edge. Fixed by reading every kind of contract there, not only the conditional.

What is left is one cause: **a template bound at several parameter positions at once**
(`max`/`min`), refused below with the measurement of why the cheap version is unsafe.

**A template bound at several positions at once: refused, and the cheap version is unsafe.**
`max`/`min` are written `@template T`, `@param non-empty-array<T>|T $value`, `@param T ...$values`,
`@return T`. Two parameters bind `T` and the contract carries one position, so `bound_by_an_element`
wins on `$value` -- and `max(1, 2)` then asks for the element of an `int` and answers `mixed`.
Refusing the union binding was built and reverted: it makes `max(1, 2)` answer `int`, and it makes
`max(1, 'a')` answer `string`, which is **narrower than the truth** and turns into a false
`incompatible-argument` at the next call. A type that is too narrow is worse than `mixed`. The fix
is a branch carrying several positions -- `ArgumentTypes { parameters }` -- which is an HIR variant,
a sema variant and a cache tag; worth it only alongside the other two.

**"The standard library is not generic enough" is mostly already closed, and what is left is one
thing.** Measured against the reference checkout: 1 143 call sites in its own non-test code reach a
stub function whose _declared_ return is a bare `array` or `mixed`, which reads as a large gap.
Asked what those calls actually answer, nine of the twelve most-used are already precise --
`array_map(fn (Row $r): string ...)` gives `string[]`, `array_values`/`array_merge`/`array_filter`
give `Row[]`, `explode` gives `string[]`, `reset` and `array_pop` give `?Row`, `preg_replace` gives
`?string`. Three are not, and each for its own reason:

- `json_decode(...)` gives `mixed`, and that is **correct** -- it returns whatever the JSON held.
- `max`/`min` give `mixed`. The honest answer is the union of the arguments, which is a `@template`
  over a variadic and needs the generics work below.
- `array_keys` and `iterator_to_array` give a bare `array`. Both need a type argument to survive:
  `array_keys` over a `list<T>` is `int[]`, and only a `list` says so -- and `T[]` is how this
  renders one, so the listness is gone before the question is asked.

So there is no separate "improve the stubs" task. What remains is generics, and it is the same lever
as `@template T of object`, `class-string<T>` and `@param callable(Foo): Bar $fn` -- four refusals
with one cause: no type-ref variant keeps its arguments through interning and the disk cache.

**A parameter's `@param` inherited through `{@inheritdoc}`.** `flatten` already propagates a
_member's_ docblock type onto an override that declared a native one -- `getFields(): array` under
`{@inheritdoc}` gets the interface's `@return Field[]` -- and does not do the same for parameters.
Since a parameter's docblock _widens_ what the callee accepts, an implementation written
`{@inheritdoc}` arguably inherits that promise too. Implemented and measured: **the ratchet did not
move by one finding on 1662 files.** The case it was meant to serve does not arise -- Drupal's
`fetchAll` is called through the interface, where the widening already reads, and `fetchAllAssoc`
has no `@param` on the interface at all, so there is nothing to inherit. What it did change was a
rendered declaration: `public function find(array<int, string> $ids)` over source that says
`array $ids`, which is the same invented type refused for untyped constants. Declined on a
measurement rather than on plausibility; re-open it only with a project where it moves a number.

**`@phpstan-import-type`.** `@phpstan-type` landed; the cross-file half needs the declaring class's
alias table available while another file is lowered, which lowering cannot do one file at a time, so
it would mean persisting the table in the index and resolving it in the view. Counted first: **one**
`@phpstan-import-type` in the whole reference checkout, and the imported name is used four times.
Not worth a persisted table and a cache round-trip.

**Narrowing a negated `&&` in a guard clause: refused at 0 real sites.**
`if (!is_string($x) && !$x instanceof Foo) { return; }` should leave `string|Foo` behind it and
leaves the declared union whole. The dual is already read --
`if (is_string($x) || $x instanceof Foo) { return; }` narrows correctly, because `!(A||B)` is
`!A && !B` and both claims then hold. `!(A&&B)` is `!A || !B`, where _neither_ holds, so it needs a
union of two proofs the way `one_of_instanceof` builds one for the positive `||` -- except the
operands here are a kind claim and a class claim rather than two classes, so `Proved` would need to
express "either of these" in general.

Counted before building any of it: **zero** occurrences of the same-subject form
(`if (!$x ... && !$x ...)`) in the reference checkout, and 9 of `if (! ... && ! ...)` at all --
every one of them about two different subjects, where narrowing is correctly refused because either
operand could be the reason the guard did not fire. Against 24 sites of the `||` form that already
works. One corpus fixture, no real site; re-open it with a project where the idiom appears.

The other seven shapes of the family were probed at the same time and all work: `!$x instanceof T`,
`is_string($x)`, `!is_string($x)`, the positive `||`, and both `throw` spellings.

**A union answers what any of its operands answers -- changed, not refused.** phrost used to hold
that `A|B` reaches only the members both declare, and the reasoning was sound as far as it went: a
call to `B`'s own method fatals whenever the value turns out to be an `A`. What made it wrong was
that the position was only half held. The _checker_ said nothing about such a call -- `Uc|Ud` with
no common ancestor reached `UnknownReceiver` and was silent -- so withholding the completion row
prevented no mistake; it only left two unrelated operands with an empty list, which is what an
editor shows when a server is broken. And where the operands did share a base, the reduction to that
base produced a **false positive**: `Undefined method UBase::cOnly()` for a call `Uc` declares,
naming a class the source never wrote, reported twice, with goto dead at the same site.

So the rule is now: resolution names the operand that declares the member (goto and hover land
there), the outcome of a union is the best any operand reaches (a report only when _none_ of them
can), and completion offers every operand's members with `— Uc only` beside the ones that are not on
all of them, the shared ones sorted first.

The reason it is not the _strict_ completion of the position -- reporting the call, as PHPStan does
-- is specific to this codebase and worth keeping written down: a union here is often built by
inference, and `PathRoot::EitherOf` reads every operand at the joining expression's own offset, so
the union can be _wider_ than the value really is. A report on an approximate union is a report on
our own imprecision. If the joins ever become exact, strict becomes available and this note is where
to start.

Measured: resolved 44 906 -> 44 913, untyped receivers 648 -> 640, one access correctly reclassified
from "no type" to "no operand has it" (guarded by `method_exists`, so still not reported). Ratchet
held, every count unchanged. Across the three corpora **+19 fixtures** from the one change: 287 ->
295, 324 -> 335, and 245 unmoved -- the widest single move any change has produced on them, which is
the measure of how much a half-held position was costing.

**Typing `->value` from the field catalog: refused at 32 of 510 sites.** The one Drupal-shaped hole
left, and the one an outside reader named as the thing nobody does. The chain is expressible today
-- `PathStep::MethodReturn` already carries the literal arguments, `settled_by_chain` already reads
an earlier link, and `fields::catalog_places` already reads each field's `type:` out of its storage
YAML -- so the obstacle is not mechanism. It is reach. Counted on the reference checkout: **510
`->value` reads in 160 files, and 32 of them are written `->get('field_x')->value`**. The other 478
reach the list through a local (`$field = $node->get('field_x'); $field->value`) or through
iteration, and by then the field name is gone: resolving `$field` answers `FieldItemListInterface`
and the _steps_ that produced it are not part of the answer. Nothing short of a type that carries a
catalog key would recover them, which is a new arm in `Held` and a new shape in the interned table
for a third of a percent of the project's member reads.

What it would buy at those 32 is also smaller than it sounds: a field's `->value` is a scalar, so no
navigation is unlocked, only hover and the argument checks. And it is honestly `?int` rather than
`int` -- an empty field's `->value` is NULL -- so the visible half of the win would arrive as new
`possibly-null-argument` findings. Re-open this if a project turns up where the field name is
usually adjacent, or once a keyed type exists for another reason; the gate is the _adjacency_ count,
not the `->value` count.

**A bare `array` beside `T[]` in a union.** `@return Reseller[]|array` yields no element, because
the rule added with union iteration refuses a union whose operand names no class and produced no
element -- and a bare `array` is exactly that. Arguably it should be _passed over_ instead: it is a
container that said nothing, not a scalar that positively has none. Two occurrences in the whole
checkout. Left alone.

Both were found by using the server on real code rather than by reading the corpus, which is the
loop that should produce most of what follows.

### 4b. Done: the member row's words are packed

`size_of::<MemberRow>()` was **96** with a six-byte `MemberModifiers` and **88** with a five-byte
one, so the sixth byte cost a whole word per member -- **2.2 MB of 104** on the reference checkout,
paid the moment `#[\NoDiscard]` was recorded. The five words now live in one `u8` behind methods,
the hook shape in a second byte, and the row is back to 88 with room for three more facts before the
next word is spent. Measured after: **104.0 MB**, the figure before the flag existed.

The job was 62 reads across 21 files and six construction sites, and the compiler caught every miss
because a field became a method. Two traps worth keeping: `phrost-view`'s `MemberModifiers` and the
LSP's `TokenModifiers` are _different_ structs with fields of the same names, so a blind rewrite
touches them and has to be put back; and the two copies of this vocabulary (`phrost-hir`'s and
`phrost-sema`'s) are mapped word by word rather than by handing the packed byte across, because
passing the bits would make their bit order load-bearing and silent.

### 5. Oversized files

`analysis/context.rs` 719, `index/skeleton.rs` ~1 130, `view/symbols.rs` ~1 100, `body/narrowing.rs`
~1 100. Ordinary debt; none is where every change lands.

## Open, unranked until the corpus is re-read

What is left after the refusals and the four above. Small, and none of it is blocking anything.

- **What PHPantom still offers on a Laravel model and phrost does not, measured 2026-10-12.** The
  migrations, the `@property` columns' `whereX`, `withCount`'s `*_count`, the factories and
  `increment()`/`decrement()` are all read now (`docs/08-php.md`). Left: a squashed
  `database/schema/*.sql`, which is the whole schema on Laravel.io and needs a SQL reader this repo
  does not have. What PHPantom lists beyond that on Monica is not a gap: `wherePivot` and `pivot` on
  every model (a pivot is only on a model a `BelongsToMany` loaded), and a model's public instance
  methods through `::` -- `Post::getConnection()` is
  `Non-static method ... cannot be called statically` under PHP 8.5, since `__callStatic` only runs
  for a name the caller cannot reach.

- **`@phpstan-type` aliases.** `@phpstan-type UserList array<int, User>` on a class,
  `@return UserList` on its methods. One fixture, but real projects write it.
- **`global $x`.** Importing a file-scope variable into a function scope. One fixture.
- ~~**`numeric` and `scalar` lower to `mixed`, and a stub started using the first.**~~ **Done, and
  the entry's own framing was wrong.** They lower to the sets they name now -- `int|float|string`
  and that plus `bool` -- which is a lowering rule as this said, and every operand already existed.
  What it got wrong is the prize: the `array_sum(['a', 'b'])` finding is **not recoverable**,
  because `numeric-string` folds to plain `string` with every other way the docblock language
  narrows one, so `array<numeric>` can never reject `array<string>` however it is lowered. What the
  change buys is the honest weaker claim -- `array_sum([true])` is rejected in a `strict_types` file
  -- and a reader hovering the parameter shown `int|float|string` instead of `mixed`. **Zero
  findings moved** on WordPress, Laravel, Symfony, Slim, the reference project and the vendored stub
  set. Kept anyway, for the reason the `Expectation` seam was: a rule that is right and moves
  nothing on real code has found that real code agrees with it.

  Originally recorded as: `type_lowering.rs` groups `Type::Numeric` with `Mixed` and `Scalar`, which
  was inert while nothing wrote it. mago-prelude 1.48.0 respells `array_sum`/`array_product` as
  `@param array<numeric>`, so the expectation is now `mixed[]`: `array_sum(['a', 'b'])` on an
  `array<string>` is reported by neither, where `array<int|float>` reported it. One message on five
  corpora reads less precisely for it and no finding moved. `numeric` is `int|float| numeric-string`
  and all three already exist, so this is a lowering rule rather than a new `BuiltinTy` -- which
  matters, because the vocabulary is closed by convention and adding to it is a semantics change
  needing the ratchet.

- **A Laravel command's inputs declared anywhere but its own `$signature`.** The signature is read
  (`docs/08-php.md`); a command declaring `getArguments()`/`getOptions()` arrays, or whose signature
  is inherited from a class in another file, keeps the method's own
  `array|string|float|int|bool|null`, and `undefined-input` judges neither. 8 findings on Laravel's
  own commands are the first shape.
- **A loop that rewrites each element by its key.**
  `foreach ($servers as $i => $dsn) { if (\is_array($dsn)) { continue; } $servers[$i] = [...]; }`
  leaves only arrays behind, and PHPStan reads it so; phrost reads the declared `array|string`
  elements after the loop and reports `count()` on them -- two findings on Symfony's
  `MemcachedAdapter`, exposed when a docblock over a native union started being read. The element
  write is already a slot write; what is missing is that the loop's own key names every element, so
  its writes replace the element type rather than join it.
- **A by-reference array mutator after a literal.** `$stk = [1]; array_push($stk, $row);` still
  reads as `array{0: int}`: the slot writes after a literal are folded in (`locals/filled.rs`), a
  by-reference call is not, and the stubs say what it does only in a conditional `@param-out` over
  templates -- `($array is list<V> ? non-empty-list<V> : non-empty-array<K, V>)` -- which lowering
  refuses as it refused such `@return`s. The right shape is the return-contract machinery read for
  `@param-out` too; the price is a contract per by-reference parameter, kept off `ParamRow`, which
  is the largest slice of the index heap. A text scan finds 3, 3, 7 and 33 such sites on the
  reference project, Laravel, WordPress and Symfony, and one finding on WordPress's `Services_JSON`
  is all it costs today. `sort($list)` is the other half: precise elements lost to nothing, not
  wrong.
- **mago's formatter drops parentheses a person wrote for clarity around `??` in a ternary's
  condition.** `($stored['enabled'] ?? TRUE) ? a : b` reprints without them -- the same value, since
  `??` binds tighter, and 183 hunks of that sort on the reference project's own modules, where
  `phpcs` sees nothing either way and `reprint-means-the-same.php` reports no file computing
  anything else. `preserve_redundant_logical_binary_expression_parentheses` covers `&&` and `||`
  only. Upstream's to settle; not phrost's to patch, which would be a formatter of our own.
- **A conditional return whose test is about an array's contents.**
  `@return ($array is array<Foo> ? Foo : Bar)`. `ConditionTestData` deliberately records only tests
  an argument can settle, and this is a claim about what the array holds.
- **A template inside a union inside a generic's _return_ argument.** The parameter half is done
  (`declares` now reads the element and key positions, so `@param array<K, T|null>` binds `T` where
  only `@param array<K, T>` did); the return half is not, and it is where the population is.
  `argument_branch` matches a bare `Type::Reference`, so a union argument binds nothing, becomes a
  plain `Ty`, and `returns_an_applied_class` then declines the whole contract for having no slot a
  call settles -- the return falls back to the bare `array` hint. Measured over five trees, 66 sites
  in 26 files, and the shape is not the one the report showed: **46 are a union of two templates**
  (`@return Collection<TValue|TRejectedValue>`, Laravel 34 of them), 8 are a template beside a
  sentinel, the rest a template beside a class. Only the sentinel eight are reachable without new
  vocabulary, through the `absent`/`Absence` field that already carries exactly this for a top-level
  return -- two constructors of one rule again. The other 46 need `ReturnBranchData::OneOf`, which
  is a variant in two enums, the converter, the cache codec, the dump renderer and the resolver:
  five crates. Unranked because the parameter half, which looked like the same bug, moved
  **nothing** -- coverage 48 830/49 420 on the reference project and 167 441/202 880 on Laravel,
  byte-identical either way, and no finding on any corpus. Counting what the return half would fix
  is only meaningful with that in mind.
- ~~**Chained calls through a parenthesized `new`.**~~ **Not a gap, measured 2026-09-11.**
  `(new Foo())->a()->b('x')->` offers every member, and goto answers at each hop. Left here as a
  refutation rather than deleted, because an entry that reads as open is worked on again.
- ~~**`new Foo()->method()` without parentheses** (PHP 8.4).~~ **Done.** Goto and every check always
  answered; what was missing was _completion_, which is the text scanner and read only the older
  spelling. The two part company at the point that branch is decided: with the parentheses nothing
  is named left of them, and without them the class **is** the trailing identifier. **111 sites in
  the reference project's own modules and 220 in Symfony**, against 42 for the parenthesised
  spelling -- a project on 8.4 stops writing the older one.
- ~~**Property hooks as scopes** (PHP 8.4).~~ **Mostly not a gap, measured 2026-09-11.** This said
  "goto and rename inside a hook body find nothing"; goto on a call written inside a `get` hook
  answers, and so does completion, and a hooked property is navigable from a use of it. Rename
  inside a hook body was not re-checked. Originally recorded as:
  `set(Item $value) => $this->x = $value` is function-like and is not modelled as a scope.

The rest of the 8.x surface is covered and was checked rather than assumed: backed enums, `readonly`
classes, typed class constants, `private(set)`, DNF types, promoted readonly properties, `never`,
constants in traits.

---

## Done, and what it cost

Kept because each records a decision that would otherwise be re-litigated.

**Configuration is part of the reference space, and everything downstream fell out of that.** A
Drupal service writes its class in `*.services.yml` and a route writes its controller in
`*.routing.yml`, and `references` on such a class answered _one_ -- its own declaration -- while a
rename rewrote the PHP and left the YAML pointing at a class that no longer exists, reported by
nothing because the file is not PHP. The rule saying `class:` names a class had been there as long
as goto _from_ a YAML buffer had; what was missing is that it was only ever applied to a file
somebody had open. Applying it once per configuration file at indexing time is **441 sites, 18 KB,
5.4 ms** -- small enough not to be cached, which also means it cannot become the bug §8 records for
a fact `adopt` forgets to carry. Two measurements shaped it: configuration is found in the _same_
walk as PHP, because a walk costs `opendir` per directory and a second one wants the same
directories; and the file's **name** is tested against the rules before it is opened, since a real
project's own configuration is 1 551 files and 53 of them are named by a rule.

Three things followed with no new mechanism. A route names `Class::action`, so the _method_ is a
reference too -- and `_controller` was not the only key: counted by what the project writes,
`_custom_access` is 80 and `_title_callback` 30 against `_controller`'s 642, so a fifth of the
methods a route names had no goto, no reference and no rename while the key three lines above them
worked. A container written for autowiring gives a service no id at all -- the key _is_ the class,
48 of them on one checkout -- which needed a rule about keys rather than values. And a report of
what nothing uses became possible, because the thing it was missing was never an exclusion list; it
was that a class named only in YAML _is_ used and now says so.

**`phrost unused`, and every cut in it is a rule about how code is reached.** 414 declarations
reached by nothing, then 246 once a plugin manager's `src/Plugin/` is discovery rather than a gap,
61 once a test runner's namespace is the same statement, 18 once a service keyed by its own class
counts, 16 once a framework's `ServiceProviderBase` and `DrushCommands` subclasses count as built.
The sixteen are exception classes nothing throws and three enums. Members came after, reached by
_name_ rather than by the pair of class and name -- an access with an untyped receiver resolves to
nothing and matching the pair would read that as nobody calling it -- less anything a class above
declares, which is what `foreach` calling `getIterator` looks like: **109 of 8 540** on the
reference project. It is a pass and not a diagnostic because the question is a file-count per
declaration, and it says which kind of package it is looking at, because a library's public API is
called by consumers a checkout does not hold and there the same number means the opposite.

**A docblock is brought up to date in one action, `@throws` included, and a file is fixed on save.**
`Add @param` was on the parameter and on the name only when several were missing, `Add @return` on
the name alone, and nothing said what a body throws. One reading now counts everything the docblock
lacks -- parameters, the return, every `throw new X` no `@throws` covers -- and offers by the count:
one thing gets the action naming it, two or more get `Update the docblock`, which writes them all as
one insertion in reading order. `@throws` is read off the body's own `throw new` only: a rethrow, a
callee's exceptions and a closure's are deliberately not, because the tag a docblock wants is the
one its author can see. `doc-missing-throws` reports a `throw` no `@throws` covers, where covered
means the class or an ancestor -- `@throws \Exception` is how a docblock summarises -- and it is its
own opt-in rule rather than part of `doc-missing-tags`, because Drupal's standard asks for
parameters and not exceptions and a project should be able to say yes to one without the other.

`source.fixAll` applies, over the whole file, the _preferred_ actions of the offers that say they
can be applied unread (`Offer::unattended`): the docblock tags and an import with one candidate. The
nullsafe operator, the namespace fix and implementing missing methods stay menu entries -- each is a
correct fix and each changes what the code does or writes something a person should read first. Two
fixes whose edits touch are not applied together, since their order would be the applier's. The
kinds are advertised by name so a client can ask on save; in Zed that is
`"code_actions_on_format": {"source.fixAll": true}`. Cost, because the fix runs on every request a
client sends unfiltered: code actions at a cursor 0.36 -> 0.77 ms at p95, against 12 ms.

**A plain `@return V` over a `@template`, which is where "generics" actually started.**
`reset($rows)` answered `?Row` and `array_find($rows, $fn)` answered `mixed`, and the reason was the
spelling rather than the claim: both stubs carry `@template V` and `@param array<K, V> $array`, but
`reset`'s return is written as a conditional and lowering turns a conditional into
`ReturnBranch::ArgumentElement`, which the reading side already understood. Nothing produced that
branch for a plain `@return V|null`.

Checked rather than assumed: a hand-written function with `@template V`, `@param array<K, V>` and
`@return V|null` and **no** callable parameter answered `mixed` too, so `array_find`'s callable was
not what broke it.

One rule beside the conditional it already reads, gated on the docblock declaring a template so the
parse stays off every other declaration. Untyped receivers 597 to **574**, resolved 43 058 to **43
081**, ratchet held, index heap and corpora unchanged. It reaches user code as well as stubs, and it
needed no type-ref variant, no interned counterpart and no cache format change -- which is what
three earlier refusals had priced generics at.

**A block closure with no declared return is read from its body.** `function (): Row` declares its
result and `fn () => new Row()` _is_ its result; `function () { return new Row(); }` was the one
shape in between and it answered nothing. **58 of the reference project's 82** block closures with
no declared return now answer -- against **1 of 508** for the sibling question about named
functions, and the difference is entirely structural rather than a better implementation: a closure
is assigned and invoked in one body, so the statements are in the same `FileHir` the question is
asked against. There is no ingest ordering to respect, nothing to store, nothing to put through the
disk cache, and `open == closed` falls out rather than being arranged -- the only thing that has to
come from another file is the _class_ a `return` names, which is what every other value in this
layer already asks for.

Every `return` has to resolve or the answer is nothing, because half a union is a claim the code did
not make. A bare `return;`, and a path that reaches the end of the body without returning, each
contribute PHP's null: measured rather than assumed, and it cost nothing -- none of the 58 gained a
`?` it did not deserve, because a closure with a guard clause and no else is also a closure whose
other branches rarely all resolve. A generator is declined on `BodyData::is_generator`, and a
closure that names itself is refused re-entry rather than left to the follow budget, which would
have multiplied by the number of returns at every level.

What it moved: the largest single entry in the untyped-receiver list -- one `$twinQuery` closure
feeding eleven `$dupes->addExpression()`-shaped accesses -- is gone, along with sixteen other
distinct shapes. Coverage 43 030 -> **43 058** resolved of 43 735, untyped receivers 625 -> **597**,
assignments typed 9 465 -> **9 472** of 10 261. The whole 1 508-finding list is **byte-identical**,
so there was nothing to audit in either direction. Cost: nothing measurable and nothing stored.
Index heap unchanged at 104.3 MB, bootstrap 2.81 s -> 2.75 s, `check` over 1 662 files
2.31/2.32/2.32 s against 2.33/2.30/2.28 s, diagnostics p95 16.89 ms -> 16.33 ms, all seventeen
budgets met. All three corpora unmoved at 324/379, 285/327, 245/253 -- this is a shape none of them
asks about.

Two things left where they are, both on a count. **A `@return` on the docblock above the
assignment** would be the author's own word and would have to win: **zero** of the 82 carry one, or
a `@var` either, so there is nothing yet for the precedence to protect. And **`false|true`** is what
two of the 58 render, where `bool` is what every other tool writes -- a degenerate form
`Held::one_of` does not fold, which is the shared union constructor narrowing also goes through, so
folding it is a semantics change needing its own ratchet rather than a rendering fix.

**A first-class callable is a value, and three expressions stopped answering nothing.** `(...)` was
lowered for its _names_ -- so goto and rename worked on `find` in `$repo->find(...)` -- and the
value it builds was left unmodelled, which killed everything downstream: `$fn = $this->probe(...)`
said nothing, and so did `$fn(1)` and `$fn(1)->title()` after it. A dead chain is worse than an
imprecise answer, and this is syntax the project's own rules ask people to write. The shape is
`PathRoot::Callable(ValuePath)` -- the call that was _not_ made, wrapped -- and the wrapper is the
whole of it: with the same inner path and no wrapper the variable would hold a `Repo` instead of
something that produces one. It cost one new HIR node, `CallableFunction`, because `strlen(...)`
names a function and so has no receiver to hang the existing member node on. Landed beside two
smaller ones with the same shape of cause: `/` is now `int|float` rather than silence -- the only
arithmetic operator whose result its operands do not settle, which is why `ExprData::Arithmetic` now
carries which operator was written -- and `??=` binds its target like the assignment it is, with the
value being the union `??` already builds. Measured: member-access coverage **byte-identical**,
assignment typing 9 927 of 10 881 to 9 941 of 10 892, and the whole 1 512-finding list differs by
**six removals and no additions** -- every one a `possibly-null-argument` on a `?T $x = NULL`
parameter the author had already fixed with `$x ??= ...` on the next line, which is the operator
doing exactly what it is for. Four corpus fixtures, which is the number this entry predicted when it
was still filed under _Blocked_: 322/379 to 324/379 and 283/327 to 285/327. Diagnostics unmoved --
three `check` runs a side, 2.71/2.85/2.81 s against 2.80/2.84/2.70 s -- and the index heap unchanged
at 104.3 MB, since a body exists only for an open buffer and the two new fields are one byte each.

**`getStorage('no_such_entity_type')` reports.** Asked for twice and refused twice, because it is
two jobs and each half alone changes nothing. _The key set was 107 of 111_: Drupal moved entity
types from docblock annotations to attributes in 10.2 and contrib lags by years, so a checkout
carries both spellings -- 107 attributes, 11 annotations, overlapping in 7 -- and four ids existed
in code that runs and in no catalog. The annotation is now read where the attribute is, in lowering
off the docblock mago has already parsed, for the attribute names the provider's own rules name and
no others; entity 107 -> 111 and element 77 -> 82, which takes `horizontal_tabs` with it. mago
deciding what a tag _is_ rather than a text scan is load-bearing: Drupal's `core.api.php` teaches
the syntax with a real `id = "comment"` inside an `@code` block, and reading that would have put a
second claim on the id and made it resolve to nothing. _And nothing could report it_: `catalog_uses`
reached a catalog through an argument binding or an array key, and this one is reached through a
forwarded argument, so closing it moved not one finding. The reader is keyed on the call that
_writes_ the id, gated on that call returning the class the forwarding rules are written on -- a
name alone would report every project's own `getStorage`. Cost: nothing measurable. Bootstrap 2.86 s
-> 2.88 s, index heap 104.2 MB unchanged, coverage byte-identical at 44 859 of 45 871, ratchet held
with every count unchanged and zero new `unknown-catalog-key` on 1 662 real files.

**`Implements hook_X()` reads as a cross-reference.** The sentence is the whole docblock Drupal's
standard asks of a hook implementation, and the twenty lines saying what the parameters mean are on
the hook -- 306 of them across 57 `*.api.php` files on the reference checkout, all already indexed.
So hover follows it and shows the hook's own prose and `@param` lines, attributed out loud; any name
the convention marks is a goto target, which takes the checkout's 329 `@see hook_*()` references
with it; and `misnamed-implementation` reports a claim the runtime could never honour. Everything
the convention allows arrives from the seam as one rule, placeholders included --
`hook_form_FORM_ID_alter` is implemented by `app_core_form_views_exposed_form_alter`, and reading an
uppercase run as one hole is what makes that silence rather than a report. One finding on 1 662
files, hand-audited: a function in `app_state_notice.module` named `app_state_notifications_alter`,
which no installed extension can own, so the four notification types it returns were never
registered.

**A hook a project invokes itself is a hook that exists.** `Implements hook_X()` reached 31 of the
35 hooks the reference checkout's own modules implement; the four it lost were the ones the project
declares nowhere and dispatches from its own code, through
`$this->moduleHandler->invokeAll('notifications_alter')` and four more like it. There is no
`*.api.php` to point at and there does not need to be -- **the call site is the contract** -- so the
call is indexed as the declaration. Goto and hover on the claim now reach the `invokeAll` that
dispatches it, and hover says what it honestly can: where the call is and what it passes, since a
dispatched name has no signature to read `@param` lines off. 35 of 35.

The gate is the receiver, not the method name: `alter` is called three times on unrelated services
in that one project, and a name-only rule would have indexed whatever those were passed. The index
records the receiver as the dispatching file spells it -- a member of a class the file can name --
and `phrost-view` resolves it, because resolving at ingest would make the answer depend on discovery
order. A receiver neither half can identify is kept and answers _neither_ way: it names no place to
go, and it is not evidence that the name is missing.

Cost: nothing measurable. Index heap 104.3 MB unchanged -- the rows live in a table keyed by file
rather than a field on every file's row, which is the difference between 7 rows and 30 916 empty
slice headers -- and the catalog line gains `hook 7`.

**`Implements hook_X()` where nothing declares `hook_X`**, which the above is what it was waiting
for. Refused at **7 findings, 1 real**; six of the wrong ones were the project's own dispatched
hooks and the seventh, `hook_field_widget_form_alter`, has since been removed from the checkout.
Measured now: **0 findings on 1 662 files**, and disabling the dispatch catalogue on the same binary
brings back exactly the six. So what the corpus proves today is zero false positives rather than one
true one; the true one is pinned by a fixture instead. The blind spot -- a dispatch whose receiver
the index cannot identify -- falls to silence rather than to a report, which is what makes the check
shippable rather than merely quiet.

**Six checks an external comparison found missing**, and what each one refuses is the substance.
`unimplemented-method` never reports a trait's abstract method, because the merge puts it over a
parent's concrete one and the table can no longer tell unmet from met. `read-before-assignment`
fires only where _every_ binding is below the read: the branch-sensitive tier was built, measured at
16 findings and 16 false positives -- all of them constructs the reach model cannot see are
exhaustive -- and thrown away. `non-array-subscript` leaves `string` out because a numeric string
offset is legal and all ten of its findings were wrong. `missing-return` treats a block ending in an
unmodelled statement as a _guess_ rather than a fall-through, which is what stopped it reporting a
`catch` that ends in `goto`. `undefined-function` reports zero across 1662 files and is on by
default, which it was not at first: it shipped behind a project's asking on one false positive, a
`function_exists` polyfill, and was switched on once a conditional declaration could be ranked below
the standard library. The two control-flow checks were audited against Symfony and Drupal core read
as user code, because the reference project's zero could not tell a clean check from a dead one;
four findings, all real, all already known to PHPStan.

**A never-returning call is ruled out by name.** The question -- does this branch leave for good --
was a full chain resolution to learn one bit, which is why a per-branch walk once cost 21 ms past
the diagnostics budget. It is exactly decidable from the callee's name, so a name nothing declares
`never` needs no resolution: 4579 asked, 11 resolved on 1662 real files. What is left scales with
how much a project uses `never` rather than with how much code it has.

**Type compatibility, all four shapes**, and the thing that blocked three agents in a row: `fit`
takes `(TypeId, TypeId)` and the value side of a real site is never a `TypeId`. There can be no
total `Held -> TypeId` -- an interned `Collection<Item>` would need the table open for writing. The
answer was `ValueTy` and `fit_value`, not a conversion.

**A union with one branch that cannot fit is reported.** `int|false` into `?int` is the same shape
as Drupal's `$query->execute()` declared wider than it behaves. Reporting both keeps the unhandled
`false` from a standard-library call, which is the most valuable finding the check makes.

**A truthiness guard rules out `false`, not only null.** `if ($row)` left `array|false` alone, which
is what `fetchAssoc`, `strtotime`, `file` and most of the standard library return. Fourteen
findings, all false. `PathRoot::NotNull` gained `strictly`, mirroring the field `Null` always had,
and the strict spellings -- `!== NULL`, `isset`, `!is_null` -- still report, which is the check on
the change rather than an omission.

**A docblock type may contain spaces.** The type was split at the first space, so
`@param array{a: int} $x` took `array{a:` and left `int}` where the subject belonged -- **432 of 1
279 `doc-missing-tags`, a third of the check, all false.** The exception that makes it correct is
`@param int &$count`, where the by-reference sigil sits exactly where an intersection would.

**A write in one branch is not read in its sibling.** `if/else`, `match` arms, `switch` cases and
sibling `if`s all leaked. A ruled-out write contributes nothing and a possible one _joins_, so
`if ($c) { $v = new Foo(); } $v->bar();` still resolves. Three layers had to learn about reach
together and the second two were only visible through the first -- `assigned_between`, then
`without_null` on a written union, then `Held::one_of` failing to flatten a nested one.

**A dependency's test data no longer shadows a real declaration.** `goto` on `count()` landed in
PHP_CodeSniffer's lint fixture and hover said it took no arguments. 454 such declarations, and the
discriminator is a test directory rather than the `.inc` extension -- 643 of 652 vendor `.inc` files
are under `Tests/`, and the nine that are not are Drush's real includes.

**A framework may pick a branch, and may name a position, but may not invent a type.** Three
contributions in one evening, and the difference between them is the point. `DefaultedReturn` picks
an operand the declaration already wrote, enforced structurally. `ReturnsArgumentClass` names _which
argument_, so the answer is a name the caller wrote. `DeclaredReturn`, which would have claimed a
type the declaration never mentions, was designed and refused.

**Translation placeholders.** Zero findings on this project, and 27 on Drupal core of which 16 are
real bugs -- shipping help text that renders `:views-ui` literally, three core tests whose XPath
compares against an empty node-set and passes vacuously. Two of its rules exist only because
measurement forced them.

**Completion leaves the buffer finished**: the `;` where one belongs and nowhere else, and a full
method stub with the cursor in its body when overriding.

Also landed: `match` as a value, closures held in variables, `class-string<T>` keeping its class,
generics bound from a `class-string` argument, `@phpstan-type` aliases, property hooks as scopes,
`global`, iteration yielding a type, `@var` surviving recovery, editor configuration through
`initializationOptions`, `T[]` rendering, array shapes coloured in docblocks, unused imports, and
`@throws`.

## Deliberately refused

Each of these was reached, measured, and turned down. Re-litigate one only with a new measurement.

**A member reached through a bare `object`.** PHPStan reports `Cannot call method x() on object` --
62 of them on the reference project, plus 14 properties -- and this is silent. The silence is the
rule this checker is built on rather than a gap in it: `object` says a value is _some_ object and
names no class, so a call on one is not proof of a mistake, it is the absence of information. What
the index can prove is what gets reported; everything else is silence rather than a guess. What the
276 `object`-typed declarations on that project really cost is measured under _The largest remaining
coverage gap is not ours_, and the fix is an annotation in the project rather than a check here.
Together they are 25 of the 62 PHPantom failures, which is why the score is not the thing to watch.

**A member an interface's implementer declares, reached through the interface.** Laravel types a
variable `Illuminate\Contracts\Cache\Repository` and calls `->tags()`, which the concrete
`Illuminate\Cache\Repository` declares and the contract does not. Counted: **1 027 of Laravel's 1
549 `undefined-method` findings** are nine contracts and one base class of that shape -- the cache
`Repository` alone is 361 -- and Symfony has its own, `ExtensionInterface::addUserProviderFactory`.
The reference project has **none**, which is the whole reason this is refused rather than built: the
shape lives in a framework's own source, where its interfaces are deliberately narrower than the
classes its container binds, and an application developer never edits those files.

It is not a _principled_ refusal, and saying so is the point. `ClassHierarchy::relate` is already
lenient in exactly this direction -- "a value typed as an interface is _some_ class implementing it,
and that class may be anything at all" -- so the fit judgement and the member lookup disagree about
one question. The rule that would settle it is the one `outcome_for` now applies to `$this`: an
access naming a member some _subtype_ declares is dynamic rather than missing, and a typo still
reports because no subtype declares it.

What stops it is the price. `implementers_of` scans the class table, so asking it once per
missing-member finding is 2 900 scans per `check` over Laravel and Symfony -- seconds, on a pass
that takes one. Doing it properly means a `parent -> children` table built once per revision, which
is worth having for its own sake (goto-implementation scans today, at 15.7 ms p95) and is a piece of
work rather than a rule. Revisit with that table, not before.

**Union leniency** (13 fixtures). PHPantom and Phpactor offer members from _every_ branch of `A|B`.
We offer only what both declare, because calling one branch's method is a fatal error and a
completion list is read as authoritative. This is the same strictness the nullable checks have, and
it is a deliberate choice rather than a gap. It is also the single largest apparent bucket in the
corpus, which is worth remembering before reading `narrowing/` as the top of a list.

**`array<T>` keys as `int|string`.** `iteration_key_of_held` answers `int` for any one-argument
generic, and that is wrong twice over -- PHPStan reads both `Foo[]` and `array<Foo>` as
`array<array-key, Foo>`, so only `list<Foo>` promises integer keys, and the three are one node here.
Counted on the reference checkout, the single-argument path is 428 `Foo[]` and 6 `array<Foo>`
against 70 `list<Foo>`: `int` is wrong six times out of seven.

Built anyway, measured, and reverted. Coverage was **byte-identical** -- 43 024 resolved, 633
untyped, unchanged -- because nothing reads an iteration key except an argument check. What it
produced was six new `incompatible-argument` findings, every one the same shape: an author who wrote
`FraudScanResult[]` meaning a list, passing `$key` to an `int` parameter. Each is technically true
and none is a bug.

So the correct answer produces no better answers and only more noise, and the honest trade is to
keep the wrong one. Revisit if something ever reads a key type for a reason other than checking an
argument -- then the ratio changes and this is worth paying for.

**`list<T>` as a type of its own.** `list<T>` and `non-empty-list<T>` lower to the same node as
`array<T>` and `T[]`, so the stronger claim -- integer keys, sequential, from zero -- is discarded.
Refused after building it far enough to measure, which is what settled it.

Nothing reads the distinction. `compat` can gain no finding from it in either direction: a `list`
where an `array` is expected must stay accepted, and an `array` where a `list` is expected can only
be unknown. No diagnostic in the set asks which key, or whether the thing is empty; the one that
asks about offsets at all -- `non-array-subscript` -- asks only whether the value can hold any, and
`list` and `array` answer that identically. Element resolution peels a `Generic` and ignores the
base, so `list<Item>` already resolves `$items[0]->label()`. And hover already shows the author's
own spelling, from the tag channel, beside the normalised signature -- both are on screen at once in
the checked-in snapshot.

The cost is a silent regression rather than a compile error, which is now `docs/08-types.md`: adding
`BuiltinTy::List` compiled clean with zero warnings and produced eight false positives immediately,
because `compat::builtins` ends in a catch-all that swallows any variant it does not name.

Scale, for whoever re-litigates it: `list<` appears 70 times in the reference checkout and
`non-empty-list<` never, against 1 180 `array<` and 409 `X[]`.

**Return type from a method body** (6 fixtures). Needs bodies for closed files, which breaks either
the memory rule or `open == closed`. The one shape that survives is an arrow function, and that is
built.

**Recovery at end of file** (4 fixtures). `hello(` and `$foo[` with nothing after them: mago's
recovery drops the statement. Signature help and array-shape key completion both work on the same
code once the client has auto-closed the bracket, which every editor does — verified rather than
assumed. A fixture artifact, not a shape anyone types.

**Static-vs-instance access rules** (2 fixtures). Documented policy in `reachable()`.

---

## Blocked, and what would unblock it

### Seven of these were not blocked, measured 2026-09-19

Probed one at a time against the release binary, each in its own scratch project. Every one of them
answers today, and the entries below them were describing a server that no longer exists:

| written as blocked                                                                  | what it does now                              |
| ----------------------------------------------------------------------------------- | --------------------------------------------- |
| a docblock callable, _invoked_ -- `@param callable(A): B $fn` then `$fn($x)->`      | answers `B`                                   |
| a conditional assert about an _argument_ -- `@phpstan-assert-if-true User $input`   | narrows                                       |
| iteration yields a class and never a type -- `foreach` over `Iterator<string, int>` | key `string`, value `int`                     |
| `class-string<T>` as a value type -- `new $className()`                             | resolves the class                            |
| argument-count **upper** bound -- `needs_two('a','b','c')`                          | `extra-argument`, and it is in every baseline |
| `global $x` importing a file-scope variable                                         | resolves                                      |
| `@phpstan-type UserList array<int, User>` used by the same class                    | resolves                                      |

`array<T>` "is read as having integer keys" is the eighth and is also stale, in the other direction:
the one-argument path answers **nothing** for a key now rather than `int`, which is what the entry
asked for. The `list<T>` work took the only spelling that states a key out of that branch; see _An
array argument is judged by its keys too_.

The lesson is worth more than the seven lines. **An entry that reads as open is worked on again**,
and this document had a whole section of them -- so a record of what is missing has to be
_re-measured_ on the same cadence as the numbers, not only appended to. What follows has been
re-probed; what is above it had not been.

**A docblock's callable signature is thrown away when the callable is _invoked_.**
`@param callable(Foo): Bar $fn` followed by `$fn($foo)` still gives nothing, and it is the only
shape of callable that does -- a closure literal and a first-class callable both answer, and both
reach `Held::Callable` through a `ValuePath`. This one cannot, because there is no path to build:
the claim is in a _type_, and nothing turns it into a `Held::Callable`.

What this entry used to say, and got wrong on three counts, is worth keeping as a warning about
reading a refusal twice. It claimed the _whole_ signature was unreachable and needed a new
`TypeRefData` variant, an interned counterpart and a cache format change. In fact the parameters
were reachable with none of those: the positional meaning a new node was wanted for is supplied by
the _slot_ the type sits in, so an existing `Generic { Callable, args }` in a dedicated
`callback_params_ty` says the parameters where the same shape in `ty` says the return -- and the
reader was already written, `PathRoot::CallbackElement`, typing a callback's parameter from the
container passed beside it. So a closure written `function ($app)` at a documented call site now has
a type, and 418 declared signatures in Laravel's `src/` alone are read. See
`crates/lower/src/docblock/callback.rs`.

What is left is the other direction -- calling the value the parameter holds -- which does need the
return, and a `Held::Callable` for it to land in. Smaller than what has been done, and independent
of it.

**A `@var` above a statement mago dropped declares no local.** `/** @var Foo $x */` immediately
followed by `$x->` is a shape people type constantly, and it is one of the few real editor shapes in
the recovery family rather than a fixture artifact. `lower/src/body.rs` emits `StmtData::Annotation`
per statement; recovery deletes the statement the docblock sits above, so the annotation never lands
and the local does not exist. **Two corpus fixtures fail only for this and are filed under
`generics/`** — `array_subscript_item` and `array_subscript_method_chain`. The subscript machinery
itself works, proved by probes: the same code with `$foo` actually assigned, or taken as a `@param`,
passes today.

**A conditional assert about an argument has no `PathRoot`.** `@phpstan-assert-if-true User $input`
on `Validator::isUser($input)` — the method is on `Validator`, the narrowed value is an _argument_.
`PathRoot::Asserted` is hard-wired to the opposite: method called _on_ the value being narrowed. No
variant carries "argument N of a call, checked against a `when_true` contract". Two corpus fixtures.
The tags are already lowered and hover already renders them; only the path shape is missing.

**Completion's chain walk bypassed `@assert` contracts once the receiver had a step — done.**
`$constants['C']->` offered nothing where the same contract narrowing a bare local answered, and the
cause was not the walk: `written_path` looked the literal element key up in the **project's**
interner, so the receiver had no path at all — and with no path, neither the guard branch nor the
contract branch below it can run. An array key is _data_. `'C'` says nothing about the project
having declared a `C`, and a key lives only inside a body, which a closed file never lowers, so the
project's names will not hold one however large the project is. Read through
`phrost_body::element_key_name` now, the element counterpart of `local_name`. One corpus fixture,
`narrowing/assert_instanceof_array_index`, and the borrowed set went 342 → **343/360**.

**Two instruments got this wrong before it was found, and both lessons are worth more than the
fix.** `phrost query` answers _correctly_ on the complete `$constants['C']->getImage()`, because
that is `resolve_path` and this was the chain walk — so the CLI could not see it at all.
`tools/slot-contract-probe.py` is what did: a driven `serve`, the only shape that reproduced it.

And the trap that cost the most: an in-process fixture with **four** completion markers in one
method said the fix changed nothing, which is why the commit that made it says so. §8 already
records why — a marker leaves a dangling `->` that does not parse, so statements after the first
break are dropped, and every marker past the first tests garbage. Written with one marker, the same
fixture answers. That note says the trap had cost time twice; this was the third, and the first time
it produced a _wrong conclusion about a harness_ rather than a missing answer. `World` and the flow
harness never disagreed.

**Iteration yields a class, never a type.** `iterated_class()` returns `Option<Name>`, so `foreach`
over `Iterator<TKey, string>` gives the value nothing. **Read the trap before starting:**
`supplied_element` works today _because_ `class_of_stored` silently skips non-class arguments and
keeps climbing, so making it type-aware naively stops `Test2 extends Test1<int>` at `int` — the key
— instead of reaching the value. It needs a real "which template is the value" rule, and
`iterated_class` carries 149 positions on the Drupal corpus. Two corpus fixtures; not a change to
make blind.

**`class-string<T>` as a value type.** `type_lowering.rs` folds the whole class-string family to
`BuiltinTy::String`, discarding the `<Rule>`, so `new $className()` has nothing to read. Needs a
type shape that keeps the argument, plus a `class_of_target` arm. Two corpus fixtures.

**`array<T>` is read as having integer keys.** `iteration_key_of_held`
(`crates/view/src/resolve/element.rs`) answers `int` for _any_ one-argument generic. That is right
for `list<T>` and for `T[]`, and wrong for `array<T>`, whose keys PHPStan reads as `array-key` --
`int|string`. So `foreach ($map as $k => $v)` over an `array<Thing>` gives `$k` a type it does not
have, and every use of `$k` downstream inherits it.

The fix shape is known: `array<T>` yields `Held::OneOf([int, string])`, which `Self::one_of` already
expresses. What makes it a decision rather than a change is the consequence -- `$k` becomes a union
in argument positions, so `incompatible-argument` moves off the 79 it stands at, and some of what
appears will be true. It needs its own pass with every new finding read in the source and the
ratchet re-baselined deliberately.

The key comparison below has since taken `list<T>` _out_ of that one-argument path, which makes this
smaller than it was: the shapes left there are `T[]` and `array<T>`, and both of them state no key,
so the branch now wants one answer rather than a discrimination it could not make.

**Argument-count upper bound.** `needs_two('a','b','c')` is not reported; the lower bound is. Needs
the variadic and named-argument rules finished, which is small and simply undone.

**An array argument is judged by its elements -- done, and it was a position rather than a gap.**
The compatibility rules dropped every type argument, on both sides, and said so in three doc
comments. That was right while neither side knew its element: reading one alone invents findings
nothing can corroborate. Both know now -- a parameter states its element in a docblock, a value's
comes from an annotation, an inferred container or a list literal -- and `Row[]` given a `string[]`
is the commonest real bug an array can carry in PHP.

Only when **both** know, which is what keeps the old reason true: an array of unknown elements is an
array and answers as it did, so is a shape, and so is `array<string, mixed>` -- 290 of the 526
element-typed parameters on the reference project. Covariant, and for PHP that is sound rather than
lenient: an array is passed by value, so a callee writing into it cannot reach the caller's copy.

The coercions come free and that is the design, not luck: the element comparison _is_ the scalar
comparison one level in, so `int[]` satisfies `float[]` exactly as an `int` satisfies a `float`, and
a `class-string[]` satisfies `string[]`. Both were named as boundaries from outside before the work
started and both held on the first run.

Three spellings again, and the third is what made a probe with parameters answer nothing while one
with locals answered: the interned type reads its own arguments, `Held::ListOf` carries one, and the
_written_ one -- which is what a parameter of the file being read is -- dropped it.

**Ratchet held: 0 new findings on 1 662 files.** Prospective like the near-miss report, and the
twelve boundaries it was measured against are pinned in `element_arguments.rs`.

**An array argument is judged by its keys too -- done, and the deciding change was in lowering.**
The other half of the same question, read one type argument to the left: if a parameter says its
keys are strings and the argument's are ints, the callee will index it with something that is not
there.

Two things had to be true first, and neither was. `list<V>` and `V[]` interned to the _same_
one-argument type, and they do not say the same thing -- brackets name a container and nothing about
its keys, where a list names `int` keys by definition. So lowering now materialises a list's key,
and the arg count is the discriminator: two arguments states a key, one states none. And `array-key`
sat at the end of a long list of string-ish spellings, lowering to `string` -- which would have made
an int-keyed array a finding against a key that is explicitly _either_, the one thing such a key
must never do. It is `int|string` now, and 17 of the prelude's signatures key an array with it.

The rule is **disjointness, not assignability**, and that is not a shortcut: PHP does not convert an
array's keys the way it converts an argument, so handing `array<int, Row>` to an
`array<string, Row>` gives the callee int keys whatever `strict_types` says. It is the one judgement
in `phrost_ty::compat` the directive does not reach, and a test asserts the pair of findings under
both settings.

What keeps it quiet is that the key vocabulary is _closed at two_ -- the engine normalises every key
it accepts to an int or a string -- so anything that does not reduce to a set of those two is
**unstated** rather than a third kind. A template says nothing, and `array<K, V>` is 114 of the
prelude's signatures. Neither does an inferred `Held::ListOf`: `array_map` preserves its argument's
keys and Drupal's `loadMultiple` is keyed by entity id, so of that shape's six producers two are not
lists, and claiming otherwise would report working code.

The bracket spelling stays keyless deliberately, and this is where PHPStan is refused rather than
copied: it reads `Row[]` as `array<int|string, Row>` and reports it against a string-keyed
parameter. On framework code that is noise, and brackets are the spelling this project's own rules
ask for.

Cost: one rendering changed. A `list<X>` hovers as `array<int, X>` rather than `X[]`, which says the
key it always meant. Rendering it as `list<X>` was tried and reverted -- `list` claims contiguity
that `array<int, Node>` keyed by entity id does not have, and printing the stronger claim for the
weaker type is a lie the weaker direction avoids.

**Ratchet held: 0 new findings on 1 662 files**, and prospectively so -- the reference project's
custom modules write none of these spellings, 0 uses of `list<`, `array<K, V>` or `array-key`
between them. The nine boundaries it was measured against came from outside before the work started
and are pinned in `key_arguments.rs`; the lowering half is pinned in `keyed_containers.rs`.

**A list literal holds what its entries hold -- done, and the entry was priced wrong.** It said the
`ArrayLiteral` node stores keys and values in one flat list so they cannot be told apart, and needs
a new `ExprData` variant. Both halves were stale: a keyed entry arrives as its own `ArrayElement`
node, so a list is exactly a literal with none of those, and what was missing was a `PathRoot` --
the body layer resolved every literal to `PathRoot::Builtin(Array)` and threw the entries away
before anything could ask.

`[new Row(), new Row()]` is a `Row[]`, `['a', 'b']` a `string[]`, `[1, 'x']` an `array<int|string>`.
Two refusals carry it: **every** entry has to answer, because a union of the ones that did is
narrower than the truth and a type narrower than the truth reports correct code; and a keyed literal
stays a bare `array`, because `['a' => 1]` is a shape and reading it as a list answers `int` for its
keys. Bounded at eight entries -- a render array is a literal with hundreds and this runs on the
path a keystroke takes.

Two things fell out. `T|mixed` now folds to `mixed`, the fifth degenerate form -- and the `mixed`
that matters arrives as a _written_ or _interned_ type rather than the arm, so the literal reader
asks `kind_of_held` first and the structural fold says it only handles shapes. And `Held::ListOf` of
a composite renders `array<int|string>` rather than `int|string[]`, which PHP reads as
`int|(string[])` -- the `brackets_bind` rule the two type renderers already state, missing from the
third.

**The theme hook a render array names -- done, read off the template filenames.**
`'#theme' => 'item_list'` says what draws a render array and a typo there renders _nothing_: no
exception, no log line, an empty region. 67 of them in one real project's own modules, and the
commonest half-day of Drupal debugging there is.

A theme hook is registered by `hook_theme()`, whose keys are entries of an array returned from a
function body -- not something a per-file claim can withdraw. What _is_ readable is the convention
beside it: each hook has a template named after it, hyphens for underscores, in the extension's
`templates/` directory. Measured before building: **1 263 files under 458 distinct names, in 20
ms**, which is the same per-extension shape the route reader already walks.

Partial by construction, and that is why the catalog is **open**. `hook_theme()` can register a name
whose template lives elsewhere -- that project puts 21 under `src/Generated/.../TemplateFiles/twig/`
-- and `#theme` also takes **suggestions**, `table__file_formatter_table`, composed at the call and
holdable by no catalog. **31 of the 67 sites are one.** So the only claim is the near-miss, and it
was measured before it was built: of those 31, **none** is within one edit of any of the 458
registered names, because a suggestion is a real hook plus `__` plus more.

Core is read _last_, which is the opposite order from the route reader and deliberately so: a later
entry wins, a module redefining a core route really does replace it, and a theme shipping its own
`table.html.twig` does not redefine `table` -- it overrides how the active theme draws it. Reading
core last makes hover on `'#theme' => 'table'` say `system`, the module that declares the hook,
instead of whichever theme happens to be installed.

Measured: `catalog keys 2 759 -> 2 823`, registered `2 075 (73.6 %) -> 2 108 (74.7 %)`, unjudgeable
681 -> 712 -- and that +31 is exactly the suggestions, which is the number staying honest rather
than a miss. Index heap unmoved at 104.5 MB, bootstrap unmoved: interleaved runs were 4.25/3.01/2.94
s against 3.02/2.94/2.92 s. Ratchet held. Goto on a theme hook now opens the template that draws it.

**A templated return is read with a hint written beside it -- done, and the hint was winning 85 % of
the time.** `TypeTable::refined` ranks a docblock above a native type only for the shapes where the
hint says nothing: `object`, `mixed`, a bare container. A `@return T` beside `: ?Node` is none of
those, so the tag lost -- and a template name is a `Named` and looks like a class to that judgement,
so it could not have known otherwise. Counted on one real vendor tree: **124 of the 145 templated
returns write a native type beside the tag**, which is what modern PHP encourages, so the
substitution this project already had was reading one return in seven.

The tag and the hint are not two claims. A `@template T of X` is bounded by exactly what the hint
declares, so the tag says _which_ of the declared possibilities this call gets and can only be
narrower. The choice belongs where the owner's template list is in hand, which is `phrost-view`.

The hint is kept rather than discarded, and that is the load-bearing half: a `@template` class is
often written with no arguments -- Symfony's own `Request` declares `@var InputBag<string> $query`
beside a bare `InputBag $request` -- and what the declaration wrote is then a real type and the
better answer. With no hint the template's bound answers, as it already did; only with neither is
the answer nothing.

**And a union with a template in it answered nothing at all**, because a union is not a template
name, so every substitution declined and what was left was a union of names resolving to no class.
38 of those 145. Every operand has to answer, for the reason a list literal's every entry does: a
union with an unsubstituted operand dropped is narrower than the truth, and a type narrower than the
truth reports correct code. `NodeFinder::findFirstInstanceOf(): ?Node` with `@return null|TNode` is
the shape, and it now answers `?Call`.

Ratchet held; coverage 44 921 -> 44 924 resolved and untyped receivers 632 -> 629. No cost on the
diagnostics path: interleaved `check` runs of both binaries were 2.77/2.76/2.61 s against
2.75/2.76/2.73 s, bands overlapping.

**The three things a `@return T` can say, and each was refused for a reason that had expired --
done.** `@return T` bound to a `@param class-string<T>` is the factory shape, and the declaration
beside it says `object` or `array` or `?Node`: whether the result is the class, the class _or null_,
or a _container_ of it is written only in the tag.

The null was refused whole, on a doc comment saying it "says more than the result than the argument
can answer". It does not -- the null belongs to the declaration, exactly as `@return ?T` beside a
native hint already does, and `class-string<T>` says which class and nothing about whether one was
found. Cost of the refusal, measured: `NodeFinder::findFirstInstanceOf($stmts, FuncCall::class)` was
a `?Node`, and `$first?->name` was then reported **undefined-property on code that runs**.

The container was never expressible. `findInstanceOf` sits four lines from `findFirstInstanceOf` in
the same class and differs only in this; Doctrine's `findAll()` beside its `find()`. Both were a
bare `array`.

Three spellings of the null, because a real library writes each: `?T`, `T|null`, and the
`null|TNode` `NodeFinder` writes -- mago's docblock union is binary, so it sits on whichever side
the author put it. And `?T[]` wraps the container _inside_ the null: "an array, or nothing", never
"an array of nothings".

**And a template in an element position was not substituted at all**, in either path. A class-level
`@return T[]` leaked `T[]` into hover -- a variable name in a tooltip, the same shape as the
`static[]` leak this document already records, and strictly worse than the `array` the hint says.
The module's own note said `list<T>` could not be built because its base is a builtin with no class
to hang arguments off; `Held::ListOf` is built rather than interned and had been sitting there since
the list-literal work.

**Which is where the real find was: the shape was right and the _tag_ was wrong.** Doctrine's
`find()` writes two return tags --

```text
@return object|null The entity instance or NULL if the entity can not be found.
@phpstan-return ?T
```

-- and four readers of that tag all took whichever came first. So `find()` answered `object|null`
while `findOneBy()` four lines below, which writes only the prefixed tag, answered the entity. The
plain tag exists for a reader that cannot hold the precise one; it is not a second opinion. One
helper, four callers, and every tag kind rather than just `@return`: `@phpstan-param` is **588**
occurrences on that vendor tree against `@phpstan-return`'s 381, `@psalm-return` 96, and 524 of the
lot in `doctrine/`.

Measured on the reference project: `undefined-method` **8 -> 6**, and both that went were the same
Drupal-core shape (`ListInterface::get()` writes `@return TypedDataInterface|null` beside
`@phpstan-return ?T`). `phrost coverage` went _down_ -- 44 924 resolved to 44 918, untyped receivers
629 to 637 -- and those six are those same sites: the chain used to answer `TypedDataInterface`,
whose declared surface is almost empty, so the next call on it was reported undefined. **Six
confidently wrong answers became six honest silences**, which is the instrument counting confidence
rather than correctness. Getting both would mean keeping the plain tag beside the precise one and
falling back to it where nothing binds the template -- a second type per member row, on the largest
slice of the index heap. Priced, not built.

Also priced and not built: a report from outside named a _union in a neighbouring parameter_ as the
cause (`findFirstInstanceOf` takes `@param Node|Node[] $nodes`, which looks exactly like it). A
three-by-three matrix over the return and the neighbour settled it in one run: the neighbour is
innocent and the return decides. Worth keeping as a habit -- two probes that differ in two things
cannot name either.

**A union mixing a _method_ template with a _class_ one -- done, and it was built for the form
rather than the catch.** Symfony's `InputBag::get` is `@return TDefault|TInput`, where `TDefault` is
the type of an argument and `TInput` is what the receiver was written with -- `Request` declares
`@var InputBag<string> $query`. Two resolvers answer "what does this template mean": the contract
reader, handed the call and its written arguments, and the generics substitution, handed the
receiver. Each is complete for its own kind and blind to the other's, so a union spanning both
resolved in neither and fell back to the declared `string|int|float|bool|null`.

**The size argues against it and was the wrong measure.** Counted across a 300 MB vendor tree: of 56
union returns naming a template, 48 are `T|null` with a single one (already answered), 7 gain
nothing when resolved (`RequestedType|mixed` folds to `mixed`), and **one** is this shape. What
earns it is that "two resolvers, each complete for its own kind and blind to the other's" is a form
this codebase met four times before -- the element half against the key half, the written refiner
against the interned one, two union constructors, two readers of a return tag -- and every previous
fix treated the instance. `CallFacts::receiver` is the seam that removes it: both halves of what
settles a return now arrive in one place.

`ContractData::ReturnsOneOf` is the vocabulary, one `ReturnBranchData` per operand, which is what
`ReturnsApplied` already used for "read this off an argument". Every operand has to answer, for the
reason a list literal's every entry does -- a receiver written without arguments
(`Request::$request` is a plain `InputBag`) declines the whole union and the declaration stands,
because answering it as written would put a variable called `TInput` in a tooltip.

Measured: `$request->query->get('number', '')` is a `string`, and three findings went with it --
`incompatible-argument` 79 -> 78 and `possibly-null-argument` 245 -> 243, all three confirmed false
positives from outside. 17/17 budgets, diagnostics p95 20.22 ms.

One thing fell out that is worth more than the entry: two spellings of one value do not fold. The
receiver's `@var InputBag<string>` arrives interned and the `''` beside it at the call is a
`Held::Builtin`, so `Held::one_of`'s repeat fold left `string|string` in a tooltip. Canonicalised
where the new `Held` is built, which is the same three-spellings rule this document keeps recording
-- and the general form of it, `one_of` folding by _meaning_ rather than by shape, is still open.

**An argument judged against a template parameter -- done, and it was wrong in both directions at
once.** `@param T $item` is not a type: `T` resolves to no class, so the judgement rejected every
scalar for not being one and accepted every class because nothing relates to it. Measured on a probe
before the fix: **five findings, all false, and two genuinely wrong classes silently accepted**. The
carrier is the standard library -- `Ds\Sequence::push(...$values)` is `@param TValue`, so
`$deque->push(1)` reported `$values takes \Ds\TValue` on code that runs.

What settles it is what the receiver was written with, which the one place that resolved the
receiver already had and threw away: `parameters_of` reduced it to a class and returned the
parameter list. `CalleeTemplates` travels with the parameters now, and the expectation is
substituted before the judgement _and_ before the message -- a report naming `T` tells a reader
nothing they can act on.

A receiver written without arguments settles nothing and there the parameter is not judged at all.
And a _container_ of a template declines, because the substitution has to produce an interned id and
rebuilding `array<int, Row>` needs the table open for writing -- a miss that replaces an answer
which was accidentally right for a scalar and accidentally wrong for a class.

**And an enum was reported for not declaring what PHP declares for it.** `cases()` arrives with the
provenance of an interface requirement, which was already handled by requiring a supertype to be
_written_ -- PHP forbids `enum X implements UnitEnum`. What was not handled is that an interface may
**extend** it: `interface HasCases extends UnitEnum` with `enum Stage implements HasCases` is valid
PHP and puts `UnitEnum` into the written set. The vendored standard library carries two of its own.

Both were found by running `check` over the vendored prelude, which is worth doing for exactly this:
48 findings there, **46 of them defects in mago's stub set** and recorded in its README rather than
edited away, and 2 of them ours. Ratchet held on the reference project, 17/17 budgets, and no cost
on the diagnostics path -- interleaved `check` runs were 2.75/2.76/2.77 s against 2.71/2.71/2.35 s.

**A misspelt hook name -- done, and the catalog it needed had to come from the other side.** A typo
in `$moduleHandler->invokeAll('notifications_altr')` throws nothing, logs nothing and returns an
empty array: dispatch is by name, the kernel finds no `<extension>_notifications_altr`, and that is
a legitimate outcome for a hook nobody has implemented. It is the one runtime mistake that leaves no
trace at all.

There was already a catalog of hook names and it **cannot** be measured against, because the call
being checked is what fills it -- filled from the project's own `invokeAll` and `alter` calls, a
typo registers itself. That is why the near-miss reader was silent here while it reported a misspelt
route, and the refusal recorded here last time was right about the mechanism and wrong about the
conclusion: a report from outside then measured the sources that are _independent_ of the dispatch
site, and there are three. 333 `function hook_x()` declarations in `*.api.php`, 1 601 `#[Hook('x')]`
attributes across core and contrib, and the names of the functions that implement hooks.

The third is the one that mattered, and only measurement said so: all four hooks the reference
project dispatches itself have **no** `*.api.php` and no attribute, and an implementation each. A
hook implementation's name _is_ the contract -- `app_core_notifications_alter()` implements
`notifications_alter` because it is named `<extension>_<hook>` -- so the name is the only place to
read it from, split against the `extension` catalog with the longest prefix winning.

`hook` goes from 7 keys to **1 038**, kept apart from the dispatch catalog rather than merged,
because the two answer different questions about one name. Open and unclosable: a hook nobody has
implemented is an ordinary extension point.

`#[Hook('x')]` is the fourth source and is _not_ built, with a reason worth keeping: the obvious
mechanism is `AttributeKey`, whose claim records the annotated class as what the key names -- and
`invokeAll` is bound to this catalog, so `invokeAll('form_alter')` would then be typed as the hook
class. The same trap `entities.rs` documents for `getStorage`, reached from a third direction.

**And the bridge's other half, which was one line and the larger one.** `@return TDefault|TInput`
bound `TDefault` from the _written_ argument, so `get('token', '')` answered `string` and
`get('token')` fell back to the whole declared union. What PHP puts there is the parameter's own
default -- `mixed $default = null` -- so the answer is `?string`. Measured from outside: **5 calls
write the default and 62 leave it out**, so reading the argument alone had answered the smaller
half. Recorded on the branch rather than on every parameter row, which is the largest slice of the
index heap.

Catalog keys 2 823 -> 2 831, registered 2 108 -> 2 115. Ratchet held, 17/17 budgets, index heap
unmoved at 104.5 MB, bootstrap unmoved: 3.34/3.37/3.36 s against 3.33/3.34/3.20 s.

**`@param-out` into the type layer.** 145 tags in the corpus, 129 of them on untyped by-reference
parameters. Read by hover today and by nothing else.

---

## Landed 2026-09-20: thirteen checks, and PHP as the instrument that priced them

A `php` binary turned up on the machine -- 8.5.10 -- and it changed how a check gets written here.
The method for every one below was the same: put the rule in a file, run it, read what PHP says, and
only then decide what the check claims. It is faster than reading an RFC and it disagreed with one
four times.

What landed, in the order the evidence arrived:

| code                          | what it is                                                     | measured                                                        |
| ----------------------------- | -------------------------------------------------------------- | --------------------------------------------------------------- |
| `discarded-result`            | a `#[\NoDiscard]` result thrown away (PHP 8.5)                 | 0 on five                                                       |
| `doc-redundant-type`          | `@return T[]\|array`, which says only `array`                  | 17 on the reference project, 7 on the corpora, 4 in Drupal core |
| `nothing-to-override`         | a `#[\Override]` with nothing above it                         | 0 of 3 541 uses                                                 |
| `sealed-parent`               | a class extending a `final` one                                | 0 on five                                                       |
| `redeclared-member`           | a name a class declares twice                                  | 0 on five, 9 on the stubs                                       |
| `twice-declared`              | a class two of the project's own files declare                 | 0 on five                                                       |
| `trait-collision`             | two traits giving one class the same method                    | 0 on five                                                       |
| `refused-declaration`         | seven declarations PHP will not compile                        | 0 on five, 1 on the stubs                                       |
| `out-of-context`              | `$this`, `self`, `static`, `parent` with nothing to mean       | 0 on five                                                       |
| `impossible-use`              | a value invoked, cloned or used as a class name that cannot be | 0 on five                                                       |
| `bad-string-offset`           | `$s['word']`                                                   | 1 on WordPress, 3 in Drupal core, all true                      |
| `refused-return`              | a `return` or `yield` the declared type refuses                | 0 on five                                                       |
| a static property's set scope | the half `readonly-write` was missing                          | 0 on five                                                       |

Three things worth keeping from how they were built.

**A mistake that stops a file loading has a population of zero in code that ships.** So a corpus
cannot say a check is _useful_ -- only that it is quiet. What it _can_ do is find the boundary, and
it did every time: `redeclared-member` reported 20 `@property` tags, `trait-collision` six shapes
PHP resolves, `out-of-context` 913 positions `Closure::bind` settles, `refused-return` 79 generators
and arrow functions. Every one of those was the check being wrong about PHP rather than the code
being wrong.

**The stub set and Drupal core are the two corpora where these do occur.** `php-prelude` is
declaration-only and generated, and it holds nine redeclared members and an enum with a property.
Drupal core is eleven thousand files nothing here had been pointed at; it has no `vendor/` of its
own so its totals are noise, but a new check's rows are readable and three of them were true.

**`MemberTable::conflicts` had been computed for a year under a doc-comment stating PHP's rule, and
read by nobody.** The moment something reported it, four of seven matrix rows were wrong. A field
whose comment is the specification and whose only reader is the thing that avoids acting on it will
drift, and nothing fails.

## Landed 2026-09-19: a pass over what was silent and what was wrong

Thirteen changes, each measured against five corpora and the reference project, ratchet re-armed
after each. What they have in common is that none of them was on this list: the list was ranked by a
corpus score, and every one of these came out of _probing the server by hand_ against shapes a
person writes.

**Four false positives on code that runs.** `is_a($x, Foo::class)` did not narrow, so the call
inside the guard was `undefined-method` -- and `is_subclass_of` has to be read the other way round,
because it accepts a class _name_ by default and reading it as a claim about a value reported three
correct lines on WordPress. `match (true)` narrowed nothing at all, so a guard written the modern
way reported the value it had just proven safe; 13 findings on Laravel and Symfony. A by-reference
call left the caller's `null` in place, so `$x = null; fills($x); needString($x);` was a hard error
on the ordinary shape of a populator. And `protected` was judged against the class the value was
_reached through_ rather than the class that declares the member, which is neither necessary nor
sufficient -- sixteen in one Laravel test file.

**Two whole shapes that had no type.** An anonymous class's value
(`$double = new class implements Contract { ... }`) and `clone $row`. Both were lowered as
unmodelled expressions, so every call on one answered nothing and every rule about it was silent.

**Four checks that were not asked.** A write to a `readonly` property or through a `private(set)`
scope (`readonly-write`); an array key one edit from a declared one (`misspelt-key`); an element
read through a property or a call rather than a plain variable; and the null a `?->` puts on the
whole expression, which no check carried -- `return User::load($id)?->hasPermission($p);` out of a
`: bool` is three real ones on the reference project.

**Three answers that were narrower than the truth.** An unconditional `@phpstan-assert` was gated on
the value being vague, which skipped `assertInstanceOf` -- the commonest assertion there is -- and
cost 326 findings on psl alone. A name the file itself tests for with `function_exists` or
`class_exists` was reported as undefined, 683 across four corpora. And a templated trait member
(`@use Holder<Widget>`) resolved to nothing while the identical `@extends` shape answered.

**One editor position.** `implements`, `extends` and `#[` offered the whole catalog -- the trait,
the enum and the free function beside the one interface that can stand there.

Net on the corpora: psl `undefined-property` 480 -> 167, Symfony `undefined-method` 1462 -> 1397 and
`undefined-type` 1204 -> 1162, Laravel `undefined-method` 1510 -> 1479, WordPress
`undefined-function` 36 -> 15. Every rise was read in the source first. 17/17 budgets, diagnostics
p95 10.97 ms against a 30 ms budget, `check` over the reference project unmoved at 228 ms.

---

## Landed 2026-09-19, second pass: what a server _says_, probed by hand

The 2026-09-19 note above records that none of its thirteen changes came off this list, and that
every one came from driving the server by hand. The same loop, run again over the editor requests
rather than the checks, found five more. All of them are things no corpus score can see, because no
corpus score reads what a request answers.

**Two false reports the stubs' own prose already denied.** `base64_decode($s)` cannot return `false`
without `$strict`, and the vendored set says so in a sentence above a type that does not -- 84
findings across four corpora, fixed by writing the sentence as
`@return ($strict is true ? string|false : string)`, which the conditional machine already read.
`hex2bin('400a6375')` cannot fail for an even number of hex digits, which is the whole of its
contract -- 47 more, 42 of them one psl test file writing RFC examples out by hand.

**Signature help counted commas.** `store(overwrite: true)` fills the third parameter from the first
position, and a named argument was read at its index and then refused outright once anything before
the cursor was named: of eight ways a person writes a named call, three pointed at the wrong
parameter and three at none. `ArgumentSite::name` was there the whole time, and `free_parameters`
three functions down the same file had been reading it.

**PHP writes three things with `use`, and completion answered all three as the first.** A trait use
in a class body was offered as a root-anchored import path, so accepting one left
`use Ed\Ed\Timestamps;` -- a fatal error taken from a list. A closure's `use (...)` offered 198
class names where only a variable can stand.

**A type position had no types in it.** `function run(): st|` answered `Stack` and eighteen
`stats_cdf_*` functions and not `string`; `vo|` answered `Volatile` and not `void`. `NamePlace` knew
`new`, `implements`, `extends` and `#[` and had no notion of a type, so every signature, property
and docblock tag was `NameKinds::Any` -- all three of PHP's tables. `TypePlace` names six, with the
keyword list each one's language allows.

### Landed: `#[` offers the attributes first

_(Was: found in the same sweep and not built. The entry below is kept because its reasoning was half
right and the half that was wrong is the useful part.)_

Built as a **third tier in the page** rather than as a filter or as a bit on the catalog row. The
entry below is right that a filter before the cap walks every class in the project for a bare `#[`,
and right that a lookup per candidate has the same shape -- and wrong to conclude from that that the
bit has to live in `Declarer`. What the position needs is a list of the few hundred classes that
_are_ attributes, which is `ProjectSkeleton` keeping their folded short names the way it already
keeps `never_returning` and `bound_scopes`. The tier is then filled from that list before the walk
starts, and the walk is untouched. Measured on the reference checkout: 441 attribute classes of 30
916, 21 787 places that write one, 17/17 budgets after.

Worth carrying forward for the plugin-id catalog below, which the entry pairs this with: the answer
there is probably the same shape, and _not_ a per-class bit in the catalog row.

### The original entry: `#[` offers every class

An attribute is an ordinary class to the catalog, so `#[|]` offers two hundred of them and the
handful a project declares `#[Attribute]` are not ranked first. `#[Rou|` in a Symfony project puts
`Route` among fifty other `R*` names.

What it costs is why it is here rather than done. The flag has to live in the **catalog row**, not
in `ClassSkeleton`: the filter runs inside `candidates_with_prefix`, before the cap, and a bare `#[`
has an empty prefix -- so a filter that rejects almost everything turns a scan that stops at 200
into one that walks every class in the project on every keystroke. A lookup per candidate has the
same shape. So it is `Declarer` plus the hir, the lowering, the cache codec and the dump invariant,
for a _ranking_ gain at one position rather than a wrong answer anywhere.

Worth doing, and worth doing as one piece with whatever else wants a per-class bit -- the Drupal
plugin-id catalog below is the other candidate.

## Landed after being put back once: a write a loop carries round

`$lowest = NULL; foreach ($grades as $grade) { if ($lowest === NULL || $grade->min() < $lowest->min()) { $lowest = $grade; } }`
-- the read of `$lowest` is **above** the only write that gives it a value, and on every pass but
the first that write has run. `local_binding` reads the writes before the position, so the name
answered _certainly null_, the guard beside it then had nothing left to narrow, and goto and hover
on `$lowest->getMinRating` were dead. Found by `compare-lsp` against intelephense on the reference
project, where it was one of the three remaining `reference-only` sites.

The back edge is real and the fix is three-quarters built. What it took, in the order the corpora
asked for it:

1. **`Node::Branch` learns `repeats`** -- a loop body, and not a `catch`, which shares the shape.
   `BodyFacts::carried_from` then answers where a write below the position runs from, always
   `Reach::Possible` and never certain, because the first pass reaches the read with whatever the
   name held before the loop.
2. **A carried write dies of any rebinding between the loop's start and the read.** It ran on the
   pass before, so a certain write inside the loop (`$value = $export[$name];` at the top of a
   `foreach`, read in a `switch` below it, written again in another arm) or the loop's own target
   (`foreach ($a as &$v)`) has run since. Without this the join made every such name a union of
   every shape it ever holds: WordPress `incompatible-argument` +71.
3. **The join is ranked at the loop's start, not where the write is.** What it holds arrived round
   the back edge, so a guard written between there and the read is later and is the more specific
   claim. Ranked off the write's own position, the join outranked every such guard and
   `if ($best !== NULL) { $best->m(); }` kept the null it had just ruled out: Symfony
   `possibly-null-receiver` +25.
4. **A carried write whose value reads the name it writes is refused.** `$input = substr($input, 3)`
   is a fixed point rather than a new value, and joining it resolves the name through itself until
   the budget stops -- which widens a step at a time and ends in the union of everything the name
   ever holds. WordPress `incompatible-argument` +24 was this one function.

After those four the residue was **11 findings**, and the nine unread ones were enough to put the
whole patch back rather than accept them -- which is what turned the last two up:

5. **An element write never feeds the `created` fallback when it is carried.** That fallback says
   the variable was _brought into being_ by a write into a slot, which a write from the pass before
   is not.
   `foreach ($places as $value) { if ($value instanceof \BackedEnum) { ... } $value['name'] ??= $key; }`
   answered `array` above the `instanceof` that had just said otherwise -- the one finding in the
   set that was a wrong answer rather than a precision gain.
6. **The fixed-point refusal generalised from one variable to the loop.**
   `$input = substr($input, 3)` depends on itself; `$dir = $renamed . SEP` beside
   `$renamed = substr_replace($dir, ...)` depends on itself one variable away, and a per-name test
   cannot see the second. The rule is _depends on anything this body also assigns_, which keeps
   `$lowest = $grade` -- the loop's own target is bound by the header and no assignment touches it.

The residue is now **three**, all read. Two are sound: `getHeader(): ?HeaderSegment` is called
without a check, and pclzip writes `$v_pos_entry = ftell($fd)` below a read that passes it to
`fseek`, where `ftell` really is `int|false` on the second pass. The third is a false positive and
its cause is elsewhere: Symfony's `VfsIteratorTestTrait` writes `$fileEntry = false;` and `break`s,
and `Leaving` models `return`, `throw` and `exit` and **not `break`** -- so a write the loop can
never come back from is joined. Recorded rather than fixed here: `break` leaves the loop and not the
function, so it cannot simply become `Leaving::Always`, and a position after the loop is still
reachable from it. That is the next thing this area wants.

## How to pick a piece of work

The loop that produced everything above, in order:

1. **Measure.** `just corpus <dir>`, `phrost coverage <project>`,
   `phrost check <project> --ratchet`, `just perf <project>`. Four instruments, four different
   questions.
2. **Let the data rank it** — but read the failures, not the score. Twenty-five of the sixty-two
   corpus failures are positions already taken; a bucket count that does not separate those from
   real gaps ranks a disagreement above a bug.
3. **Build the top one.**
4. **Re-measure with all four.** A corpus gain that costs a coverage regression is not a gain, and
   the ratchet is what catches a new false positive on real code.
5. **Verify every new finding by hand** before accepting a ratchet rise. Every rise this project has
   accepted was read in the user's source first.
6. **Commit.** One coherent step, present tense, describing the behaviour change.

Three failure modes this project has already paid for:

- **Run perf sweeps sequentially, on an idle machine.** A concurrent `cargo build` inflates
  bootstrap from 2.4 s to 4.4 s, which reads exactly like a regression.
- **Rebuild the release binary after applying or reverting a patch.** Measuring with a stale binary
  has cost this project an afternoon twice, once by writing a wrong ratchet baseline.
- **Read the fixture before believing its name.** `type/relative_docblock_union` sounds like a
  namespace-resolution bug and is union leniency; the two failing `signature_help` fixtures sound
  like a missing feature and are the end-of-file recovery limit. Both were classified wrong from
  their titles in this session before the files were opened.
