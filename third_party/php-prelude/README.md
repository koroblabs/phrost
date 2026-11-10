# PHP prelude stubs

Declaration-only PHP describing the standard library and bundled extensions: `strlen`,
`\Exception`, `ArrayAccess`, `DateTimeInterface`, and everything else that exists without being
declared in any project file. Without these, every reference to the standard library is
unresolvable — and the index has no fallback that would hide it.

## Provenance

Vendored from [mago](https://github.com/carthage-software/mago) `mago-prelude` 1.58.2
(`assets/extensions/*.php` plus `assets/mago.php`), licensed MIT OR Apache-2.0 — the same terms as
this project.

**This version and the `mago-*` crate pin in `Cargo.toml` are independent and will drift apart.**
Bumping the crates is a build-graph change with a test suite behind it; bumping this directory
rewrites what the index knows about the standard library, so it is its own piece of work with its
own measurement. Neither implies the other, and a reader who finds the two numbers different has
found the normal state.

`third_party/php-prelude-upstream` holds the same version exactly as published, never edited, so
that what this project changed is an artifact rather than a memory:

```
just stubs-diff     the whole local delta -- 9 865 lines across 31 files
just stubs-accept   re-record it after a deliberate edit
```

Which is what makes hand-maintaining this set survivable, and it was added because the alternative
had already failed quietly: of the seven categories of edit below, **four** had nothing that would
fail if a re-vendor reverted them. `a_re_vendor_that_reverts_a_local_edit_fails_here` now covers all
of them at once, and reading it settled something the list could not settle about itself -- every
file that *removes* an upstream line accounts for itself exactly against the prose here, four
numbers agreeing with four sentences written months apart.

The set is not forked and should not be, because upstream fixes the same defects we do. What we
carry is a patch, and re-vendoring is re-applying it -- see that directory's README for the order
that works. The 1.48.0 to 1.50.0 step was the first taken this way and it is the worked example: 17
lines across two files, three declarations, of which **two cost nothing and one cost 34 findings**,
and knowing which was which took one `--ratchet` run per arm rather than an argument.

1.50.0 to 1.52.0 was larger and almost all gain: 208 lines across 20 files, nearly every one a
parameter that PHP lets a caller leave out and the set had written as required (`getdate()`,
`xml_parse_into_struct()` with three arguments, `ReflectionClass::getStaticPropertyValue($name)`),
the two `stream_context_set_option` overloads folded into one signature, and the SPL methods
`RecursiveDirectoryIterator` and `CachingIterator` were missing. Plus two extensions: `rdkafka.php`,
taken, and `swoole.php`, left out -- see Local edits. Eight conflicts, all of the first shape below.
Measured over the findings rather than the counts: WordPress -6, Laravel -2, Symfony -54 (40 of
them the RdKafka classes its VarDumper caster names), the reference project and psl unchanged, and
**not one new finding anywhere**. It also
rewrote `Ds\Map::get` to `@template TDefault = never`, a shape phrost read as nullable until the same
step taught it to read a template's default.

1.52.0 to 1.56.3 was three lines in one file, and a worked example of a step that changes nothing:
`get_class()` and `get_called_class()` gained `@return class-string`, `get_parent_class()`
`@return class-string|false`. Merged clean beside our prose. Findings and coverage dumps are
byte-identical on every corpus and the reference project, because a docblock naming a narrower
scalar beside a native one (`class-string` beside `: string`) is not a refinement phrost reads --
`get_class($row)` was already `class-string<Row>` by a rule of its own, and the object-typed call
stays `string`. Taken so the patch stays a patch against the version upstream is on.

1.56.3 to 1.58.2 took two of this set's own edits upstream and one this set did not have.
`base64_decode()`'s conditional return and the `class_parents()`/`class_uses()`/`class_implements()`
maps keyed by name are the set's own now -- the local tags were replaced by upstream's spelling and
left the tripwire table, so the delta is smaller by those lines -- and `class_implements()` gained a
branch: an object never answers `false`. Upstream also dropped `@param object|string` from
`is_a()` and `is_subclass_of()`, whose native parameter is `mixed` and which PHP answers `false`
for anything else; read with the narrower tag, a nullable value handed to either was reported 17
times on Symfony and Laravel, none of them a bug. And `SoapVar`'s `$typeName` defaults to null.
Findings elsewhere unchanged; the one new shape it exposed -- an unsettled conditional whose
branches share an operand read `A|A|false` -- was phrost's to fix, not the set's.

Chosen over phpstorm-stubs for two reasons: it is an eighth of the size (1.2 MB against 9.7 MB, one
file per extension rather than 546), and its types are precise where php.net's prose stubs are not
(`@return int<0, max>` for `strlen`, `@pure` where it applies). What it does not carry is the prose
description of each function, which is what a hover shows below the signature — see
[Documentation](#documentation) for what was done about that, and why phpstorm-stubs turned out not
to be the answer.

`mago.php` is required, not optional: the extension files carry `#[Mago\AvailableSince]` attributes
and the class has to exist for them to resolve.

## Documentation

The set carries types and no prose. A hover over `sort` showed a signature and nothing else, and
**14** of its 4 422 methods had a description. **945 functions and 1 199 methods** now carry a summary
and **138** carry a worked example, written by hand in three passes.

The first took the 193 names one real checkout's own code actually calls — **100 % of its calls into
this set**, with the top 100 covering 95.3 % of the call sites. The second took every remaining
function, and the third every method, in the extensions **PHP itself bundles**, which is the
principled boundary: every installation has them, so every project's vendor tree reaches them even
where its own code does not. `uv` (140 functions), `gearman` (94), `stats` (68), `svn` (58), `com`
(32) and `xcache` (27) are deliberately left out — not for being old, but because nobody hovers
`xcache_get`.

The method pass has its own highest-value fact, and it is not the one the function pass had:
**whether a method mutates its receiver or returns a new instance**. `DateTime::modify()` changes the
object in place and `DateTimeImmutable::modify()` does not, which is the commonest bug in the
language, and neither signature says so. After that come what `phar.readonly` blocks, what
`ZipArchive` defers until `close()`, where an `XMLReader` cursor ends up, which of `ReflectionClass`'s
near-synonyms includes inherited members, and — for an interface method — when PHP itself calls it.

A summary is one sentence saying what the function does, plus — only where it earns the line — the
facts a person actually opens a hover for. In rough order of value:

- **If it returns an array whose keys matter, name the keys.** The highest-value case by a distance,
  and 82 functions in the bundled set are in it. `@return array<int|string, int>` on `localtime` says
  nothing; `tm_sec`, `tm_min`, `tm_hour`, `tm_mday`, `tm_mon`, `tm_year`, `tm_wday`, `tm_yday`,
  `tm_isdst` — plus the two traps, that `tm_mon` is 0-based and `tm_year` counts from 1900 — says
  everything. `stat`, `getimagesize`, `date_parse`, `debug_backtrace`, `gc_status` and
  `timezone_transitions_get` are the same shape.
- **What each value of an int or bool flag does**, for the 36 that take a `$flags`, `$mode`,
  `$options` or `$type`. `count_chars`'s five modes and `parse_ini_file`'s three scanner modes are
  not derivable from anything.
- What it returns on failure; whether it mutates in place; whether a comparison is loose; whether it
  counts bytes or characters; whether the argument order is the one people get wrong.

An example goes in a `@code` block, which phrost renders as a fenced code block, and only where prose
cannot carry it — `hrtime()`'s two return shapes, `symlink()`'s argument order, `gzopen()`'s `'wb9'`
mode string, `chmod()`'s leading zero.

Two mistakes are worth recording because a later pass will make them again. The placing script read
*any* non-tag line in a docblock as an existing description, and a multi-line conditional type
continues on lines opening with `?` and `:` — so nine functions were silently skipped, `array_map`
(178 call sites) and `array_splice` among them, and the skip read as safety. And two written examples
carried `/* ... */`: PHP comments do not nest, so the `*/` closed the docblock early and turned the
rest of it into code — **24 syntax errors** in a corpus that had none, with `phrost check
third_party/php-prelude` as the only thing that noticed. The script now refuses a comment delimiter
outright.

### Why not phpstorm-stubs

Because its prose is php.net's *short description*, which is a title rather than documentation.
Measured over both sets, top-level functions only:

| | phpstorm-stubs | here |
| --- | ---: | ---: |
| functions with prose | 4 699 | 945 |
| methods with prose | 7 | 1 199 |
| worked examples | 0 | 138 |
| median prose length | 51 chars | ~200 chars |
| says anything about the return | 18.6 % | most of them |
| mentions `false` or `null` | 3.8 % | most of them |
| carries raw HTML in the prose | 954 | 0 |
| opens with a `(PHP 5 >= 5.2.0)` banner | 822 | 0 |

`sort` there is "Sort an array"; `strpos` is "Find the position of the first occurrence of a substring
in a string". The facts a hover is opened for — `strpos` returning `false` so the result needs `===`,
`reset` moving the internal pointer, `in_array` comparing loosely — are on php.net's page body and
are not in the stub. So importing it would trade 195 useful summaries for 4 699 titles, and cost the
precise types on the way.

Its breadth is real for a name that does not explain itself, and that is the case for keeping it in
mind as a **floor** for the extensions left out above rather than as a source. It is not the case for
anything a PHP installation ships: those are all covered now.

`crates/stubs`'s `a_re_vendor_that_drops_the_written_summaries_fails_here` is the tripwire, because a
re-vendor takes every summary at once and a docblock that is simply gone fails nothing.

## Local edits

The **why**. What each edit *is* is `just stubs-diff`, which cannot go stale; this list is the part a
diff cannot carry — what was wrong, what it cost, and whether it is worth reporting upstream. A
docblock this project's lowering cannot read is inert without failing anything, which looks exactly
like a stub that does not work, and only a sentence can say which.

- **Nine methods the set declared twice in one class are one declaration each** (`imagick.php`,
  `intl.php`, `parle.php`). An extension can overload a method, or take a different signature
  depending on the library it was built against, and PHP cannot write either -- the Imagick
  extension's own stub puts `convolveImage(ImagickKernel $kernel, ...)` and
  `convolveImage(array $kernel, ...)` either side of `#if MagickLibVersion >= 0x700`. Upstream
  writes both out, which PHP refuses, and the index keeps the *last*: so the kernel object, which
  is what a build against ImageMagick 7 takes and what throws a `TypeError` for an array there, was
  reported as the wrong type on a real project. This used to be a row in the table of defects left
  as vendored, on the ground that none of them costs anything in use. Probed, all nine do: ten false
  findings, among them `IntlCalendar::set(2026, 10, 10)`, which is the standard library, and the
  three-coordinate form of `setImageBluePrimary` and its three siblings.

  Each is folded into the one signature that takes whatever either took, which is the weaker claim
  and the only one a stub can make about a build it cannot see: `ImagickKernel|array $kernel`, a
  third coordinate that may be left out, `Imagick::count(int $mode = 0)`, `IntlCalendar::set` with
  its four trailing parameters optional, and `Parle\RLexer::push` over its three forms. What that
  gives up is reporting the form the installed build refuses, which nothing here could have told
  apart. `no_class_in_the_set_declares_one_method_twice` counts them back to zero, and its
  neighbour lists what the published set still declares twice, so a fold upstream takes shows up
  as one this set can stop carrying. **Worth reporting upstream.**

  `setrawcookie()` is the same defect at the top level and was found by asking the same question of
  functions (`standard.php`). Its two forms -- seven positional parameters, or a name, a value and
  an array of options -- were two declarations, the first is the one a call is read against, and so
  `setrawcookie($name, $value, ['expires' => $at])` was an array handed to an `int`. It is one
  declaration with `array|int` in the third position, which is how the set already writes
  `setcookie()`. Five more functions are declared twice and are left: `pg_socket_poll`,
  `pg_result_memory_size` and `pg_change_password` are the same signature repeated, and for `pg_jit`
  and `ldap_exop_sync` the first copy is the one PHP documents and the one that is read. They cost
  nothing in use, and `the_functions_the_set_still_declares_twice_are_these` lists them so that a
  sixth, or `setrawcookie` coming back, is seen.

  What none of this is: a choice the set made and the index ignored. Nothing on either copy of any
  of these says when it applies -- no attribute, no comment -- and what decides it for Imagick is the
  version of a *library*, which the set's own version attributes (`Mago\AvailableSince` and its nine
  siblings, each taking a PHP version) could not say if they were there. Those attributes are a
  separate matter and a real gap: the set writes `AvailableSince` 298 times and `AvailableUntil`
  once, and nothing here reads either, so a function PHP 8.4 added is known to a project that
  requires 8.1.

- `DateInterval`'s eleven properties lost their `readonly` modifier (`date.php`). PHP declares none
  of them readonly: `$interval->invert = 1;` is how a negative interval is built by hand, and
  Symfony writes it in eight files of its own -- a transformer, two normalizers and their tests.
  Reading the stub as written made the `readonly-write` check report every one of them on code that
  runs. **Worth reporting upstream.**

- The 2 144 prose summaries and 138 examples above, across 19 files. See
  [Documentation](#documentation) for what they say and for the mistakes the placing scripts made.
  Two more, from the method pass, are worth the same record. Several stub files wrap everything in a
  braced `namespace { ... }` block, which indents every class inside it — anchoring the class scanner
  at column zero found no class in `dom.php` at all and left 26 `DOMElement` methods unplaced. And a
  method has to be keyed `Class::method`, not by its name: `count`, `current`, `next` and `offsetGet`
  are declared by dozens of classes, and keying by owner is what caught six invented methods
  (`SplDoublyLinkedList::enqueue`, `IteratorAggregate::current`, `PDORow::openBlob` among them) that a
  bare-name match would have attached to whichever class happened to declare something of that name.

- **`RedisCluster`'s four `FAILOVER_*` constants**, which the addition below left out. php-redis
  declares `FAILOVER_NONE`, `FAILOVER_ERROR`, `FAILOVER_DISTRIBUTE` and
  `FAILOVER_DISTRIBUTE_SLAVES`, and Symfony's `RedisTrait` writes all four in one `match` --
  8 `undefined-constant` findings against a class this project added itself.

- **Forty declarations were written with two docblocks in a row**, and the first one's tags are lost
  -- PHP attaches the nearer block and so does every other tool. `DOMNode::appendChild()` had
  `/** @return DOMNode|false */` above its prose block and no native return type either, so it was
  completely untyped, and with it every chain through it. Thirty-three of the forty are `dom.php`,
  three each `date.php` and `xmlreader.php`, one `spl.php`. The tags are merged into the block that
  keeps them; `no_declaration_carries_two_docblocks` counts them back to zero, so a re-vendor that
  reintroduces the generator's artifact fails rather than silently dropping the tags again.
  **Worth reporting upstream.**

- **`base64_decode()` carries a conditional return type** (`standard.php`). It is declared
  `string|false`, and the `false` is reachable only with `$strict` set -- left out, which is how
  nearly every call writes it, invalid characters are silently discarded and a string always comes
  back. The set's own prose says so and its type does not, so `$this->takes(base64_decode($raw))`
  was reported on code that cannot fail: 84 findings across four corpora, and the one real
  `base64_decode($s, TRUE)` still reports. Written as
  `@return ($strict is true ? string|false : string)`, which the docblock language says exactly.
  `the_added_type_tags_survive` is the tripwire. **Worth reporting upstream.**

- **Four DOM methods return the node they were given**, and the set declares `DOMNode|false` --
  which is true and useless: `$parent->appendChild($doc->createElement('p'))->setAttribute(...)` then
  reads as a `setAttribute` on a bare `DOMNode`, reported 29 times on Symfony. `appendChild`,
  `insertBefore`, `removeChild` and `replaceChild` now carry `@template TNode of DOMNode` with
  `@param TNode` and `@return TNode|false`, which is what the manual says each of them hands back and
  what PHPStan's own stubs declare. **Worth reporting upstream.**

- **`DOMNode::$childNodes` carried no element type** (`dom.php`). `DOMNodeList` is declared
  `@template-covariant TNode of DOMNode|DOMNameSpaceNode`, so an unparameterised one falls back to
  the bound and `foreach ($element->childNodes as $child)` types `$child` as
  `DOMNode|DOMNameSpaceNode` -- an `incompatible-argument` on every `$doc->saveHTML($child)` beside
  it, and PHPStan reporting nothing at the same site is what found it.

  A namespace node cannot be in a tree at all, which PHP settles rather than the specification:
  `$ns instanceof DOMNode` is `false`, `class_parents('DOMNameSpaceNode')` is empty, and both
  `appendChild()` and `importNode()` throw `TypeError: must be of type DOMNode, DOMNameSpaceNode
  given`. The only thing that produces one is `DOMXPath::query()` over a namespace axis. Written
  `@var DOMNodeList<DOMNode>`, which is what phpstorm-stubs declares (`dom_c.stub`) and what this
  set's own modern twin `Dom\Node::$childNodes` already carried -- so it is an omission on the
  legacy half rather than a position. `the_added_type_tags_survive` is the tripwire.
  **Worth reporting upstream.**

  The bound itself is right, and the set hides the one place it applies: phpstorm-stubs declares
  `DOMXPath::query()` as `DOMNodeList<DOMNode|DOMNameSpaceNode>|false` where this set declares
  `mixed`. So the union was arriving only where it cannot happen and nowhere that it can, which is
  why narrowing the property loses nothing.

- **`ReflectionClass::newInstanceArgs()` returns `T`, not `T|null`** (`reflection.php`), native
  `object` rather than `?object`. Run against 8.5.10: an instantiable class gives the instance, an
  abstract one throws `Error`, a private constructor throws `ReflectionException` -- there is no
  path that returns `null`, and the nullable hint is left over from a PHP 7 signature. With the
  class's template bound by its constructor, the `?` made every `$r->newInstanceArgs($a)->m()` a
  possibly-null receiver (Symfony's routing tests, three). What PHPStan's stubs declare.

- **`DOMXPath::query()` carries `@return DOMNodeList<DOMNode>|false`** (`dom.php`), where the set
  declares only the native `mixed`. Run against 8.5.10: `//p` hands back a `DOMNodeList` of
  `DOMElement`, `//@id` of `DOMAttr`, `//text()` of `DOMText`, and a malformed expression `false` --
  never anything else, and `evaluate()` is the method that can return a scalar. Left `mixed`,
  `iterator_to_array($xpath->query(...))` typed nothing and every node read off it was an untyped
  receiver. `DOMNode` rather than the list's bound `DOMNode|DOMNameSpaceNode`, although a namespace
  axis does produce one: with the union, every `removeChild($node)` over an element query reported
  an `incompatible-argument` for a node that query cannot return. What PHPStan's stubs declare,
  which is what the reference project's CI judges it by; phpstorm-stubs keeps the union.
  **Worth reporting upstream.**

- **`preg_replace` keeps 1.48.0's unkeyed `@param`/`@return`** (`pcre.php`), which is the one place
  this set is deliberately *behind* upstream rather than ahead of it. 1.50.0 added
  `@template K of array-key` so that `preg_replace($p, $r, $subject)` hands back an array keyed the
  way `$subject` was -- a real improvement, and one phrost cannot read: a free function whose return
  names its own template answers **nothing at all**, so the declaration's native `string|array|null`
  stands and every `trim(preg_replace(...) ?? '')` in sight is reported. Measured: WordPress **+13**,
  Symfony **+9**, Laravel **+1**, the reference project **+10**, all of the shape
  `$string takes string, and this is array|string`.

  The `@param-out non-negative-int $count` from the same hunk **is** taken, and the two other
  declarations 1.50.0 changed -- `array_unshift` narrowing its variadic to the array's own value
  type, `array_splice` defaulting two templates to `never` -- moved no count on any of the five
  corpora. So this is one hunk of three, declined with a number beside it. Take it the moment the
  key half of `ReturnBranch::ArgumentElement` exists; `docs/08-types.md` holds the four-arm probe
  that names the gap.

- **`preg_match_all()`'s `$matches` names no keys** (`pcre.php`), where the set has
  `list<array<string>>`. That is the shape of `PREG_SET_ORDER` only: the default
  `PREG_PATTERN_ORDER` keys the array by group number *and* by every named group, so
  `$matches['uri']` after `(?P<uri>...)` read as a string key in an int-keyed list and was reported
  as one no run can hold -- in Symfony's `DomCrawler` and in WordPress's SimplePie. Unkeyed covers
  both orders and claims nothing either one breaks.
- **Three of `openssl.php`'s `$certificate` parameters take `OpenSSLCertificate|false`**, where
  PHP 8.5's own reflection says `OpenSSLCertificate|string` -- `openssl_x509_read()`,
  `openssl_x509_fingerprint()` and `openssl_pkcs7_sign()`, beside the dozen siblings the set
  already spells right. A PEM string is how all three are called, and once phrost learned that
  weak mode converts nothing to a `false`, every such call in Symfony's mailer and WordPress's
  PHPMailer read as handing a string to a parameter no string reaches.
- **`swoole.php` is left out**, which 1.52.0 added -- the one file upstream ships that this set
  does not carry. It declares global `go()` and `defer()`, short-name aliases that exist only while
  `swoole.use_shortname` is on, and `defer()` is also Laravel's own global helper, declared behind
  `function_exists('defer')`. A stub outranks a guarded project function -- right for a polyfill of a
  *bundled* function, wrong for an extension the project does not load -- so every `defer()` in
  Laravel's tests resolved to Swoole's: **41 findings, all false**. Taking the two aliases out fixed
  that and left the question the README already answers for `ext-relay`: the extension appears in no
  corpus except as a word in a string, so its signatures would be paid for by nobody. Add it back the
  day a corpus uses it, without the aliases.

- **49 `@throws` tags, across 10 files**, plus one on `DateTime::__construct`. The set describes
  what a function throws in *prose* and almost never declares it: 428 tags against 8 258 functions,
  where phpstorm-stubs carries 2 244. Where a summary already says "Throws X if ..." and no tag says
  so, the tag was added -- sourced from the set's own text rather than invented, and machine-readable
  where the sentence was not. `phar.php` accounts for 20 of them and `spl.php` for 15.

  `DateTime::__construct` is the single addition the prose did not source. It is an asymmetry rather
  than a judgement: `DateTimeImmutable::__construct` two hundred lines below documents the same
  `DateMalformedStringException` for the same argument, and both constructors throw it.

  **Worth reporting upstream**, with the `@code` trap beside it: a first pass read
  `throw new ErrorException(...)` out of the *example* in `ErrorException::__construct`'s own
  docblock and declared the constructor as throwing itself. A docblock is exactly where somebody
  demonstrates a tag, which is what `Lowerer::doc_is_ignored` already records for `@ignore`.

- `redis.php`, `RedisCluster`: **added**, which is the one edit here that supplies a declaration
  rather than correcting one. The file declares `Redis`, `RedisException`, `RedisArray` and
  `RedisSentinel` and not the cluster client, and `RedisCluster` is named in type hints by anything
  that supports redis in cluster mode -- 257 `undefined-type` on Symfony's own source and 15 on
  Laravel's, from one class. The 267 signatures are phpredis's own
  [`redis_cluster.stub.php`](https://github.com/phpredis/phpredis/blob/develop/redis_cluster.stub.php),
  which is the authoritative source and is PHP License 3.01; its prose docblocks are left there
  rather than re-licensed into this file, and its declarations are stub-style (`;` for a body) and
  are written here the way every other class in this set is (`{}`).

  **Its constants are not here, and that is deliberate.** phpredis writes every one of them
  `= UNKNOWN`, a code-generator placeholder, so the values live in C source this could not
  retrieve. `OPT_SLAVE_FAILOVER` is the exception: the stub's `@cvalue` names the same C constant
  this file already states for `Redis`, so it is written with that value. The `FAILOVER_*` family
  is omitted and costs **8** `undefined-constant` across the two corpora, against the 272 the class
  removes -- a missing answer where the alternative was an invented one, which is the rule the rest
  of this section follows. Worth reporting upstream, where the values are known.

- `standard.php`, `str_replace`, `str_ireplace` and `getopt`: added `@param-out int` for the
  by-reference out-parameter (`$count`, `$rest_index`). All three declare it `?int` and all three
  always write an int when the argument is passed, so the signature is weaker than the truth and a
  caller was told the result may be null. Three of the ten untagged weak-typed by-reference
  parameters in the set; the other seven are genuinely unknowable -- `settype` depends on a string
  argument, `array_walk_recursive` and `mb_convert_variables` do not change the type at all, and
  `mysqli_poll`'s arrays are in *and* out.

  Read from every position a call can be written in, which they were not when they were added: the
  by-reference reader saw only a call written as a statement of its own, so `$n = str_replace(...,
  $count)` was not one of them. `phrost_body::passed_to_any_call` is what reads them now.

- `reflection.php`, `ReflectionEnum::getCase`: 1.48.0 respelled its return as `static`, which
  inside `ReflectionEnum` means `ReflectionEnum`. It returns a `ReflectionEnumUnitCase` --
  `ReflectionMethod` says so and `get_class()` on a real call agrees -- so the vendored spelling
  would have sent every `getCase(..)->` chain to the wrong class. Left as an edit rather than in
  the table below because a wrong answer costs more than a missing one, which is the same reason
  the two edits below are edits.

- **`json_encode()` carries a conditional return type** (`json.php`). It is declared `string|false`
  and never returns `false` when `JSON_THROW_ON_ERROR` is in the flags -- it throws a
  `JsonException` instead, which was run against 8.5.10 along with the one combination that keeps a
  string alive beside it (`THROW|PARTIAL_OUTPUT` returns `'[null]'`). Written as
  `@return ($flags is 4194304 ? string : string|false)`, the number being the flag's value, because
  a conditional tests what the call wrote and a call writes the constant's *name*. 18 findings
  across five projects, every one a `md5(json_encode($x, JSON_THROW_ON_ERROR))` or its shape.
  **Worth reporting upstream.**

  No `@throws` beside it, deliberately: the tag has no conditional form, so declaring it would
  report every plain `json_encode($value)` as throwing -- measured at 16 on the reference project,
  and four of the four sampled were calls with no flags at all. A missing answer where the
  alternative is a wrong one, which is the rule the rest of this section follows.

- `standard.php`, `str_replace` and `str_ireplace`: the `@return` conditional's else branch was a
  second conditional. `phrost-lower` has no type to lower out of a branch that is itself a test, so
  it drops the whole contract and both functions answered with their bare `array|string` signature.
  Flattened to `($subject is string ? string : array<string>)`, which says the same thing about the
  two cases a caller can be in.

- `core.php` and `standard.php`: **33 constants the engine fills in carry a `@var`**, the type
  PHP 8.5.10 reports for each (`get_debug_type(constant(...))`). The set writes them as
  `const PHP_EOL = PHP_EOL;` -- a value that names the constant itself and so states nothing -- and
  a global constant is typed from its value, so `PHP_EOL`, `PHP_VERSION_ID`, `DIRECTORY_SEPARATOR`
  and `STDIN` were the untyped ones among the constants people actually read. The three that PHP
  on this machine does not define (`SUHOSIN_PATCH_VERSION`, `RADIUS_OPTION_*`) are left alone.

- `spl.php`: **`class_parents`, `class_uses` and `class_implements` return a map, not a list.**
  The set declares `list<class-string>|false` and its own prose for `class_uses` says "keyed and
  valued by their fully-qualified names", which is what 8.5.10 returns for all three
  (`['T' => 'T']`). Inert while a docblock could not refine the native `array|false` beside it, and
  the first thing to speak when it could: Laravel's `trait_uses_recursive` builds its result with
  `$traits += ...` and declares `array<string, string>`, and the list read as a wrong return. Now
  `array<class-string, class-string>|false` and its two siblings; `the_added_type_tags_survive`
  holds all three. **Worth reporting upstream.**

- **What a string function's result is known to hold**, now that `non-empty-string` and
  `non-falsy-string` are types rather than spellings of `string`. A declaration saying `string`
  where the engine always hands back something is a report on every call that passes the result to
  a parameter declaring one, so each of these says what PHP 8.5.10 returns, matching PHPStan's own
  function map:
  - `md5`, `sha1`, `hash`, `hash_hmac` are `non-falsy-string`, and their `_file` forms that or
    `false` -- a digest is never empty and never the one character `'0'`, raw bytes included;
  - `uniqid` and `serialize` were `non-empty-string` upstream and are `non-falsy-string`: thirteen
    hex digits at the least, and a serialization at least `N;`;
  - `spl_object_hash` is `non-falsy-string`;
  - `realpath`, `getcwd` and `tempnam` are `non-empty-string|false`, a path the engine resolved;
  - `json_encode` is `non-empty-string` in both branches of its conditional -- `json_encode(0)` is
    `'0'`, so not non-falsy;
  - `date` and `gmdate` are `($format is non-empty-string ? non-empty-string : string)`;
  - `PHP_VERSION`, `PHP_OS`, `PHP_OS_FAMILY`, `PHP_SAPI`, `PHP_SHLIB_SUFFIX`, `PHP_EOL`,
    `DIRECTORY_SEPARATOR` and `PATH_SEPARATOR` carry `@var non-falsy-string` where the entry above
    wrote `string`. The directories and `PHP_BINARY` stay `string`: a build may leave any of them
    empty.

  Measured as the cost of leaving them: on PHPStan's own probe of the commonest string functions,
  every one of these was a call phrost reported and PHPStan did not. `strval`, `str_repeat`,
  `random_bytes` and `str_pad` hold something only for a positive int, which no conditional here
  can test, and are left as they were.

## Known defects, left as vendored

`phrost check third_party/php-prelude` reports 54 beside the two lint categories, and every one of them is a defect in the vendored
set rather than in the checker -- which is why they are recorded here instead of edited away: a local
edit is silently reverted by a re-vendor, and none of these costs anything in use. The stubs are
indexed as `SourceOrigin::Stub`, so a project's own `check` never reports them.

| | count | what it is |
| --- | ---: | --- |
| `undefined-type` | 28 | `#[TentativeType]` (23) and `#[ArrayShape]` (1) written bare with the declaring class shipped by nobody, and `#[Mago\AvailableSince]` written *relative* inside `namespace Random\Engine` (4), where PHP resolves it to `\Random\Engine\Mago\AvailableSince` too |
| `unimplemented-method` | 8 | `ds.php` is incomplete: `Ds\Deque` and its neighbours declare none of `Ds\Sequence`'s `apply`, `push`, `insert`, `set`, `sort`, `unshift`, nor `ArrayAccess`'s four, nor `jsonSerialize`. `RecursiveDirectoryIterator` without `getChildren`/`hasChildren` was the ninth, and 1.52.0 declared both |
| `unused-import` | 8 | `use RuntimeException;` beside code that writes `\RuntimeException` |
| `bad-override` | 1 | `phar.php` redeclares `_bad_state_ex()`, which `SplFileInfo` declares `final` |
| `refused-declaration` | 2 | `random.php` gives `enum IntervalBoundary` a `public string $name`, which PHP refuses: an enum holds cases, constants and methods, and `$name` is implicit. And `exec()` declares `int|int &$result_code`, which is `Duplicate type int is redundant` -- a generator artifact, and **worth reporting upstream** |
| `unknown-named-parameter` | 5 | `#[Deprecated(reason: ...)]`, once in `intl.php` and four times in `reflection.php`: PHP's attribute takes `message` and `since`, and `reason` is the name JetBrains' own attribute gives it |
| `doc-undefined-type` | 2 | `@return v` in `spl.php` and `@param Job $job` in `zend.php`, neither of which names anything |

### What the set does not declare at all

Found the other way round -- not by checking the stubs but by checking a project against them.
`phrost check` on Symfony's own source reports 1 460 `undefined-type`, and **911 of them are two
extensions this set has no stub for**:

| | count | what it is |
| --- | ---: | --- |
| `Relay\Relay`, `Relay\Cluster`, `Relay`, `Relay\Exception`, `RelayCluster` | 648 | `ext-relay`, which has no file here at all |
| ~~`RedisCluster`~~ | ~~257~~ | **added** -- see Local edits above |
| `Sentinel` | 6 | the same file's `RedisSentinel` under its other spelling |

`ext-relay` is left out on purpose rather than for want of a source. It is one corpus's optional
cache backend -- Symfony names it in a union beside every other one it supports -- and nothing in
the other three corpora mentions it, so vendoring a second extension's worth of signatures would
be paid for by one instrument. `RedisCluster` earned its place by appearing in two.

Two of the original 48 *were* phrost's and are fixed: `enum RoundingMode implements UnitEnum` reported
`cases()` as unimplemented, because an interface may extend `UnitEnum` even though an enum may not
implement it directly -- and `Ds\Sequence::push(...$values)` is `@param TValue`, so `$deque->push(1)`
reported `$values takes \Ds\TValue` on code that runs. Both were found by checking this directory,
which is what it is worth doing for.

## Updating

Pinned deliberately rather than tracked: a stub set that changes under a fresh checkout changes what
the index knows, which is the same reason `Cargo.lock` is committed.

**Do not copy the new files over these.** That was what this section used to say, and it is the one
procedure guaranteed to lose the 2 144 summaries -- they live in the same docblocks upstream edits.
Take the *upstream diff* instead, and merge it in three ways: the pinned version's file as the base,
this directory's as ours, the new one as theirs, one `git merge-file` per changed file. 1.47.2 ->
1.48.0 touched four files and 166 lines, which is the normal size; almost every declaration it
touched also carried a summary, so nearly every hunk conflicted, and the conflicts have exactly
three shapes:

- **Ours is prose then tags, theirs is tags.** Keep our prose, take their tags -- upstream moved
  what our tag said into the signature (a return type, a `#[\Deprecated]`). Mechanical, and the
  bulk of them.
- **Theirs is empty.** A declaration was removed. Check what else is inside the conflict before
  taking the empty side: one of them held the removed method *and the docblock of the next method,
  which survives*, so taking theirs whole would have deleted a summary silently.
- **A declaration moved.** `git merge-file` keeps both copies and nothing complains. This is the
  one that got through here -- `ReflectionAttribute::__toString` was declared twice -- and what
  caught it is that `reflection.php` is the only one of the four that `php -l` can lint, the rest
  failing on `Cannot redeclare` against PHP's own built-ins before they reach anything real.

Then check, in this order, because each catches what the one before cannot:

1. **Declaration sets against upstream's**, keyed `Class::method`, per file. Nothing missing,
   nothing extra, no new duplicates. This is what proves the merge structurally, and it is the only
   check that sees a moved declaration.
2. `cargo nextest run -p phrost-stubs` -- the parse test and the summary tripwire.
3. `phrost check third_party/php-prelude` against the known-defects table above -- 54, and the
   same six kinds, beside the `no-ffi` and `no-literal-password` lint findings the declarations make.
4. `phrost check` over the corpora, diffing the **findings** and not the counts.
