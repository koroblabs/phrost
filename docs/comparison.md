# Where phrost stands

Not a plan document — the three numbered ones own the plan. This one reports what other PHP language
servers answer that phrost does not, and what a session with each of them costs.

Every row is produced by `just compare`, and every row can be produced again. Nothing here is copied
from anyone's README, including ours: the point of the exercise is that the ranked list at the end
tells us what to build next, and a number we cannot re-derive cannot do that job.

```
just compare <project> --reference intelephense
just compare <project> --reference phpantom
just session-cost <project> --servers phrost,phpantom
```

Two instruments, two questions. `compare` asks **what a server knows**; `session-cost` asks **what
it costs to know it**, and the second exists because the first cannot answer it -- see the last
section.

## 2026-11-05, against phpantom at `87ae5faf`

phpantom `0.11.1-387-g87ae5faf`, 149 commits after the last section (no release since 0.11.1),
against phrost `4b7e2e98`. One machine, every run sequential, idle except where noted.

**What changed on their side**, read from `docs/CHANGELOG.md`: almost all of it is framework
breadth, much of it from new outside contributors. Symfony gained services declared by bundles and
the container builder, route names, translation keys and Twig (templates, blocks, filters,
functions, variables). WordPress gained hook names across `add_action()`/`do_action()`, `@global`
typing, stub packages installed with Composer and return types read from arguments. Yii gained views
and routes. Laravel gained validation rules, middleware names, resource routes, a routes CodeLens,
unknown Eloquent methods and schema dumps. PHPUnit gained data-provider checks, `setUp()` without
the parent and a "create test" action. Beside that: enum cases as types, conditional returns inside
unions, variables an `include` defines, and typed properties nothing initializes.

**What each knows** -- sites only one server answers (`compare`):

|                     | definition, only phrost | definition, only them | hover, only phrost | hover, only them |
| ------------------- | ----------------------: | --------------------: | -----------------: | ---------------: |
| Drupal (reference)  |                      21 |                 **0** |                 27 |            **0** |
| `laravel/framework` |                      37 |                     1 |                 38 |                2 |
| Symfony             |                      57 |                 **0** |                 54 |            **0** |
| WordPress           |                       8 |                 **0** |                 11 |            **0** |
| BookStack           |                      14 |                     2 |                  9 |            **0** |
| Monica              |                      20 |                     1 |                 12 |                1 |
| laravel.io          |                      18 |                    16 |                 20 |                2 |

The laravel.io sixteen are twelve Blade implicit variables (`$slot`, `$attributes`, `$errors`) whose
target phpantom gives as line 0 of the template, one `$loop` it sends to its `@foreach`, and
`$article->id`/`$thread->id` -- which is a real gap. laravel.io squashed its migrations into
`database/schema/mysql-schema.sql`, and phrost reads migrations only, so no model of that project
has a column: of its 347 unresolved member accesses, `id` alone is 71 and the next ten are almost
all columns. phpantom reads the dump. The completion residue is the two shapes recorded before:
model methods offered on every model through `::`, and items a typed prefix would filter.

**This instrument cannot see most of what they built.** `compare` samples PHP identifiers, so a
route name, a translation key, a hook name or anything in a Twig file is outside it -- in both
directions, since phrost's own Drupal catalogs are invisible to it too. And no Symfony _application_
is checked out here (`corpora/symfony-app` holds no PHP), so the Symfony features above are
unmeasured rather than absent from the table.

**Typing conformance** on a fresh copy of the suite at `3e0471c`: phrost **96.9 %** (283/292),
phpantom **96.6 %** (282, from 95.2), PHPStan 93.5 %. Of phrost's nine failures phpantom passes
three: sealed shapes (the 2026-10-23 position), `this_out_self_out` (the suite's markers), and
`properties_uninitialized_read`, which is their new uninitialized-property check.

**A session** (`session-cost`, phrost warm, twenty files opened):

|                     | says ready | truly quiet | total CPU |   idle |    end |
| ------------------- | ---------: | ----------: | --------: | -----: | -----: |
| Drupal, phrost      |      1.2 s |       2.2 s |     7.5 s | 230 MB | 231 MB |
| Drupal, phpantom    |      1.5 s |       9.6 s |    30.1 s | 922 MB | 925 MB |
| Laravel, phrost     |      0.5 s |       2.6 s |     8.3 s | 314 MB | 245 MB |
| Laravel, phpantom   |      1.1 s |       3.1 s |    11.1 s | 490 MB | 458 MB |
| BookStack, phrost   |      0.6 s |       1.6 s |     2.8 s | 168 MB | 171 MB |
| BookStack, phpantom |      1.0 s |       2.0 s |     4.5 s | 205 MB | 212 MB |

**A first reading said phpantom had halved its Laravel cost, 217 MB idle, and it had not.** That run
overlapped another project's release build. Under the pressure macOS compresses pages, and `ps`
reports RSS after compression, so a server can read lighter for no reason of its own. Re-taken on a
quieter machine it is 490 MB, where it was 483 on 2026-10-25. `vmmap --summary` is the instrument
that cannot be fooled this way, and it splits what RSS lumps together. On `laravel/framework` 65 s
after start:

|          |    RSS | physical footprint | malloc live | malloc fragmentation |
| -------- | -----: | -----------------: | ----------: | -------------------: |
| phrost   | 298 MB |             176 MB |  **106 MB** |         84 MB (45 %) |
| phpantom | 475 MB |             376 MB |      318 MB |         68 MB (18 %) |

phrost holds a third of the live data, and its RSS is mostly what the allocator keeps after the
background check of every file. The index does not grow during that pass, 62.8 MB before and after
on that project. The growth is the bounded memo of flattened member tables, about 45 MB of it, and
fragmentation for the rest. `MallocLargeCache=0` takes idle RSS from 337 to 265 MB, and
`release_freed()` cannot, because macOS 26's allocator ignores the call (see `lspkit-heap`).

Cold, phpantom is still first to ready on every project `compare` started: 0.1-1.1 s against
phrost's 0.3-2.0. That is a design difference rather than a slow path. phpantom walks no vendor
directory at start: it takes names from Composer's classmap or a byte-level scanner and parses a
dependency when something first needs it. phrost lowers every file it discovers into the skeleton,
13-27 thousand dependency files here, and that is 80-90 % of a cold start. Warm, phrost says ready
first on all three session projects.

**Whole project from the command line**, `phrost check` against `phpantom_lsp analyze`:

|                                                 |        phrost |                    phpantom |
| ----------------------------------------------- | ------------: | --------------------------: |
| Drupal (phpantom over `docroot/modules/custom`) | 2.5 s, 290 MB |               2.2 s, 641 MB |
| Laravel                                         | 3.1 s, 338 MB | 3.3 s, 634 MB (was 13-33 s) |
| Symfony                                         | 4.5 s, 343 MB |               9.5 s, 835 MB |
| WordPress                                       | 1.2 s, 240 MB |   4.4 s, 631 MB (was 1.4 s) |
| BookStack                                       | 1.6 s, 214 MB |               1.2 s, 224 MB |

## 2026-10-25, against phpantom at `7bb6686`, on plain PHP

phpantom `0.11.1-238-g7bb66868` (built with `cargo +1.99.0`) against phrost `d7867f55`, on the four
projects where a framework is the least of what is being asked -- the Drupal reference, and the
`laravel/framework`, Symfony and WordPress sources -- since Blade and application conventions are
already covered above. One idle machine, every run sequential.

**What each knows** -- sites only one server answers:

|                     | definition, only phrost | definition, only them | hover, only phrost | hover, only them | completion labels only them |
| ------------------- | ----------------------: | --------------------: | -----------------: | ---------------: | --------------------------: |
| Drupal (reference)  |                       5 |                 **0** |                 11 |            **0** |                           8 |
| `laravel/framework` |                      11 |                     1 |                 38 |                2 |                          91 |
| Symfony             |                      38 |                 **0** |                 54 |            **0** |                           1 |
| WordPress           |                       7 |                 **0** |                 21 |            **0** |                           0 |

The Laravel residue is one Blade `$body` (phpantom's target is line 0 of the template), a virtual
`afterCommit` on a mailable, and completion on phpredis and Memcached, whose methods phrost's stub
set lacks. The one label on Symfony is `Attribute::TARGET_CONSTANT`, new in PHP 8.5.

**Typing conformance**, both run fresh on one copy of `zonuexe/php-typing-conformance` at `438824e`,
which has grown from 249 tests to 292: phpantom **95.2 %** (278), PHPStan 93.5 % (273, stored),
phrost 93.2 % (272), mago 87.3 %. phrost passes 240 of the 249 it had before; eleven of its twenty
failures are among the 43 new tests, which the suite drafted partly from recent analyzer changelogs,
phpantom's among them. phpantom passes thirteen that phrost fails, and PHPStan passes all thirteen:
`array_fill` of a string, a write through a by-reference parameter, strict `in_array` narrowing, an
array callable invoked, `new ArrayIterator($xs)` inference, `GMP` and `BcMath\Number` operators, an
implicit `Stringable`, `new` on a union of class strings, `@phpstan-this-out`, `CONST[T]` offsets,
`[null]` element subtraction, an `assert-if-true` to `non-empty-string`, and shapes sealed by
default -- the last a position phrost reversed on 2026-10-23 after it misfired on the reference
project. phrost passes seven that phpantom fails.

**Latency**, median / p95 ms over one pass of each `compare` run (indicative, not a perf sweep):

|                           | completion         | definition         | hover              |
| ------------------------- | ------------------ | ------------------ | ------------------ |
| Drupal, phrost / phpantom | 0.3/2.3 · 0.8/4.2  | 0.1/2.5 · 0.2/3.4  | 0.1/0.2 · 0.3/4.5  |
| Laravel                   | 0.7/6.3 · 1.7/22.8 | 0.1/3.9 · 0.4/11.6 | 0.1/0.3 · 0.5/18.5 |
| Symfony                   | 0.6/5.2 · 0.4/16.2 | 0.3/6.5 · 0.2/2.7  | 0.2/0.6 · 0.2/2.8  |
| WordPress                 | 0.3/0.8 · 0.6/3.0  | 0.1/3.4 · 0.1/0.5  | 0.1/0.3 · 0.3/2.1  |

**Memory**, at ready and after the 26-39 files a `compare` run opens:

|           |        phrost |          phpantom |
| --------- | ------------: | ----------------: |
| Drupal    | 265 -> 284 MB | 315 -> **899** MB |
| Laravel   | 189 -> 305 MB |     355 -> 569 MB |
| Symfony   | 294 -> 279 MB |     217 -> 742 MB |
| WordPress | 124 -> 189 MB |     306 -> 360 MB |

**A session** (`session-cost`, phrost warm, twenty files opened):

|                   | says ready | truly quiet | total CPU |   idle |    end |
| ----------------- | ---------: | ----------: | --------: | -----: | -----: |
| Drupal, phrost    |      1.2 s |       2.2 s |     8.8 s | 228 MB | 229 MB |
| Drupal, phpantom  |      1.9 s |      10.0 s |    39.0 s | 904 MB | 907 MB |
| Laravel, phrost   |      0.6 s |       2.6 s |     8.9 s | 317 MB | 278 MB |
| Laravel, phpantom |      1.2 s |      14.3 s |    17.4 s | 483 MB | 487 MB |

phrost's CPU on Laravel includes diagnosing 979 files in the background; phpantom diagnoses the
twenty that are open. A _cold_ phrost -- the first start after phrost itself changes -- still comes
second: 2.5 s against 1.4 on Drupal, 1.2 against 0.5 on Laravel, 1.8 against 0.3 on Symfony.

**A whole-project check from the command line**, `phrost check` against `phpantom_lsp analyze`, wall
time and peak RSS, repeated where the first reading looked out of line:

|                                                 |            phrost |                                  phpantom |
| ----------------------------------------------- | ----------------: | ----------------------------------------: |
| Drupal (phpantom over `docroot/modules/custom`) |     2.0 s, 303 MB |                             2.5 s, 657 MB |
| Symfony                                         |     3.9 s, 331 MB |                         8.6-8.8 s, 875 MB |
| Laravel                                         | 2.7-2.8 s, 342 MB | 13.0-32.7 s (three of four 32.5+), 640 MB |
| WordPress                                       |     1.0 s, 244 MB |                             1.4 s, 497 MB |

The finding counts are not compared, because they are not answers to one question: 56 720 of
phpantom's Symfony findings are PHPUnit methods in a checkout that does not install PHPUnit. What
was compared is Drupal, where the project passes its own PHPStan and phrost reports no type finding:
phpantom reports 353, 107 of them `FieldItemInterface` expected and `TypedDataInterface` given
(iterating a field item list), and most of the rest `Url::toString()`'s `string|GeneratedUrl`,
`reset()` on an entity query's `int|array`, and `strtotime('-1 day')` read as possibly `false`.

### The same day, after closing the gap

Seventeen commits later (`0b303dc2` through `ee9c9ac4`) the same suite reads phrost **96.9 %**
(283/292), phpantom 95.2 % (278), PHPStan 93.5 % (273). Of the thirteen phpantom passed and phrost
did not, eleven were rules phrost lacked and are built; two were not gaps.
`assertions_this_out_self_out` fails because the suite names the engines expected to honour
`@phpstan-self-out` in `E<...>` markers and phrost is not among them -- phrost reports the line.
Shapes sealed by default is the position taken on 2026-10-23.

Two of the eleven were positions, decided by the user rather than built around: an object whose
declared type has no relation to the expected one is now rejected whichever side is an interface, as
PHPStan does at level 5 (a downcast stays silent, which PHPStan does not do); and `non-empty-string`
is a type, with `string` where one is declared reported as PHPStan does at level 7. One was a gap
between two checks rather than a rule: `?int` reaching a `?string` was owned by neither the sentinel
check nor the type check.

What it cost, on the projects the instruments run against: the reference project and Slim **0** new
findings; Laravel +152, Symfony +202, WordPress -11, BookStack +9, Monica +7 net over the whole
pass, read by category -- the interface rule's are the sites PHPStan 2.2.12 reports at level 5, and
95 of `non-empty-string`'s 116 are the finding PHPStan gives on the same line. The 21 it does not
give are listed in `docs/08-types.md`; about eight are inference PHPStan has through extensions
(`preg_match` proving its subject, `substr` with a positive length, `str_replace` keeping a string
non-empty). Every request budget is unmoved on the reference project -- after one regression caught
on the way, code actions' p95 doubling when `class_header` started asking every class about
`__toString`.

What is left that either other engine passes: `switch (get_class())` narrowing, a partial member on
a union, `@var` invariance on a docblock property (PHPStan only), and the FOLLOW_LIMIT cut-off that
makes a conditional return answer both branches when its argument is out of reach.

## 2026-10-23, against phpantom at `bcfafca`

phpantom moved 86 more commits (`0.11.1-186-gbcfafcadb`, built with `cargo +1.99.0` in a scratch
clone), and phrost gained macros, collection callbacks, relation scopes and object shapes since the
last row, so the two applications are taken again:

|           | definition, only phrost | definition, only them | hover, only phrost | hover, only them | completion labels only them |
| --------- | ----------------------: | --------------------: | -----------------: | ---------------: | --------------------------: |
| BookStack |                      10 |                     2 |                  9 |            **0** |              27 (32 before) |
| Monica    |                      17 |                     1 |                 12 |                1 |            259 (268 before) |

Definition and hover are at parity or past it; the one Monica site phpantom answers alone is a
migration column read through a property nothing types. The completion labels left are almost all
offers phrost declines on purpose, and reading the sites says which: `wherePivot` and `pivot` on
_every_ model (they are a many-to-many relation's and a model it loaded, and phrost now types
`->pivot` where the model came through one), a model's instance methods after `::` (`User::update()`
runs on a fresh instance through `__callStatic`, which is never what it means), and a `where` for
every relation name. None is a type phrost lacks.

## 2026-10-12, against phpantom at `5d3fe03`

phpantom moved 51 commits in a day (`0.11.1-100-g5d3fe03`, built with `cargo +1.99.0`), so this is
re-taken against it, on two Laravel applications as well -- BookStack and Monica, where a
framework's conventions are most of what a model is -- and after phrost started reading migrations,
factories and one-argument collections (`docs/08-php.md`, `docs/08-types.md`).

**What each knows** -- sites only one server answers:

|                     | definition, only phrost | definition, only them | hover, only phrost | hover, only them | completion labels only them |
| ------------------- | ----------------------: | --------------------: | -----------------: | ---------------: | --------------------------: |
| Drupal (reference)  |                       3 |                 **0** |                 22 |            **0** |                           8 |
| `laravel/framework` |                      13 |                     1 |                 38 |                2 |                         125 |
| BookStack           |                      10 |                     2 |                  9 |            **0** |     32 (826 before the day) |
| Monica              |                      18 |                     1 |                 12 |                1 |          268 (1 265 before) |

What is left on the two applications is mostly what a list offers beyond the type: `wherePivot` and
`pivot` on a many-to-many relation, `increment()`/`decrement()` (protected, reached through
`__call`, which phrost does not report and does not offer), and a `where` for every relation name --
which phpantom offers and phrost declines, because each asks for a column that does not exist. On
`laravel/framework` it is extension stubs: phpredis and Memcached methods phrost's stub set lacks.

**Typing conformance**, both run fresh on one copy of `zonuexe/php-typing-conformance`: phrost
**96.0 %**, phpantom 94.0 % (it was 93.2 in the suite's own stored results), PHPStan 93.6.

**Ready**, start to the first correct answer about a project class, probed every 50 ms on BookStack,
three runs each:

|                                             |           ready |          RSS |
| ------------------------------------------- | --------------: | -----------: |
| phrost, warm (its stored index read back)   | **0.30-0.35 s** |       124 MB |
| phrost, cold (the stored index moved aside) |     0.79-1.17 s |   183-196 MB |
| phpantom (keeps no index)                   |     0.38-0.65 s | **72-76 MB** |

A warm phrost answers first; a cold one is a few hundred milliseconds behind, because it indexes the
declarations of every dependency where phpantom reads one when asked -- on Drupal 2.7 s against 1.6.
A start is cold once per project after phrost itself changes, since a stored index from another
build is refused, and warm every time after. Earlier runs of `compare` printed phrost a second
behind on every project, and that was the instrument twice over: it waited a whole second between
probes, so a server ready at 1.05 s read as 2.3, and it ran straight after a rebuild, so phrost was
cold every time. It probes every 50 ms now and times the answer rather than the drain after it.

## 2026-10-11, against phpantom 0.11.1 and intelephense 1.18.5

The sections below are older runs and keep their method notes; this is the latest reading of all
three servers, on the reference Drupal checkout and on the `laravel/framework` source, which is the
ground phpantom is built for. phpantom is `main` at `c4d52f5` (0.11.1 plus 49 commits), built beside
this repo. `compare` runs warm for phrost and for intelephense, which keep an index, and cold for
phpantom, which keeps none; `session-cost` is cold for both.

**What each knows** -- sites only one server answers, out of 636 on Drupal and 560 on Laravel:

|                       | definition, only phrost | definition, only them | hover, only phrost | hover, only them |
| --------------------- | ----------------------: | --------------------: | -----------------: | ---------------: |
| phpantom, Drupal      |                       3 |                 **0** |                 22 |            **0** |
| phpantom, Laravel     |                      12 |                     2 |                 36 |                4 |
| intelephense, Drupal  |                       0 |                 **0** |                  0 |            **0** |
| intelephense, Laravel |                      78 |                     1 |                 78 |                1 |

phpantom's Laravel row read 4 and 6 on the first run, and four of those were one feature phrost did
not have: inside a trait, `$this` read as the classes that use it -- `InteractsWithPivotTable`'s
`$this->parent`, and 474 completions on its `$this->`. Taken (`0aa624dc`), the row reads 2 and 4:
two variables, and two members read under `property_exists()` / `method_exists()`, which phrost does
not take as a guard. On Drupal every label phpantom still offers that phrost does not -- eight, from
`getAutowireArguments` at 27 sites to Prophecy's `countProphecyAssertions` -- is a _private_ member
of a trait a parent class uses, which a subclass cannot reach. intelephense's single Laravel site is
a `$post` the probe could not reproduce by hand.

**What it costs**, cold, twenty files opened, from `session-cost`:

|                                        |   ready |   quiet | processor s | idle MB |  end MB |
| -------------------------------------- | ------: | ------: | ----------: | ------: | ------: |
| phrost, Drupal                         |     2.3 |     3.3 |    **12.9** | **289** | **241** |
| phpantom, Drupal                       | **2.0** |     4.0 |       217.9 |   1 040 |   1 042 |
| phrost, Laravel skeleton (8 111 files) |     1.1 |     1.1 |     **1.5** |     123 |      98 |
| phpantom, Laravel skeleton             | **0.8** | **0.8** |         2.6 |  **81** |  **87** |

phpantom reaches ready a few hundred milliseconds sooner on both, and on a project that is almost
all `vendor/` it holds less, because it reads a dependency only when asked. On the large checkout it
keeps working after it says it is ready -- 171 processor-seconds in the minute after, and a whole
core through the thirty idle seconds at the end -- and holds a gigabyte. The `compare` runs read the
same way: phpantom 1 015 MB after its pass on Drupal and 1 154 MB on Laravel, intelephense 1 175 and
616, phrost 232-313. Per request, p95 hover is 0.2-0.7 ms here against 2.6 for phpantom and 4.0-5.7
for intelephense, and completion 1.9-4.7 against 4.3-8.6 and 2.8-5.5; intelephense's definition p95
(0.8-1.2 ms) is the one row lower than ours (2.0-5.9).

## Method

**Corpus.** A production Drupal 10 codebase: 30 965 PHP files discovered, 1 711 of them the
project's own, the rest Drupal core, contrib, and `vendor/`. Both servers were pointed at the same
checkout, one after the other, never at the same time.

**Questions.** Positions across 40 files, chosen by scanning the file _text_ — instance and static
member accesses, `new`, class-like names, variables. Never chosen from phrost's own index, which
would only ask about sites we already know about. `just compare … --sites-only` prints them. The
sample is redrawn each run: 692 in the intelephense run below, 722 in the phpantom one.

The sampler is deliberately blind, and that is about not confining it to the project's _own_ code:
this run reached into contrib and into a vendored PHPStan whose namespaces are prefixed, which is
where an editor's user reaches too.

**A tool's cache is not source, and this measurement was diluted by one for a long time.**
`.phpstan/ cache` holds one `<?php return [...];` blob per analysed file, with no declaration in any
of them, and on this checkout that is **11 272 of the 25 147 non-vendor PHP files — 45 % of the
sample space**, drawn first because the stride runs over a sorted list and a dot sorts early. Every
count in the previous version of this table was measured partly against generated data, and the
outline row was dominated by it: zero symbols is the right answer for a file with no symbols, from
either server. The skip list now names the tool caches, and every number below is from after that.
The tell was arithmetic: 2.4 outline entries per file, where a 339-line class yields 20.

The sample is redrawn every run, so a count here is only comparable to another count from the same
run. Where a section quotes a movement, both ends are from the same instrument — and where the
instrument changed, the number is restated rather than compared.

**Readiness.** Each server is asked for a class the sampled files declare, over `workspace/symbol`,
until its index has it. A server's own progress messages are not enough: they arrive in phases, and
believing the first one made a working server look like it answered nothing at all, a thousand times
over. See the commit that fixed it.

**Licence.** intelephense's key is passed when one is installed. Without it the server declines
eight capabilities and answers fewer questions, and a measurement that does not supply it is
measuring the free tier while claiming to measure the product.

**Memory** is read with `ps` from outside the process, which is the only way to ask a Rust server
and a Node server the same question. It is also the only way to ask one at all: no server reports
its own footprint, and phrost's `index --memory` -- which does, deterministically and per row -- has
no counterpart to compare against.

**Comparing item lists needs both spellings normalised, and getting that half-right is worse than
not doing it.** Servers write a completion label differently: `t`, `t()`, and
`t($string, $args = ..., $options = ...)` are one method. The harness stripped a trailing `()` and a
leading `$` and nothing else, so every method the other server labelled with its signature counted
as one we do not offer -- 968 such labels against phpantom, where a direct probe of one site found
both lists identical. The label is cut at the first `(` now, PHP having no overloading.

## Answers, against intelephense, 40 files, 688 positions

Re-measured on 2026-10-07 with intelephense's licence key installed -- without it eight of its
capabilities report `false` and the table flatters us -- and with the sampler no longer drawing from
a tool cache.

| Go to definition                 |       |
| -------------------------------- | ----: |
| both agree                       |   675 |
| both answered, different target  |    12 |
| **they answer, phrost does not** | **0** |
| phrost answers, they do not      |     0 |
| neither answers                  |     1 |

| Hover                                 |       |
| ------------------------------------- | ----: |
| both describe it                      |   687 |
| **they describe it, phrost does not** | **0** |
| phrost describes it, they do not      |     0 |
| neither                               |     1 |

The twelve that disagree are all the stub-source difference below -- `\InvalidArgumentException`,
`\RuntimeException`, `$e->getMessage` landing in the vendored prelude rather than in intelephense's
own set. Completion: 178 positions, both non-empty at 177, and five labels only intelephense offers,
every one a `protected` member reached from outside its class -- the shape the section below reads
in full. Indicative cost of the same run: phrost ready in 1.5 s at 220 MB, intelephense in 2.3 s at
1.1 GB growing to 2.2 GB; completion p95 2.1 ms against 8.4 ms.

What follows is the previous run's account, kept because its entries are the history of the gap.

**Both bolded columns used to be the point of this document.** They were 7 and 4; they are 1 and 0.
The one remaining is a `$format` variable. Every one of the seven the previous version listed is
closed, and the two entries it recorded as _refusals_ are worth keeping rather than the counts:

| what it was                                                                                                 | what happened                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `$index`, `$id`, `$kk` -- the **key binding site** of a `foreach`                                           | fixed: a scope that began at the body left the whole signature and every binding site outside four capabilities                                                                                                                                                                                                                                                                 |
| `SymfonyConstraint` -- the alias token of `use … as …`                                                      | fixed                                                                                                                                                                                                                                                                                                                                                                           |
| `?->willReturnCallback` -- the `?` is the harness's own placeholder                                         | never a real site                                                                                                                                                                                                                                                                                                                                                               |
| `@var FileUrlGeneratorInterface\|MockObject` -- a mock written with `\|` where the value is an intersection | **still refused.** `expects()` is on one operand only, so the chain dies at the first hop. Requiring a member on every operand of a union is what keeps `undefined-method` and the completion list honest, and the `&` spelling of the same mock resolves. intelephense answers by guessing; matching it would soften every member check in the project for one legacy docblock |

**The six we answer and they do not are all the framework seam** doing what it is for:
`Drupal::service('…')->start`, `->destroy`, `$this->container->get('…')->getCurrentTime`,
`$manager->getInstance`, `$entity_repository->loadEntityByUuid`, `$am->load`.

**And the ten that disagree are a difference of stub source, not of symbol.** `$e->getMessage`,
`new \Exception`, `$exception->getCode`, `\Traversable`, `\ReflectionObject`: ours land in the
vendored `mago-prelude` (`core.php`, `reflection.php`) and theirs in their own bundled set
(`Core_c.php`, `ReflectionObject.php`). Neither is wrong about which declaration it is.

### Completion, and why this table is not one

**181 positions, both non-empty at 170.** The eleven where we are silent and they are not is what
there is to work on, and the number is now small enough to open one by one rather than rank.

The per-label difference still reads badly and almost none of it is a missing feature: those counts
are the tail of the eleven, where we offered nothing at all and intelephense offered a whole list,
so every item in it counts once. `::class` _is_ offered, verified directly. The magic methods are a
real difference and a deliberate one: `__sleep` on `->` is not something anyone is reaching for.

#### Re-measured over 40 files and 3 820 positions, and the whole remaining gap is inaccessible

**Definition and hover are now zero reference-only in every site kind** -- name, var, static,
member, `new` -- against 3 753 and 3 815 agreements, with three sites we answer and they do not and
none the other way. The `$format` variable the row above records as the last one is closed.

Completion: **1 253 positions, both non-empty at 1 248, and 19 distinct labels only intelephense
offers.** Every one of those nineteen was read: `_serviceIds` and `_entityStorages` from Drupal's
`DependencySerializationTrait`, and `nodeStorage`, `userStorage`, `fileStorage`, `getUser`,
`createUser`, `setEntityTypeManager`, `entityTypeManager`, `typedDataManager`, `referencedTargetId`,
`referencedEntityFromItem`, `referencedParagraphFromItem`, `findParagraphItem`, `entityValues`,
`adjustParagraphsOrder`, `createEntity`, `updateEntity` from the project's own
`EntityManagerHelperTrait`, `ReferenceItemHelperTrait` and `EntityArrayHelperTrait`. **All of them
are `protected`, and every site is `$this->someService->`** -- a receiver of another class, where
PHP answers `Cannot access protected method`. Offering them is intelephense being wrong rather than
phrost being short, and it is the whole of the difference: the shape rows read `3 x 8.0`, `2 x 8.0`,
`2 x 8.0` -- the same eight inaccessible members of one service, counted once per site. That took a
detour worth recording. The labels read exactly like a gap in trait support, and three scratch
projects were built to find it -- a vendor trait, a trait reached through a vendor base class, two
traits in one `use` statement -- and all three resolved and completed correctly. What settled it was
reading the _declarations_: `protected function nodeStorage()`. **A label list says what a server
offers and not whether PHP would accept it, so a per-label diff has to be read against the
declaration before it is read as a gap.**

### Document symbols

phrost **227** entries, intelephense **772**, on this run's forty files. The gap is a difference of
policy rather than a shortfall, and the decomposition below is from an earlier run over a different
sample -- restated rather than re-measured, because the harness prints totals and not a breakdown:

|                                                               |   count |            |
| ------------------------------------------------------------- | ------: | ---------- |
| local variables inside methods                                | **379** | not adding |
| an anonymous class and its members, all in one generated file |      36 | not adding |
| a `Namespace` node per file                                   |      21 | not adding |
| `Closure` entries                                             |       3 | not adding |

Eighty-six per cent of it is locals. intelephense lists every `$var` bound in every method as a
child of that method -- seventeen in one `buildForm` alone. An outline drives the breadcrumb and the
symbol picker as well as the scroll, and flooding all three is not completeness. It is also part of
what its 6.5 ms p95 buys.

A kitchen-sink probe found nothing genuinely missing: constants, functions, interfaces, traits,
enums with cases, class constants, static and typed and nullable properties, promoted constructor
properties, abstract and static and magic methods, property hooks, several classes per file, and it
survives a dangling `->` at end of file. Properties differ in spelling only -- `FIELD`/`backends`
against `PROPERTY`/`$backends` -- and phrost's matches what its own completion says.

Document symbols now have a row in the perf sweep, so the margin cannot regress unseen: **0.04 ms
p95** in process. And the count is checkable directly, which is how the sampler's cache problem was
found: `ViewsData.php`, 339 lines and nineteen members, yields **20 entries** -- the class and every
one of its members.

### Diagnostics

phrost **88**, intelephense **47**, and the shapes say what the difference is. Ours are type
findings -- 17 `doc-missing-tags`, 5 `possibly-null-receiver` on `id()`, 4 `possibly-null-argument`,
plus `undefined-method` and `undefined-property` -- against theirs: 22 "no type information
available", 11 "declared but not used", 10 "undefined method", 2 deprecations.

The previous version of this table read 166 against 167, with 161 of each sharing one message shape.
That was the tool cache: thousands of generated `<?php return [...];` blobs, about which both
servers had the same one thing to say.

## Answers, against phpantom 0.10.0

A second reference, and a much closer one: another Rust server over the same mago parser, indexing
the same checkout. 40 files, **722 positions**, drawn by the same blind scanner.

`phpantom` is built from the upstream tag. The sibling checkout beside this repo is a fork pinned to
0.9.0, and measuring that would have reported a version nobody runs -- the two answer alike, but the
preset is `phpantom-0.10` for a reason.

| Go to definition                 |       |
| -------------------------------- | ----: |
| both agree                       |   594 |
| both answered, different target  |    34 |
| **they answer, phrost does not** | **2** |
| phrost answers, they do not      |    70 |
| neither answers                  |    22 |

| Hover                                 |       |
| ------------------------------------- | ----: |
| both describe it                      |   620 |
| **they describe it, phrost does not** | **2** |
| phrost describes it, they do not      |    79 |
| neither                               |    21 |

**The two we lose are one Drupal shape each**, and they are the same two in both tables:
`$entity->uuid` -- a field reached through the entity system's magic `__get`, which has no
declaration to find -- and `$this->select('…', '…')->fields`, the database API's fluent builder.
Both appear again among the four completion sites below where we offer nothing, so this is two
features rather than six rows.

**The thirty-four that disagree are two policies and no error.** Nine are the standard library,
where ours land in the vendored `mago-prelude` (`core.php`) and theirs in their own bundled set --
neither is wrong about which declaration it is, and this document records the same for intelephense.
The other twenty-five are variables, and the difference is which binding counts as the definition:
on `PhpStorageFactory::get`, where `$configuration` is initialised once and then reassigned in two
branches, we answer the initialisation and they answer a branch. Both are defensible and neither has
been argued for; it is recorded so that a future run does not read it as a regression.

**The seventy we win are one cause.** `$this->assertEquals`, `parent::setUp`, `$this->assertSame`,
`$this->drupalLogin`, `$this->getSession`, `$this->randomMachineName`, `$this->markTestSkipped` --
every one of them is a method inherited from a Drupal or PHPUnit test base class. The same gap
accounts for their diagnostics count and for most of what our completion offers that theirs does
not, so all three instruments are reporting one thing.

### Completion, and an instrument that was wrong about it

**207 positions, both non-empty at 169.**

The first run of this table read _968 distinct labels only phpantom offers_, and almost none of it
was real. The two servers spell a completion item differently -- `t` here,
`t($string, $args = ..., $options = ...)` there -- and the harness normalised only a trailing `()`
and a leading `$`. Probed directly at one `$this->` site, both servers returned **nine items, the
same nine**, and the harness scored seven of them as ours-missing. The signature comes off with the
parentheses now, PHP having no overloading, and the count is **201**.

What remains, ranked, is mostly one deliberate difference: `__sleep` 30, `__wakeup` 27, `__toString`
14, `__clone` 12, `__isset` / `__set` / `__get` 10 each, `__unset` 9. Offering a magic method on
`->` is not something anyone is reaching for, and this document has recorded that position for
intelephense as well.

Under that are the four sites where we offer nothing at all, two of them the shapes above:

| site                                                        | items only they offer |
| ----------------------------------------------------------- | --------------------: |
| `$this->select('…', '…')->fields`                           |                    62 |
| `$event->getResourceObject()->getField($field_name)->value` |                    54 |
| `$entity->uuid`                                             |                    45 |
| `$user->id`                                                 |                    19 |

Everything else is five items or fewer per site.

The other direction is the test base classes again: `assertIsNotCallable`, `assertStringEqualsFile`,
`assertDirectoryIsReadable`, `atLeastOnce` and fifty more, at 53 or 54 sites each.

### Document symbols

phrost **254**, phpantom **255**, on the same forty files. No decomposition needed.

### Diagnostics

phrost **113**, phpantom **415**, on the forty files this run opened -- and the ratio is a
resolution gap rather than a checking one.

**354 of their 415 are one message shape, `Method 'X' not found on class 'Y'`, and reading one file
settles it.** On `DriverSpecificSchemaTestBase.php`, 117 findings:

```
 18: Class 'Drupal\KernelTests\Core\Database\DriverSpecificKernelTestBase' not found
 20: Class 'Drupal\Tests\Core\Database\SchemaIntrospectionTestTrait' not found
115: Method 'fail' not found on class '…\DriverSpecificSchemaTestBase'
173: Method 'assertTrue' not found on class '…\DriverSpecificSchemaTestBase'
185: Method 'assertFalse' not found on class '…\DriverSpecificSchemaTestBase'
```

The parent is not found, so every method the class inherits reads as undefined. Two unresolved names
produce a hundred and fifteen findings, and the same gap is what our seventy definition wins and our
fifty-three-per-site completion wins are made of -- one cause, visible in three instruments.

**Why we resolve them is the framework seam doing its job.** `Drupal\KernelTests` is in neither
`autoload_psr4.php` nor the classmap: `core/tests/bootstrap.php` registers seven such namespaces at
_runtime_, and `providers/drupal/extensions.rs` contributes them as ordinary autoload rules, with no
Drupal name reaching the generic layer. A server reading only what composer dumped cannot see them.

Ours are type findings, by what they say rather than by count: 20 about reaching a property through
a value that may be null, 21 about calling a method on one, 16 about passing one where a class is
required, 15 from mago's linter about `==` where `===` is meant, and the rest in ones and twos.

**This row used to compare different things.** The harness counted every `publishDiagnostics` a
server sent, and phpantom checks the whole project rather than the open buffers -- so the first run
read _59 against 10 045_, which is a difference in scope wearing the costume of a capability gap. It
counts the opened files now and prints the wider total separately.

## Is every advertised capability real?

A different question from the one above, and a blunter one. `initialize` advertises nineteen
capabilities. Four of them -- definition, hover, completion, document symbols -- are measured
against another server in the section above. This section is about the other fifteen, and it asks
only: **driven over stdio against real code, does it answer, and is the answer worth having?**

The second half is the point. A technically-non-empty answer is the easiest thing in a language
server to ship by accident: a `selectionRange` that only ever returns the token under the cursor is
non-empty at every position and useless at all of them, and so is a `documentHighlight` that
highlights the one occurrence you are already looking at. So every row below is scored twice, and
what counts as _useful_ is written down per capability in `tools/capability-audit.py` rather than
left to the reader's charity.

```
python3 tools/capability-audit.py <project> --filter modules/custom --files 24 --limit 20
python3 tools/rename-audit.py <project>
```

**Method.** Twenty positions per capability, chosen by scanning the file _text_ with
`compare-lsp.py`'s own scanner -- never from phrost's index -- and spread across the sample rather
than taken from the first two files. Sample A is 23 files of the project's own code under
`docroot/modules/custom`. Sample B, used only where sample A did not hold enough of the right shape,
is 20 files matching `Interface.php`. Where a silence is the _correct_ answer the row says so and
does not count it against the server: a variable typed `string` has no type definition to go to, a
`final class` with no parent has no type hierarchy, and a tidy file has no quick fix.

| capability         | answers                                                  | useful | verdict                                       |
| ------------------ | -------------------------------------------------------- | ------ | --------------------------------------------- |
| codeAction         | 2/20 as written; **20/20** when the condition is created | 20/20  | **real**                                      |
| declaration        | 20/20                                                    | 20/20  | **real**                                      |
| documentFormatting | 20/20                                                    | 20/20  | **real**                                      |
| documentHighlight  | 20/20                                                    | 19/20  | **real**                                      |
| foldingRange       | 20/20                                                    | 20/20  | **real**                                      |
| implementation     | 7/7                                                      | 7/7    | **real**                                      |
| inlayHint          | 20/20                                                    | 20/20  | **real**                                      |
| references         | 20/20                                                    | 19/20  | **real**                                      |
| rename             | 13 of 13 shapes behave                                   | --     | **real** -- re-measured 2026-10-06, see below |
| selectionRange     | 20/20                                                    | 20/20  | **real**                                      |
| semanticTokens     | 20/20                                                    | 20/20  | **real**                                      |
| signatureHelp      | 20/20                                                    | 20/20  | **real**                                      |
| typeDefinition     | 20/20                                                    | 19/20  | **real**                                      |
| typeHierarchy      | 20/20                                                    | 20/20  | **real**                                      |
| workspaceSymbol    | 20/20                                                    | 20/20  | **real**                                      |

**Three rows moved, and two of them were one bug.** `documentHighlight` was 11/20 and
`typeDefinition` 17/20, both marked _thin_ and both annotated "dead at a parameter's declaration".
They were the same defect: a scope that began at the _body_ put the whole signature -- and so every
parameter's binding site -- outside four capabilities at once. `references` and `rename` were in it
too. Fixed, and the annotation is gone from the table rather than restated.

**The third row moved because the instrument was wrong, in our favour's opposite direction.**
`semanticTokens` read **0/20** on this run before the tool was fixed: its client declared
`tokenTypes: []`, and a server is entitled to send only the kinds a client says it can render --
phrost's legend is the _intersection_, which `answers/tokens.rs` states outright. So the audit asked
for nothing, got nothing, and reported a working capability as dead. A direct probe of one file
returns **659 tokens**. An instrument that understates is as wrong as one that flatters.

`callHierarchy` is advertised and answered now, with its own rows in the perf sweep --
`call hierarchy head`, `outgoing calls`, `incoming calls`. It is not in the table above because the
audit does not score it yet.

### What "useful" turned out to mean

Not one of these is a stub, and several are better than the line count suggested.

**documentFormatting is mago's formatter and behaves like one.** Every file in the sample was
mangled in a buffer -- indentation stripped, spaces removed around `=`, `,`, `;` and `.` -- and
formatted back. In **20 of 20** the result was byte-identical to the formatted original. Seventeen
of the twenty needed no edit as written, because the Drupal preset already matches how the project
is written. This is the capability the whole audit was commissioned to doubt, and it is the least
doubtful one in the list.

**selectionRange really widens.** Parent chains of five to nine distinct ranges, never one.

**semanticTokens classifies rather than colours.** 746 tokens on a 300-line form across thirteen
types -- `parameter` 79, `method` 63, `class` 41, `decorator` 32, `type` 31, `variable` 31,
`namespace` 19, `property` 13. (`tools/lsp-smoke.py` was reporting 466 of those as `?` because its
copy of the legend had gone stale by five entries; fixed.)

**implementation and typeHierarchy answer where the question exists.** `AnnotationInterface` returns
three implementers, `PluginAwareInterface` four, `OrderManagerTrait` forty-nine. Sample A held only
seven interfaces, traits and abstract classes, which is why sample B exists -- asking on a
`final class` and reporting the silence would measure the sample.

**codeAction fires, and the two-out-of-twenty is the codebase, not the server.** Only two positions
in sample A had a diagnostic at all, and at both a fix with a real edit was offered (`Add @return`,
`Add @param $path`). So the condition was created instead: a docblock deleted, a `@param` removed,
an unused import added, a class written without importing it. **Twenty out of twenty** offered the
right fix -- `Write a docblock`, `Add @param $internalNote`, `Remove unused import …`, and
`Import Drupal\node\NodeInterface` / `Import Drupal\Core\Entity\EntityTypeManagerInterface` /
`Import Drupal\app_core\OAuth\Exceptions\OAuthInvalidTokenException`, correctly ranked. The offers
are position-scoped: they appear on the _token_ and on the diagnostic's range, not anywhere on the
line, which is standard and worth knowing when probing.

**references is right where it looks wrong.** The one row under twenty answered 15 references across
6 files for `AttachmentType` while 24 files write that word -- but eighteen of those write it as a
_namespace segment_ (`…\AttachmentType\AbstractAttachmentType`), not as the class. The five files
that use the attribute, plus its declaration, are exactly the six it returned.

### One bug explains four rows

`documentHighlight`, `references`, `typeDefinition` and rename all go dead at the **same** position:
the cursor on a parameter's own name in a declaration. Everything else at that position works, which
is what makes it hard to notice.

```
docroot/modules/custom/app_core/src/Form/Checkout/CheckoutFormBase.php:66
    protected CheckoutManager $checkoutManager,
                              ^ cursor here
  hover             CheckoutManager $checkoutManager        ✓
  definition        CheckoutFormBase.php:66                 ✓
  prepareRename     allowed, placeholder "checkoutManager"  ✓
  typeDefinition    null                                    ✗
  documentHighlight []                                      ✗
  references        []                                      ✗
```

At a _use_ of the same kind of parameter the same four answer perfectly -- `$form_state` on line 227
of that file gives three highlights, three references, and a type definition landing in
`FormStateInterface.php`.

Nine of the eleven documentHighlight silences and all four typeDefinition silences in sample A are
this one position; in sample B, where every sampled variable is an interface parameter, it is 20
of 20.

**The cause, and the smallest honest fix.** `Scope::enclosing_declaration` in
`crates/body/src/scope.rs` finds the innermost function _body_ containing the offset, and says why
in its own doc-comment: "a cursor on a parameter's type hint is not inside the body." That is right
about the type hint and wrong about the parameter's own name, which is the local's binding site --
`local_occurrences` already returns that exact range as a `Write` when asked from a use. The fix is
to let that lookup also accept an offset inside a parameter's _name_ range, which the scope already
holds. One function, and it repairs four capabilities at once.

### Rename, in more depth

Rename gets its own tool because a wrong answer here destroys code rather than disappointing
somebody. `tools/rename-audit.py` discovers one example of each shape from the project itself, asks
`prepareRename` and `rename`, checks that every edit replaces exactly the name it claims to, and
then greps the project for occurrences of the old name the edit did _not_ touch -- split into code,
docblock and string, because those are three different severities.

Thirteen shapes, **all of which behave** -- re-measured on 2026-10-06 after the day's rename fixes.
The two rows that were "refused" are answered now, and the row that was declined is the feature
`MemberFamily` became.

| shape                                                            | prepareRename                             |          edits | outcome                                                                                                                              |
| ---------------------------------------------------------------- | ----------------------------------------- | -------------: | ------------------------------------------------------------------------------------------------------------------------------------ |
| a class used across files                                        | allowed                                   |   4 in 4 files | correct, nothing left in code                                                                                                        |
| an interface method with an implementation                       | allowed                                   |   5 in 5 files | correct: the declaration, its implementation and three calls -- the five `processBatch` "left in code" are unrelated batch callbacks |
| a parameter, cursor on its declaration                           | allowed                                   |  11 in 4 files | correct, nothing left in code                                                                                                        |
| the same, cursor on a use of it                                  | allowed                                   |  11 in 4 files | correct, and the same set                                                                                                            |
| a local assigned in two functions of one file                    | allowed                                   |    2 in 1 file | correct, does not cross the other function                                                                                           |
| a class path inside a string literal                             | allowed                                   | 42 in 19 files | correct: the last segment of every string naming it                                                                                  |
| a `$name` on a `@param` docblock line                            | allowed                                   |   3 in 2 files | correct: the tag moves with the parameter                                                                                            |
| a method a framework string names (`'::validateTestExtraction'`) | allowed                                   |    2 in 1 file | correct: the string moves with the declaration                                                                                       |
| a name reached through an alias                                  | allowed                                   |    5 in 1 file | correct: the alias is file-scoped                                                                                                    |
| a vendor class (`GuzzleHttp\…\PromiseInterface`)                 | refused: _not in this project's own code_ |              0 | correct                                                                                                                              |
| a Drupal core class (`Drupal\Core\…\EntityStorageException`)     | refused: same                             |              0 | correct                                                                                                                              |
| an invalid new name (`1nope`)                                    | allowed                                   |        refused | correct                                                                                                                              |
| a PHP reserved word as the new name                              | allowed                                   |        refused | correct                                                                                                                              |

The audit's "left in code" column greps the project for the old _name_, so for a common one it
counts other declarations that merely share it -- read the sites before reading the number. The
thirteenth shape is a framework's own-method string (`'#submit' => ['::saveNote']`), which a rename
of the method left behind until 4e153354; `where-goto-answers-the-rest-do` found it, and the audit
now asks it of every project it is pointed at.

Four of these were broken when this document was first written, and what each turned out to be is
worth keeping.

**A parameter renamed from its own declaration edited nothing.** `Scope::chain_at` resolved by body
range, so the whole signature -- every parameter's binding site -- fell outside every capability
that asks which scope owns an offset. Four requests were dead at that one offset and perfect one
word to the right. Fixed; the tell for the class is a feature that works at a _use_ and not at the
declaration of the same thing.

**Renaming through an alias renamed something else.** With the cursor on a use of `QuoteId`,
`prepareRename` offered `"QuoteId"` and `rename` replaced **`UUIDv7`** at thirty sites in six files.
Matching by resolution alone is right for _finding_ references and wrong for _editing_ them: the
`as QuoteId` bindings that made those names work were left untouched, which breaks exactly the sites
the binding was keeping correct. Renaming the alias is a third operation -- file-scoped, so the
audit's project-wide "left in code" count reads as a miss and is not one.

**A PHP reserved word was accepted as a new name**, because `is_valid_name` checks PHP's _lexical_
rule and `class` satisfies it. A reserved-word set decides it now, asked only in the class-like
position -- PHP has let a method be called `list` since 7.0, and a variable always could.

**A promoted constructor parameter was renamed as half of itself.**
`__construct(public int $across)` is one declaration PHP reads as two, and the two halves are found
by two different searches, so a rename took whichever the cursor was on: from the parameter, every
`$this->across` in the project kept the old name; from the property, the constructor body was left
reading a variable nothing binds. Neither half is a subset of the other. This checkout writes **1508
promoted properties in 431 files**, so it is how a modern constructor is written rather than a
corner. Fixed at the target: a promoted parameter answers as the property it declares, and the
property's sweep adds the constructor body's own occurrences in the declaring file.

**Renaming an interface method is declined**, and that is the one row still short of the feature.
`processBatch` on `AttachmentProcessorInterface` is declared by an implementation as well, and
renaming one declaration of an override family leaves the other answering the old name. Renaming the
whole family is the correct feature and a much larger one -- an interface's implementers include
classes under `vendor/`, where an edit is undone by the next `composer install`, and a class
implementing two interfaces satisfies the second by the same name. Until that is answered, declining
with a sentence is the honest half.

### What these requests cost

Sample A, median / p95 in milliseconds, as measured over stdio -- indicative, not a perf run.

| request                |     p50 |     p95 |
| ---------------------- | ------: | ------: |
| codeAction             |    0.18 |    0.61 |
| declaration            |    0.21 |    2.38 |
| documentHighlight      |    0.26 |    1.41 |
| foldingRange           |    0.14 |    0.39 |
| formatting             |    0.94 |    3.65 |
| implementation         |    1.58 |    4.23 |
| inlayHint              |    0.23 |    1.46 |
| prepareTypeHierarchy   |    0.10 |    0.33 |
| selectionRange         |    0.18 |    0.68 |
| semanticTokens/full    |    0.38 |    1.03 |
| signatureHelp          |    0.13 |    0.50 |
| typeDefinition         |    0.10 |    1.61 |
| typeHierarchy/subtypes |    1.53 |    6.02 |
| workspace/symbol       |    0.12 |    0.71 |
| **references**         | **529** | **686** |

Everything is sub-millisecond except references, which is three orders of magnitude slower than
anything else here because it reads every file of the project's own code off disk to grep it. It is
a command rather than a hot path, and it is deliberately not on one: document highlight asks the
same question restricted to the open buffer and costs 0.26 ms. Worth knowing before it is called
from anything that fires on cursor movement.

## What a session costs

`compare` reports memory twice, once at the end of indexing and once at the end of the run, and that
turns out not to be enough to say anything. A server that has finished indexing and one that is
still going look identical in two readings, and which of the two you are looking at is most of what
a memory comparison is about. `session-cost` runs named phases and samples memory and processor time
once a second _inside_ each of them:

```
just session-cost <project> --servers phrost,phpantom
```

| phase    | what it is                                                                    |
| -------- | ----------------------------------------------------------------------------- |
| `ready`  | start to the first correct answer about a class the project declares          |
| `settle` | idle, asked nothing -- what a server is still doing after it says it is ready |
| `open`   | forty files opened as an editor restoring its tabs would, and nothing else    |
| `work`   | one hover and one completion in each of them                                  |
| `rest`   | idle again                                                                    |

### phrost against phpantom 0.10.0

Both cold: phrost's stored index was moved aside first, and phpantom keeps none. Sequential, one
whole session each, on an idle machine.

The memory and processor rows come from `session-cost`; the per-request rows come from the `compare`
run above, which is a separate pass. They are not mixed anywhere that matters -- a request latency
is measured after both servers are indexed, so it does not care which start it followed -- but the
bootstrap figure from `compare` is _not_ reproduced here, because phrost read a cache in that run
and phpantom could not.

Cold, which is the only start phpantom has:

|                                  | phrost, before |   phrost, now |                             phpantom 0.10.0 |
| -------------------------------- | -------------: | ------------: | ------------------------------------------: |
| says it is ready                 |          4.8 s | **2.7-3.4 s** |                                       1.1 s |
| actually stops working           |          4.8 s | **2.7-3.4 s** |                                       2.1 s |
| processor seconds, whole session |            4.5 |           5.2 |                                        12.1 |
| RSS idle, after indexing         | **258-259 MB** |    306-313 MB | 352-384 MB _(ours at 8 workers)_ / 467.8 MB |

And warm, which is every start after the first and the one a person actually has:

|                    | phrost, before | phrost, now |
| ------------------ | -------------: | ----------: |
| says it is ready   |          2.7 s |   **2.2 s** |
| RSS idle           |     205-207 MB |  205-209 MB |
| RSS after 40 files |         222 MB |      222 MB |

Per-request latency, from the `compare` run over stdio, is unchanged by any of this: hover 0.1 / 1.0
ms against their 0.3 / 5.9, completion 0.5 / 3.5 against 0.5 / 3.9, definition 0.1 / 1.2 against 0.1
/ 3.1. So is what each server answers -- 594 definitions agree, the same two are theirs alone and
the same seventy ours.

**The comparison found one piece of work and it was not an algorithmic one.** phpantom was ready in
2.1 s against our 4.8, and the reason was cores: `phrost index` measured 4.12 s of wall clock for
2.42 user + 1.66 sys -- ninety-seven per cent of _one_ core of twelve -- while phpantom spent 12.1
processor-seconds in 2.1 s and so was holding six to eight.

Indexing is parallel now: on the same 30 965 files it goes **2.92 s to 1.44 s**, and a cold server
is ready in 2.7-3.4 s rather than 4.8. They are still quicker to ready, by about a second rather
than by half the time.

The phase split is what made it a small change rather than a rewrite, and both obvious guesses about
it were wrong. Reading is 1.019 s of a cold start and lowering 1.627 s, against 16 ms to register a
file and 255 ms to claim it -- so nine tenths of the work touches nothing shared. And a _warm_ start
is 921 ms of reading out of 1.08 s, because a file's stamp hashes its content: there is no cheaper
question than reading the file, and parallelism therefore helps the start a person sees every day
and not only their first.

### Where the rest of the memory was

`phrost index --memory` prints what the process holds beside what the index does, because the gap
was the number nobody could see: 88 MB of index in a 205 MB process, and measuring the second meant
remembering to run `ps` and subtract.

Asked where it went, `vmmap --summary` answered in one line -- **93.8 MB of empty-but-dirty
`MALLOC_LARGE`** on a server that had written a cache. Both halves of the cache were built in
memory: `encode` grew a `Vec` by doubling to 46 MB, and `load_cache` read the file whole to decode
from a slice. Neither needed to, since the format is written once in order and read once in order.
Streaming both:

|                         | before |      after |
| ----------------------- | -----: | ---------: |
| warm session, RSS       | 209 MB | **161 MB** |
| cold session, RSS       | 261 MB | **237 MB** |
| outside the index, warm | 118 MB |  **71 MB** |
| warm indexing           | 1.10 s |     1.11 s |

What is left is 68 MB of the same shape on the cold path, which is the parse arenas. Reusing one per
worker instead of one per file was tried and is **much** worse -- `LocalArena::reset` keeps its
chunks, so each worker's arena grows to the largest file it ever sees and holds it: cold RSS 237 ->
401 MB. Reverted.

**What parallel indexing costs is 12-15 MB a worker, on a cold start only.** Four workers rather
than the eight the machine offers, because that takes the larger half of the speed for the smaller
half of the memory. The cost is not the batching -- claiming each file as its worker finishes,
rather than collecting a batch, accounted for 8 MB of it -- but the allocator holding each thread's
high-water mark, most of which is the bump arena a parse needs. One per file, because the tree
borrows the arena and there is no reset. Removing that is what would make eight workers affordable,
and it is the next thing on this row.

### Two numbers here that should not be quoted precisely

**`open cost` for phrost is noise at this resolution.** Two runs of the same forty files measured
**-16.2 MB** and **+11.7 MB**. The allocator, not a property. The deterministic instrument is
`phrost index --memory`, which reports the index heap with a per-row breakdown -- 87.9 MB on this
checkout, _identical_ cold and warm. There is no equivalent for phpantom, so the RSS row is what
there is and it is quoted as an order of magnitude.

**`processor seconds` for phpantom varied 7.8 and 12.1** across two runs. `ps -o time=` has
one-second resolution, so that is a real spread rather than rounding, and the honest claim is "two
to three times ours" rather than a figure.

### phrost warm, for scale

The same session with a stored index to read: ready **3.3 s**, idle **204.3 MB**, ends at 218.4 MB,
**2.7** processor-seconds. Warm holds 59 MB less than cold while the index heap is byte-identical,
so the difference is transient allocation the allocator has not returned -- the cold path reads and
parses 30 965 files, the warm one decodes a single 46 MB blob. Not compared against phpantom above,
because phpantom has no cache and a warm start against a server that cannot have one measures the
conditions rather than the servers. This document has made that mistake once already.

### phrost against intelephense

An older run, kept because the shape is different rather than the numbers larger, and **its
bootstrap row is not a like-for-like** -- see below.

|                          | phrost           | intelephense                    |
| ------------------------ | ---------------- | ------------------------------- |
| Time to answering        | 2.9 s _(cold)_   | 3.2 s _(warm)_ — not comparable |
| RSS once ready           | **202.9 MB**     | 1 267.6 MB                      |
| RSS after 40 files       | **218.6 MB**     | 1 722.0 MB                      |
| hover p50 / p95          | **0.1 / 0.9 ms** | 0.6 / 8.3 ms                    |
| definition p50 / p95     | **0.1** / 1.1 ms | 0.1 / **0.7 ms**                |
| completion p50 / p95     | **0.5 / 3.0 ms** | 1.1 / 10.3 ms                   |
| documentSymbol p50 / p95 | **0.1 / 0.1 ms** | 0.2 / 0.8 ms                    |

Memory is the decisive one: **an eighth** of theirs once ready, and the gap widens rather than
closes as files open -- theirs grows by 454 MB over forty files where ours grows by 16. Their
definition p95 is better than ours, by a margin that is not a rounding error and that nobody has
chased.

**The bootstrap row.** The harness hands intelephense an explicit persistent `storagePath` with
`clearCache: false`, so it starts warm on every run after the first. phrost's own cache is stamped
with the build that wrote it -- deliberately, because a change to the code that _derives_ the index
invalidates it as surely as a format change -- and this measurement was taken minutes after
rebuilding the binary. So phrost indexed from nothing while intelephense read a cache, and the
difference is the two conditions rather than the two servers, in whichever direction it lands. Run
it through `session-cost` with both caches cleared before quoting it again.
