# Types, resolution and narrowing

> Part of the record §8 of `CLAUDE.md` began. Each note is a bug that was paid for once; the point
> of writing it down is that the _general_ shape at the end of each one recurs. Where a note says
> "§8", it means the record as a whole — `CLAUDE.md` §8 plus this family of documents. **Read this
> file before working in the area it names.** Add new notes here rather than to `CLAUDE.md`, and
> keep them in the shape the existing ones have: what looked true, what was true, what the tell was,
> and what generalises.

The type table, `Held`, guards and narrowing, docblock tags and templates, the compatibility
judgement, and the checks built on them. This is the largest part of the record because it is where
one rule most often ends up with two constructors.

**The notes, by their tell:**

- A degenerate form a union constructor does not fold out silently defeats every guard.
- A union's _spelling_ reached the type table, so a render bug was an inference bug.
- A _builtin_ arrives in three spellings too, and the same rule caught arithmetic.
- A union arrives in three spellings and only one of them is `Held::OneOf`.
- `PathRoot::EitherOf` reads every operand at the joining expression's offset.
- Adding a `BuiltinTy` variant compiles clean and changes behaviour silently.
- A local exists in two places, and a new binding construct has to reach both.
- An interface's parameter type is a lower bound, not the contract.
- A docblock refines a return and widens a parameter, and the difference is not a preference.
- `passed_to_calls` only sees a call written as a statement of its own, and that is most of what a
  by-reference parameter is not.
- A scope that begins at the body leaves the whole signature outside every feature.
- Two source spellings can intern to the _same_ type, and then the distinction is gone before any
  reader exists.
- Two resolvers answer "what does this template mean", and a union needs both.
- A docblock tag naming a template is not a competing claim about the type, and `refined` cannot
  tell.
- A declaration may write the same tag twice, and four readers all took the first one.
- A template name reaching a judgement is wrong in _both_ directions, so the tell is not a false
  positive alone.
- A by-reference parameter's declared type is a lower bound too, and that makes three.
- A _finished_ statement can still hold statements that ran, and the tell is that the assignment was
  visible while the guard was not.
- A guard whose rule exists, is wired, and fires -- and is powerless, because it was written as a
  filter over alternatives the declaration had already listed.
- The same asymmetry between one class and several had to be closed in _three_ places, and each one
  hid the next.
- A rule can be right and still buy nothing, because a _different_ rule is the blocker -- and the
  weight measurement cannot see that.
- An assignment into a _declared_ slot refines it and cannot widen it, so reading the write as the
  answer is a regression wearing a fix's clothes.
- A guard can be about a value its condition never names, and `?->` is the operator that makes that
  sound.
- `?:` is not `??`, and spelling both as one flag put a `false` in a `?string`.
- A union whose operand is itself a union defeats every guard, and the _reader_ is where to fix it.
- Two readings of one question, dispatched on the shape of the input rather than on the question.
- `strict_types` is not the whole of what PHP refuses, and the difference is the _literal_.
- A fact derived from the tree alone cannot answer a question about a declaration, and the
  derivation quietly answered it anyway.
- A local and a property slot are one join, and holding them as two constructors cost fifteen false
  positives on the first check that read a return.
- A gate whose absence is a _lost_ answer, and a gate whose absence is a _stale_ one.
- A walk that matches roots by _shape_ stops at the first shape nobody added.
- A value means what it meant where it was written, and two places forgot.
- A guard that _filters_ is not a guard that _claims_, and `is_string` was only the first.
- A name that resolves to no class is not a type, and only _one_ of the two ways to write one was
  refused.
- A conditional write's join has three operands and the third is about a path that sometimes does
  not exist.
- And then the guard's complement, which was waiting on it.
- And the write whose guard says nothing was throwing away the writes beside it.
- A closure can run as a class it is not written in, and `Closure::bind` is how a framework says so.
- A destructuring target writes through no variable, so every name it binds had no type at all.
- An element write is the element type only where the container started here, and that was answering
  for arrays filled elsewhere.
- Two things an outside reading of 82 new findings caught that measurement did not.
- `iterable` and `callable` are not families of their own, and a filter written as equality dropped
  what answers them.
- `element_written` took every write before the offset, in any branch.
- `@return MockObject&T` is what every mock a PHPUnit test makes is written as, and both readers of
  a template declined it.
- A namespace is declared by nothing of its own, and the import check asked only about classes.
- `use ($x)` and `use (&$x)` were one shape in the tree, and the `&` is a _write_.
- The class direction of a guard had one complement and the other two directions had both.
- A leaving guard nested one branch deep is invisible to the code after that branch, and the
  frequency measurement was the wrong reason to leave it.
- A renderer is judged by a person reading it until something _writes_ its output into a file, and
  then a spelling it cannot parse loses the whole type in silence.
- A union keyed by a name looked through no sentinel, and `reset()` is where everybody meets it.
- A disclosure that fires wherever a rule _applied_ rather than where it _decided_ anything is seven
  parts noise, and the filter is the checks themselves.
- `{@inheritdoc}` carried a return down and not a parameter, and the two halves of one signature
  were the two constructors.
- Three walks each wrote `if argument.name.is_some() { break; }`, so a call written by name was
  judged not at all.
- `resource` is class-like where a declaration writes it and a type where a docblock does, and only
  PHP could settle which.
- `array{...}|array{}` lost its keys at three collapses in a row, and each was the spelling beside
  the one that worked.
- PHP has four shapes of call and `site_of` matched three, so a closure was not a call at all.
- A through-write was a flag, so one stale statement silenced every slot write for the rest of the
  body.
- The template leak was in the _sentinel_ check, not in lowering -- and lowering was where the first
  fix went.
- A bare `object` receiver is 69 parts a project's own declaration and 5 parts a service, and
  neither is a gap.

---

- **A degenerate form a union constructor does not fold out silently defeats every guard.**
  `Held::one_of` folded out a one-operand wrapper and a bare null, and said why. It did not fold out
  a _nested_ union, because until a join existed nothing built one. Every narrowing works by taking
  a union apart and keeping the operands that survive, and an operand that is itself a union answers
  nothing about its own class or kind -- so a positive test drops it unexamined and a negative one
  keeps it whole. Nothing fails: the guard runs, returns a type, and is powerless. The tell is a
  guard that works on a declared type and stops working on the same type reached another way. When a
  layer starts _building_ a type rather than pointing at one, the constructor's degenerate-form list
  is the first thing to revisit.

- **A union's _spelling_ reached the type table, so a render bug was an inference bug.**
  `string|int|null` interned as `Union[string, Nullable(int)]`, because both grammars that write `|`
  are binary and nothing folded the result. It looked cosmetic -- hover printed `string|?int`, which
  PHP cannot even spell -- and it was not: a not-null guard removes an operand from a union, and
  here the null was welded to whichever operand the parse happened to nest it with, so the guard
  could not reach it. One real `possibly-null-argument` false positive on the reference project, on
  a line reading `if ($node && $node->id())`. Fixed at `TypeTable::union`, which now folds out a
  nested union, a `?T` operand and a repeat -- the same degenerate-form list §8 already records for
  `Held::one_of`, which had learned it and the interned constructor had not. Two constructors of one
  concept is how one of them comes to be missing a rule the other has.

- **A _builtin_ arrives in three spellings too, and the same rule caught arithmetic.** `7 * 2`
  answered `int` and `$n * $m` answered nothing, because `arithmetic_result` matched `Held::Builtin`
  and a parameter's `int` is a `Held::Written` into the file's arena or a `Held::Stored` interned in
  the project's table. Every computed number in a project is the second kind, so the whole of a
  codebase's arithmetic was untyped while its literals were fine -- which is why a probe written
  with literals reports the feature working. `scalar_named` already existed **forty lines below in
  the same file**, with a doc-comment stating exactly the three spellings. When a rule matches one
  shape of `Held`, the question is never whether the other spellings exist; it is which helper
  already knows them.

- **A union arrives in three spellings and only one of them is `Held::OneOf`.** A rule written on
  that arm governs unions built by narrowing and nothing else: a docblock `@return A|B` is an
  interned `TypeRef::Union` inside a `Held::Stored`, and a `@param A|B` is a `TypeRefData::Union`
  inside a `Held::Written`, and both fall past it into the single-type path. Nothing fails -- the
  arm is exercised by guard tests and reads as covered. `resolve/narrowing.rs::union_operands` knows
  all three; a second reader of a union calls it rather than re-matching.

- **`PathRoot::EitherOf` reads every operand at the joining expression's offset.** So
  `is_array($x) ? $x[1] : $x` joins the _un-narrowed_ type, even though hover at either branch shows
  the narrowed one -- hover resolves at the branch. `MaybeWritten` beside it carries a position per
  write for exactly this reason, and `EitherOf`'s own doc-comment states the single position as
  though it were harmless. It is invisible while either operand resolves to nothing, and becomes a
  false positive the moment inference improves upstream -- looking like the new work's fault.

- **Adding a `BuiltinTy` variant compiles clean and changes behaviour silently.** No `match` on
  `BuiltinTy` in the workspace is exhaustive -- every one has a `_ =>` arm or names its variants --
  so a new one produces zero compiler errors while falling down the wrong branch everywhere. Adding
  `List` to try `list<T>` was measured doing exactly that: `compat::builtins` has a catch-all
  `(_, Array | Iterable | Object) => Rejected`, which turned every `list` against `array` into a
  false positive; `references.rs::has_no_members` stopped reporting; `predicate.rs::written_kind`
  broke `is_array` narrowing. The vocabulary is closed by convention rather than by the type system.
  Treat an addition to it as a semantics change needing the ratchet, never as a widening.

- **A local exists in two places, and a new binding construct has to reach both.** `local_binding`
  answers _what does this name hold_; `bound_names` / `locals_in_scope` answers _does the name exist
  at all_, and `chain_root` goes through the second before it can use the first. Teach only one and
  it works everywhere the variable also appears as an expression -- because `local_name` falls back
  to scanning expressions for the name -- and fails silently where it does not. `@var` annotations
  were in `local_binding` and not in `bound_names` for exactly that reason, and the lowering fix for
  them moved nothing until the second search learned too.

- **An interface's parameter type is a lower bound, not the contract.** PHP's parameters are
  contravariant, so an implementation may accept _more_ than the interface declares -- Drupal's
  `StatementInterface::fetchAllAssoc(string, ?FetchAs)` is implemented by
  `StatementBase::fetchAllAssoc(string, FetchAs|int|null)`, and passing an int runs. A call typed
  through the interface therefore cannot be judged from the interface alone, and one known false
  positive on the reference project comes from exactly that. Declining on every interface method was
  measured instead of assumed: 8 of 81 findings are interface calls and 7 of them are real, so the
  blunt rule costs seven to buy one. The precise rule -- ask whether some implementation widens this
  parameter -- is `implementers_of`, a scan of every class in the index, which is the shape the
  `NeverReturns` note forbids on a request path.

- **A docblock refines a return and widens a parameter, and the difference is not a preference.**
  What a caller is told it _got_ can only get more precise; what a callee documents it _undertakes
  to handle_ can only add. `effective_ty` narrows, which is right for a return and silently wrong
  for a parameter -- it discarded Drupal's `@param FetchAs|int|null` beside a `?FetchAs` hint and
  reported eight working calls. The fix costs no memory, because lowering already stored both types;
  only the reader was choosing one. A narrower `@param` still changes nothing, and that falls out of
  offering the two as alternatives rather than needing a case of its own.

- **`passed_to_calls` only sees a call written as a statement of its own, and that is most of what a
  by-reference parameter is not.** Its doc justifies the restriction for `@assert` --
  `$this-> assertNotNull($x);` is written for its effect and nothing else -- and the by-reference
  reader shares the walk. Measured on the four ways one function is written:
  `preg_match($p, $s, $m);` answers `string[]`, and `if (preg_match($p, $s, $m))`,
  `$ok = preg_match(...)` and `if (preg_match(...) === 1)` all answer nothing. The first is the
  shape a probe reaches for and the second is the shape everybody writes, which is how "`@param-out`
  works" was measured and reported on one case in four. On the reference project all three real
  capture sites are `if (preg_match(...))` or `if (!preg_match(...))`: nought of three.

  **Fixed with two walks, and the second walk is the point.** Widening the shared one cost
  `phrost check` **2.42 s -> 3.15 s**, +30 %, because `contract_narrowed` asks it about every value
  whose declaration was vague and on a real project that is most of them -- the `NeverReturns` shape
  again, and again invisible to every test. `passed_to_any_call` is the wide walk and is reached
  only from the `ByReference` root, which exists precisely for a name nothing else bound: rare, and
  the only place the width is needed. Back to 2.46 s, with eleven of eleven shapes answering. The
  lesson is that "widen the walk" and "widen the walk _for this caller_" are different changes with
  the same diff size, and the second one is usually the one that was wanted.

  Two details of the walk are load-bearing and were each found by one failing row of a matrix. A
  statement wholly behind the offset needs its **condition** walked too, not just its own expression
  -- `if (matched($p, $s, $m)) {} $m` was the last shape still silent. And the `else` branch must
  see the write: PHP fills `$matches` whether or not the pattern matched, so making the tag visible
  only in the selected branch would be a type narrower than the truth. One shape stays unsound on
  purpose: under a short-circuit, `if ($f && matched(..., $m)) {} else { $m }` claims a type where
  the call may not have run, and dropping calls under `&&` would lose the sound and far commoner
  branch beside it.

- **A scope that begins at the body leaves the whole signature outside every feature.**
  `Scope::chain_at` resolved by body range, and its doc-comment justified that correctly for a
  parameter's _type hint_ -- then gave the same answer for the parameter's _name_, which is the
  local's binding site. Four capabilities were dead at that one offset and perfect one word to the
  right: rename reported success and edited nothing, and `documentHighlight`, `references` and
  `typeDefinition` all returned zero. No error anywhere. The tell for this class of bug is a feature
  that works at a _use_ and not at the _declaration_ of the same thing. Anything keyed on "which
  body contains this offset" has to state what it does about the signature.

- **Two source spellings can intern to the _same_ type, and then the distinction is gone before any
  reader exists.** Not the "three spellings" trap -- that is one type reached three ways. This is
  two types collapsed into one: `list<V>` and `V[]` both lowered to a one-argument
  `Generic{array,[V]}`, and they say different things, because brackets name a container and nothing
  about its keys where a list names `int` keys by definition. So the rule that needs them apart
  cannot be written above lowering _at all_ -- no amount of care in `compat` recovers a fact the
  type table never held. The fix was to materialise what the spelling means (`list<V>` ->
  `array<int, V>`), which makes the argument count the discriminator and needs no new vocabulary.
  The second half is the render direction, and it is not a matter of taste: when two spellings do
  collapse, print the **weaker** claim. Rendering `array<int, Node>` as `list<Node>` was tried and
  reverted -- `list` claims contiguity, and `array<int, Node>` keyed by entity id is the commonest
  int-keyed array in a framework project and emphatically not a list. Also worth knowing:
  `expect-test`'s inline diff can render an unbalanced string for a nested type change
  (`array<int, list<Row>` for what is actually `list<Row>`); accept into a scratch commit and read
  the file rather than trusting the diff.

- **Two resolvers answer "what does this template mean", and a union needs both.** A `@template`
  written on the _method_ and bound by an argument's own type is resolved by the contract reader,
  which needs the call site; one written on the _class_ and bound by the receiver's
  `@var Bag<string>` is resolved by the generics substitution, which needs the receiver. Each is
  complete for its own kind and blind to the other's, which is invisible until a return names one of
  each -- `@return TDefault|TInput` on Symfony's `InputBag::get` -- and then it resolves in neither.
  The tell is a union answering nothing while each of its operands answers alone. Related and
  cheaper to get wrong: when a union declines an operand, decline on _names nothing real_ rather
  than on _is a class template_, or the operands the other resolver owns are silently kept as names
  resolving to nothing.

- **A docblock tag naming a template is not a competing claim about the type, and `refined` cannot
  tell.** That judgement ranks a hint above a docblock unless the hint says nothing -- `object`,
  `mixed`, a bare container -- and a template name is a `Named` that looks exactly like a class to
  it. So `@return T` beside `: ?Node` lost, and on one real vendor tree that is **124 of 145**
  templated returns, because modern PHP writes the hint and lets the docblock add what PHP cannot
  spell. The choice has to be made where the owner's template list is in hand. Keep the hint when
  you make it: a `@template` class written with no arguments is ordinary -- Symfony's `Request` has
  `@var InputBag<string> $query` beside a bare `InputBag $request` -- and preferring the tag
  unconditionally turns a real declared type into nothing.

- **A declaration may write the same tag twice, and four readers all took the first one.**
  Doctrine's `EntityRepository::find` writes `@return object|null` _and_ `@phpstan-return ?T`; the
  plain tag is there for a reader that cannot hold the precise one -- PHP has no syntax for `T` and
  `object|null` is the closest thing that is true -- so the prefixed tag is not a second opinion.
  Taking whichever came first made `find()` an `object|null` while `findOneBy()` four lines below,
  writing only the prefixed tag, answered the entity. mago folds the prefixed spellings into one tag
  value and records the prefix in `Tag::vendor`, so this is a comparison rather than a scan for
  names -- and it applies to **every** tag kind: `@phpstan-param` is 588 occurrences on one vendor
  tree against `@phpstan-return`'s 381. Two consequences worth carrying. Preferring the precise tag
  can _lose_ a resolution where the plain one named a real class and the precise one is a template
  nothing binds (Drupal's `ListInterface::get()` is `@return TypedDataInterface|null` beside
  `@phpstan-return ?T`) -- and on the reference project those lost resolutions were exactly the
  sites of two `undefined-method` false positives, so `phrost coverage` going down was it counting
  confidence rather than correctness. And a cache that writes one flag while the reader reads two
  silently desynchronises the whole contract stream; the compiler's unused-variable warning is what
  caught it.

- **A template name reaching a judgement is wrong in _both_ directions, so the tell is not a false
  positive alone.** `@param T $item` resolves to no class, so `compat` rejected every scalar for not
  being one (`$deque->push(1)` reported `$values takes \Ds\TValue`) _and_ accepted every class
  because nothing relates to an unresolvable name. A probe that only checks for false positives
  finds half of it; one that passes a deliberately wrong class finds the rest. What settles it is
  what the receiver was written with -- which `parameters_of` had in hand and reduced to a class
  before returning. When a check is asked about a type that is a _name standing for_ a type, ask
  what bound it before asking whether it fits; and substitute for the _message_ too, because a
  report naming `T` tells a reader nothing they can act on.

- **A by-reference parameter's declared type is a lower bound too, and that makes three.** Reading
  `sort(array &$array)` as "an array is what is there now" threw the element type away at 36 sites
  on one real checkout -- `sort` 11, `usort` 11, `array_splice` 6, `uasort` 4, `rsort` 2,
  `array_walk` 1, `array_push` 1. Never a false positive, because wider than the truth reports
  nothing; it is precision that existed and stopped existing, which is the same loss as a `T[]`
  leaking into a render arriving through inference instead of a renderer. The fix is to ask the
  parameter only whether it **contradicts** what the caller knew, and `Fit::Rejected` rather than
  `Fit::Accepted` is the test -- `sort` writes `@param array<T>` beside its `array` hint, a bare
  container is a hint that says nothing, so `effective_ty` is `array<T>` with `T` bound by nothing
  and a `list<Item>` against that is `Fit::Unknown`. Demanding `Accepted` would have kept the bug on
  the one function that exposed it. `mixed` stays the exception in the other direction, because it
  is not a bound at all. Measured: `check` 2.41-2.43 s either way, one finding removed and none
  added -- and that one was a false positive of exactly this shape, `?array &$entity` given a
  non-null `array` by a callee that never assigns to it. What generalises is the count: **three**
  notes here now say a declaration on the _input_ side is a lower bound (an interface's parameter, a
  `@param` docblock, a by-reference parameter), and the next reader of one should assume it is a
  fourth before assuming it is new.

- **A _finished_ statement can still hold statements that ran, and the tell is that the assignment
  was visible while the guard was not.** `visit_preceding_statements` visited a completed statement
  and stopped there, which is right for an `if` -- a guard in a branch proves nothing once the
  branch is behind you -- and wrong for a `try`. So
  `try { $p = $i->product(); if ($p === NULL) { throw ...; } } catch (E) { return ...; } $p->m();`
  reported `possibly-null-receiver` on a line that plainly checks for null, and it read as a
  _resolution_ bug because `$p` still resolved to `?Product`: the walk that finds writes descends
  into blocks and the walk that finds guards did not. One value, two searches, only one of them
  looking inside -- the same shape as `local_binding` against `bound_names`, which §8 already
  records. The condition is what makes the fix sound rather than convenient: a `try` body earns the
  descent only when **no catch clause can complete normally**, because a catch that falls through
  reaches the code after the statement without the guard having run (`always_leaves` per clause
  answers it, and an empty clause list is vacuously true, which is right -- a `try`/`finally` with
  no catch has only the one normal path). A `finally` earns it unconditionally, since it runs on
  every path that reaches the `try` at all. A bare `{ }` block earns it for the same reason. A loop
  does **not**, however similar it looks, because it may run zero times. Measured: 1483 -> 1480
  findings, none added, `check` 2.43 s unchanged -- and the third one nobody had reported was the
  `try`/`finally` shape narrowing a `string|false`.

- **A guard whose rule exists, is wired, and fires -- and is powerless, because it was written as a
  filter over alternatives the declaration had already listed.** `one_of_instanceof` reads
  `$x instanceof A || $x instanceof B` whole, `Proved::OneOfClasses` carries it, and the resolution
  narrows a declared union down to the classes it named. That works for `Baz|Boo|int` and answers
  **nothing** for `mixed` or for a parameter with no hint -- which is how the guard is almost always
  written, because a value whose declaration already lists the alternatives rarely needs the guard.
  The missing half is one `also_of_class` has had all along for a _single_ class: nothing named
  before, so the guard is the whole answer. Two constructors of one rule, and the same asymmetry
  appeared **twice, one layer apart** -- the helper kept a `mixed` unchanged, and the arm above it
  used `?` on a resolution that legitimately returns `None`, so the unhinted parameter needed both
  fixed. Corpus 343 -> 345, and the two fixtures still failing are a _stated position_
  (`instanceof A || true` is not a narrowing, whatever phpactor offers) and a separate rule (De
  Morgan over a negated conjunction whose branch leaves). Zero change on the reference project, and
  that is the honest reading rather than a disappointment: it writes the shape **twice** in 1 662
  files. The general tell is a narrowing that works on a declared union and stops working on the
  same value reached with no declaration -- §8 already records the mirror of it, where a guard works
  on a declared type and stops working on the same type reached another way.

- **The same asymmetry between one class and several had to be closed in _three_ places, and each
  one hid the next.** `also_of_class` knows three things a guard can mean; `of_one_of_classes` knew
  one. Fixing the helper's "nothing named before" arm made `mixed` work and left the unhinted
  parameter, whose _resolution arm_ used `?` on a lookup that legitimately returns `None`. Fixing
  that made both work and left the commonest case of all -- a receiver that already names a class --
  which needs the third rule: every class the guard named being below what the value already names
  makes the guard strictly the more specific claim. Until that one, `Zero $z` guarded by
  `instanceof One || instanceof Two` proved nothing, and `Zero` is precisely how a sealed hierarchy
  is written. The tell each time was a narrowing that worked on one declaration and stopped on
  another.

- **A rule can be right and still buy nothing, because a _different_ rule is the blocker -- and the
  weight measurement cannot see that.** De Morgan over `!$x instanceof A && !$x instanceof B` was
  measured as buying zero corpus fixtures and zero project sites, and refused on that basis. The
  measurement was correct and the conclusion was wrong: one fixture it would win
  (`parenthesized_narrowing`) was silent for the _third_ reason above, and the one I had read
  (`combined_negated_guards`) was blocked by the union-leniency position, so counting fixtures found
  nothing either way. What settled it was reading the fixture's _types_: two classes sharing a
  method, where a union really does offer something. Counting what a rule would fix is only
  meaningful once every other rule between it and the answer is known to work -- otherwise a zero
  means "something else is also missing", not "this is not worth it".

- **An assignment into a _declared_ slot refines it and cannot widen it, so reading the write as the
  answer is a regression wearing a fix's clothes.** A property filled on one line and read on the
  next still answered the `?T` its declaration says, because a guard about `$this->x` was read and
  an assignment to it was not; teaching the resolver to read the assignment removed five real
  findings on the reference project. Taken as the _whole_ answer it also added five:
  `$this->node = $storage->load($id)` puts an `?EntityInterface` into a `?NodeInterface` property,
  and PHP checks a property write at runtime -- so at the read the declaration is the guarantee and
  the write is worth having only where it is the more specific of the two. Reading it as the answer
  turned five true `possibly-null-receiver` warnings into
  `undefined-method: EntityInterface::get()`. The test is `Fit::Accepted` against the declared
  answer, and demanding `Accepted` rather than "not `Rejected`" is what makes it work -- an
  interface where another interface is wanted is `Unknown`, which is exactly the case that must keep
  the declaration. Same rule §8 already records for a docblock refining a return, arriving from a
  third direction.

- **A guard can be about a value its condition never names, and `?->` is the operator that makes
  that sound.** `if ($request?->firstInvoiceDate !== NULL)` is a guard about `$request` as much as
  about the property -- the branch is then written `$request->firstInvoiceDate`, with a plain arrow,
  because the author already knows -- and a reader matching the subject against what the condition
  _names_ finds nothing and reports the receiver. Six on the reference project, in three spellings:
  `!== NULL`, a bare truthy call, and the negated early return `if (!$x?->y) { return; }`. Sound
  only through `?->`: a plain `$a->b !== null` says nothing about `$a`, because had it been null the
  program would have died at the arrow rather than reached the comparison -- the test is downstream
  of the assumption rather than evidence for it. The second half is the one that nearly ate the fix:
  the arm reading a method's `@assert` contract matches `$x?->isReady()` _first_, so a new claim
  added after it has to **compose** rather than rank -- `Proved::and_also` says at length why, and
  this is its third instance.

- **`?:` is not `??`, and spelling both as one flag put a `false` in a `?string`.** Both operators
  choose the right-hand value when the left did not survive, and _what_ did not survive is the whole
  difference: `??` tests for null, `?:` tests truthiness. `tempnam(...) ?: NULL` therefore answered
  `string|false|null` -- `false` being exactly what that line is written to replace. The type layer
  can name two falsy values and that is as far as it goes; a `0` or an `''` is a value no type in
  the table names.

- **A union whose operand is itself a union defeats every guard, and the _reader_ is where to fix
  it.** §8 already records this for `Held::one_of`, which folds the one spelling it can see;
  `narrowed_union` is the other half -- it asked `union_operands` for the operands and judged each
  _whole_, so an operand that is itself an interned union answered nothing about its own kind and
  survived intact. `int|false` joined with an `int` kept its `false` right through
  `if (!$timestamp)`: the guard ran, returned a type, and changed nothing. Narrowing operand by
  operand is the same claim applied to each alternative, and it is five findings on the reference
  project. The attempt that came first is worth keeping: flattening the operands where the union is
  _built_ rather than where it is read. It fixes the same five and adds **twenty-nine**, because
  `every_operand_names_one_class` reads a `?Foo` operand as naming `Foo` and collapses the join to
  it -- dropping a null nothing proved impossible. That hole is load-bearing: `MaybeWritten` models
  each write independently, so `if (!$selected) { $selected = $currentQuote; }` leaves a null the
  model cannot rule out, and the one-class collapse is what has been hiding it. Making the union
  honest exposes the modelling gap; narrowing at the reader does not. Also, recursion through a
  predicate taken as `impl Fn` hits the monomorphization limit -- `&dyn Fn` is what compiles.

- **Two readings of one question, dispatched on the shape of the input rather than on the
  question.** `element_written` asks _what went into this slot_, and there are two ways a statement
  answers: a write into a slot of the container (`$rows['a'] = $pen`), and a literal assigned to the
  container itself (`$rows = ['pen' => $pen]`). It matched on the **target's** shape, so a write
  whose target was an index only ever got the first reading -- and
  `$rows['a'] = ['pen' => new Pen()]` is both at once: a write into a slot for one container and a
  whole literal for another. So `$rows['a']` answered the literal and `$rows['a']['pen']` answered
  nothing, which is how every array of records in PHP is built. Invisible from either end, because
  each half works alone. The fix is to try both readings and let the container decide; they are
  disjoint by construction, since one compares the container against the write's _base_ and the
  other against the whole target, and a path cannot equal both. Measured on 1 710 real files: **+42
  member accesses resolved, +6 assignments typed**, no findings changed. What does _not_ follow is
  the travelling case -- `$x = $rows['a']` and `foreach ($rows as $row)` both resolve the literal
  into a `Held` first, and `Held` has no arm for a shape, so the keys are gone before the next step
  asks. That is 321 `Held::` sites and 37 catch-alls in `resolve/`, against two corpus fixtures.

- **`strict_types` is not the whole of what PHP refuses, and the difference is the _literal_.** Weak
  mode converts a string where an `int` is declared, so the type judgement is right to accept one --
  a `string` may well hold `'12'`. A literal that is not a number cannot: measured on PHP 8.5,
  `needs_int('bad')`, `$holder->count = 'bad'` and `return 'bad'` from `: int` are each a
  `TypeError` on every run, and so are `'5 apples'` and `''`, while `'12'` and `'12.5'` get through.
  What makes the rule cheap is asking it of the **strict** table against a plain `string` rather
  than peeling the expectation: if a string is rejected there, coercion was the only thing that
  could have saved the value -- so `int|string`, `mixed` and an unbound template stay silent with no
  case of their own. Three checks have the written expression in hand (an argument, an assignment, a
  return) and they share one entry point, because two constructors of one rule is how one of them
  comes to be missing what the other has -- and one of them needed it _twice_, since
  `rejected_return` asks the judgement again over a union's alternatives and the second ask silently
  undid the first. Zero findings added on 1 662 real files, which is the honest reading: it is a bug
  PHPStan catches at level 5 in weak files too. What it is worth is the other direction -- **PHPStan
  at level 5 says nothing about nullables** (that is level 9), so the two tools' silences are
  complementary rather than overlapping, and it is worth measuring which is which before building
  either.

- **A fact derived from the tree alone cannot answer a question about a declaration, and the
  derivation quietly answered it anyway.** `facts.rs` computed "does this branch leave" once per
  revision with a `never` oracle hard-wired to `false`, because a `: never` declaration lives in
  another file and the derivation has no reader -- so `if ($c) { $x = new Bar(); abort(); }` joined
  `Bar` into `$x` for as long as the facts existed, and the three fixtures for it had been failing
  since before the facts replaced the walks. The shape that fixes it is `Leaving::Ask`: settle what
  the tree settles, and keep the one unsettled case _as the question_, put to the reader's oracle at
  lookup. Cheap because the oracle rejects nearly every call by name. Related, and the actual bug in
  two of the three: `leaves` read a list's _last_ statement, so `abort(); $dead = 1;` counted as
  falling through -- and a guard clause looks exactly like that the moment somebody forgets to
  delete the line after it. Any statement that leaves settles the list.

- **A local and a property slot are one join, and holding them as two constructors cost fifteen
  false positives on the first check that read a return.**
  `if ($this->cache === NULL) { $this->cache = $this->build(); } return $this->cache;` is the
  commonest way a property is filled, and it is exactly `if (!$v) { $v = ...; }` with `$this->cache`
  for `$v`. The local's search joined the branch write with what the name held before, minus what
  the guard ruled out; the slot's search _withdrew_ its claim on any branch write, so every lazily
  filled property read as its declared `?T`. `PathRoot::MaybeWritten` now carries the path to read
  rather than a name, and both searches build it. One rule is load-bearing: a slot joins **only
  under a guard about the slot**. A write to a slot must fit its declaration, so with nothing proved
  the join says what the declaration says -- and joining anyway _loses_ a null, because `union_of`
  collapses a class beside its nullable to the class (the hole §8 already records above), which
  turned `if ($when) { $this->request = $this->build(); } run($this->request)` silent. The
  withdrawal is the declaration, and the declaration is right there.

- **A gate whose absence is a _lost_ answer, and a gate whose absence is a _stale_ one.** Reading
  `@phpstan-assert-if-true` on a plain function in a guard is 145 false positives on WordPress and
  one true finding the code admits in a comment -- but the arm alone had to be reverted once,
  because `PathRoot::AssertedArgument` resolves its subject at the _guard_, so a root built for a
  callee that promises nothing answers with what the value held before the branch. Not wrong:
  **stale**, with the binding reading "narrowed by a guard" for a position no guard narrowed. The
  `NeverReturns` shape answers it -- a name set the index fills where it already notes a `never`
  return, scanned before anything resolves -- and threading it cost **8** signatures, not the 24
  that carry `never`, because `Flags` already reaches every recursion of `proved_about`. Look for
  the existing carrier before pricing the plumbing. Kept beside `never` rather than bundled: the
  flow, reach and facts layers ask `never` and hold no guard, so a struct would hand fourteen
  callers a field they never read.

- **A walk that matches roots by _shape_ stops at the first shape nobody added.**
  `class_a_path_names` had an arm for a local and one for an annotation, so _any_ guard between a
  `@var class-string<Rule>` and its `new $class` lost the argument -- `is_string`, `=== NULL`,
  `class_exists` -- and equally through a static method's assertion, which had shipped for months.
  Nothing widened anything, which is why the diagnosis in the record before it was wrong: the walk
  simply had no arm. `PathRoot::narrows` answers what a guard root narrows and where,
  **exhaustively**, naming every non-guard root rather than falling through a `_`, for the reason §8
  records about `BuiltinTy` -- a tenth guard root must not answer "not a guard" quietly. The fixture
  written to pin that defensive checks cost nothing declared no `class_exists` of its own, and a
  fixture has no stubs, so nothing in it narrowed at all: it pinned the gap and its comment claimed
  the opposite.

- **A value means what it meant where it was written, and two places forgot.** `element_written`
  handed back the value put into `$apps[$n]` and the reader resolved it where the slot was _read_,
  after the `if` whose throwing guard had made it non-null -- so every memoising
  `static $cache = []` in a Drupal service returned a nullable. `ElementWrite` carries the position
  now, the rule `MaybeWritten` already followed per write. And `Proved::and_also` composed a third
  guard operand by _replacing_ a contract's receiver with the new claim, discarding the receiver
  that already carried the first operand's not-null -- so `!$a || !$a->m() || !$this->x->y($a)`
  proved less than the same guard with two operands. A composition that takes `..` from the thing it
  composes onto is the tell.

- **A guard that _filters_ is not a guard that _claims_, and `is_string` was only the first.**
  `of_kind` kept the operands of a declared union that were of the kind and left every other value
  as it was, which is the same asymmetry §8 already records three times for a class -- and here it
  was two false positives at once. Null answers `false` to every one of PHP's predicates, so
  `if (is_string($s))` on a `?string` is a null check; a `?string` is not a union to
  `union_operands` while `kind_of_held` reads through the `?`, so the guard filtered nothing and the
  null it had just ruled out was reported on the next line. And a value believed to be _null_ inside
  `if (is_array($v))` was passed on as null, so the call the guard exists to protect was reported as
  being handed one. Measured: **174 findings gone across five corpora** -- WordPress 4046 -> 3950,
  Symfony 19704 -> 19662, Laravel 7583 -> 7563 -- and `check` slightly _faster_, because the other
  half of the fix walks less. Two things had to come with it or the fix reports working code.
  **`in_array($x, [<scalars>], true)` claims "an object" on the branch where the needle was not in
  the haystack**, which is a way of removing the strings from a declared union and true of nothing
  else; inert while a kind nothing held could not be an answer, and 12 false positives the moment it
  could. So `Proved::Kind` carries whether the guard **named** the kind (`settled`) and only a named
  one may be the whole answer. **And `visit_preceding_statements` descended into _both_ branches of
  the `if` the offset sits inside**, so a leaving guard written in the first arm put its claim into
  the second -- `if (is_array($m)) { if (!is_int($m)) { throw; } } elseif (...) { $m }` read `$m` as
  an int on a path where neither line ran. Whichever branch holds the offset, and never both. What
  it made visible were three _other_ bugs, each fixed in a commit of its own and written up below:
  an argument judged against a free function's own `@template`, a conditional write's join reading a
  skipped path that does not exist, and `Otherwise` complementing a guard that also holds at the
  read. The order mattered -- the third was written first, measured as trading three WordPress false
  positives for four elsewhere, and reverted; with the second one in front of it, the same diff
  costs two changed messages and nothing else. **A correctness fix that measures badly may be
  waiting on a different one**, and the way to tell is to read what it added rather than the total.

- **A name that resolves to no class is not a type, and only _one_ of the two ways to write one was
  refused.** A class template the receiver left unbound was already declined -- judging an argument
  against the bare name rejects every scalar for not being a class and accepts every class because
  nothing relates to a name that resolves to nothing. What decides it there is the receiver, so the
  rule is written where the declaring class's template list is in hand, and a **free function's
  own** `@template` has no list to be in: only `ClassRow` carries templates, so WordPress's
  `@template T` + `@param T $value` on `wp_unslash` reached the judgement as a class called `T` and
  reported `$value takes \T` -- a sentence no reader can act on. The gate is the same question asked
  without an owner: does anything in this expectation name a class the project does not declare.
  **355 findings gone across three corpora and none added** -- WordPress 116, all `\T`; Laravel 194,
  mostly `TValue` and `array<K, V>`; Symfony 45 -- corpus unmoved, `check` unchanged. It recurses
  into a union and into a generic's arguments, which is where most of the Laravel ones live, and
  that costs one true finding: a `Traversable` whose stub documents its elements with templates
  declines as a whole where the bare hint beside it would have been judged. A class that genuinely
  does not exist declines too, and that is `undefined-type`'s finding about the declaration rather
  than this check's about every call.

- **A conditional write's join has three operands and the third is about a path that sometimes does
  not exist.** The writes, what the name held before the construct, and what the guard that skipped
  them proved -- and `if ($c) { $this->x = a(); } else { $this->x = b(); }` skips nothing, so the
  declared `?T` was joined in and every use of the value below reported a null that cannot arrive.
  It is how a value is built lazily when the two ways of building it differ (Symfony's
  `Router::getGenerator`), and the _guarded_ shape beside it was already right for a different
  reason -- there the guard says what the skipped path holds. `BodyFacts::writes_fill_every_arm`
  answers it from the branch nodes the facts already hold: exactly two exclusive arms of one
  construct, each holding a write whose chain _ends_ at that arm. An `if` with no `else` pushes one
  arm and a write nested a level deeper is conditional again, so counting is the whole test; a
  `switch` needs its `default` read and a `match` its own totality rule, and both decline.
  `Otherwise::Impossible` is the answer, and it is the _type's_ own vocabulary rather than a flag --
  what the skipped path satisfies, when there is no such path. Measured: 28 findings gone across
  three corpora and five added, two of which are the same site reported in better words (a
  `possibly-null-argument` becoming the `incompatible-argument` it always was, once the null it
  could not have was gone). The three genuinely new are one shape -- `str_replace` on a `mixed`
  subject answers `string|array`, which a `mixed` operand in the union used to swallow -- and
  PHPStan reports every one of them.

- **And then the guard's complement, which was waiting on it.** `Otherwise` is the guard that let
  control into the write's branch, read backwards, and the search for it finds whatever narrows the
  name at the write -- an _enclosing_ guard included.
  `if (!is_array($t)) { if (...) { $t = str_replace(...); } trim($t); }` complements `!is_array` and
  says the skipped path held an array, which is the one thing the branch both lines are in rules
  out. The gate is one comparison: the guard must end at or after `Reach::Possible`'s `since`, which
  is the start of the outermost branch the reader is _not_ in, so a guard ending before that
  encloses both. Five findings gone on WordPress and two whose message changed -- one of them to a
  _stronger_ claim that is still wrong, because a leaving `isset` guard was silencing a lazily
  filled property by the same unsound complement.

- **And the write whose guard says nothing was throwing away the writes beside it.** The third
  operand is what the path that skipped _every_ write satisfies, so it satisfies whatever skipping
  any one of them requires -- a claim true of the wider set is true of the intersection. Read as
  needing every write to _agree_, one silent write made the whole join answer nothing.
  `if ($c) { $v = new Fmt(); } else { if (!isset($v)) { $v = new Fmt(); } }` is the shape: skipping
  the first means taking the other arm, which says nothing, and skipping the second means `isset`
  was true, which is the whole answer. **51 findings gone and one restored to the weaker wording it
  had before** -- 21 of them `possibly-null-receiver` on one Symfony method, which writes the same
  shape with `instanceof`. What is left unsound and is not worth a flow model: skipping an `elseif`
  arm's write is read as its guard being false, and taking a _sibling_ arm is the other way to skip
  it -- both give non-null in the measured cases, and the derivation is luck rather than a rule.

- **A closure can run as a class it is not written in, and `Closure::bind` is how a framework says
  so.** The visibility check reads the enclosing _declaration_, which is right everywhere but the
  one construct PHP has for reaching into another object on purpose:
  `Closure::bind($fn, null, CacheItem::class)` gives the closure that class's scope, and inside it
  the item's protected fields are as reachable as they are in its own methods. Symfony's Cache
  component is built on it -- every adapter makes its items that way -- so **55
  `inaccessible-member` findings were on code PHP runs**. Read from the closure _outward_: a reader
  has an offset, and the question is which class the code there counts as, so every closure the
  offset is in is asked innermost first (a closure written inside a bound one inherits the binding).
  Two details are load-bearing. The scope must be _written_ as a class --
  `Closure::bind($fn, $object)` takes it from the object and `$this::class` says it dynamically --
  and where it is not, the answer must fall back to the enclosing class rather than to nothing:
  answering nothing makes every access public and reports the class reaching into _itself_, which is
  two findings this added before the fallback went in. And `enclosing_call` walks every expression
  in the file, which is affordable only because the check asks it after an access has already
  resolved and been found unreachable.

- **A destructuring target writes through no variable, so every name it binds had no type at all.**
  `[$startTime, $endTime] = [Carbon::parse(..), Carbon::parse(..)]` -- and the 922 other
  destructurings in Symfony's `src/` alone, 335 in Laravel's, 32 in one real project's own modules.
  `bound_by` answers what a target writes _through_, which for `$rows['a'] = 1` is `$rows`; a
  literal target writes through nothing, so the write was recorded for nobody and the name kept
  whatever the parameter had said, if anything. What settles each name is the slot it took. The slot
  has to come from _lowering_, and that is the half worth remembering. A positional element says
  which slot it is only by counting -- and a hole counts (`list($a, , $c)` binds `$c` to index 2)
  while lowering _drops_ it, so a reader counting the nodes it can see gets it wrong. Nor can the
  body layer mint the key: `Name` has no constructor outside the interner, deliberately. So a
  destructuring target's elements carry their index the way a keyed element carries its key --
  `destructuring_elements`, one wrapper node per element and only for a target, because keying every
  element of every _value_ literal is the memory `ArrayElementKind::LiteralKey` exists to avoid.
  Reading a literal right-hand side needs the _element_, not the index.
  `[$name, $count] = [$row->name(), 0]` read as `<literal>[1]` answers the literal's element type,
  which is the union of everything in it -- wider than the truth, and a union carrying an
  alternative the parameter refuses reports the call that passes the one that fits.
  `slot_expression` walks the literal instead and answers the exact element, declining where PHP
  re-numbers (a spread, a computed key, a written key beside bare ones). **And the `foreach` half is
  refused, measured rather than assumed.** `foreach ($rows as [$row, $label])` reads a slot of the
  subject's _element_, and a literal of rows has one element type for all of them -- so a table of
  test cases gives every name the union of every row. Over four corpora it moved one finding each
  way on Symfony and nothing anywhere else, against two reports on one project's own tests. What it
  wants is the travelling array shape. What it _did_ surface is the note below, and one that is
  still open: a literal reaching a destructuring through a variable, where only its element union
  survives -- the travelling shape again.

- **An element write is the element type only where the container started here, and that was
  answering for arrays filled elsewhere.** `$rows = []; $rows[] = $user;` is the shape `elements.rs`
  was written for, and there the writes really are every word about what the array holds.
  `$calls = $value->getMethodCalls(); $calls[] = [$constructor, $args];` is the same syntax and the
  opposite fact -- the push says what was appended and nothing about what the call had already put
  in -- so reading it as _the_ element type states a type narrower than the truth, which is how a
  checker reports correct code. Two sources say the container was filled elsewhere and neither is a
  type question: an assignment of anything but a literal, and the parameter list, which is the one
  source no statement in the scope shows. **83 findings gone across three corpora and none added**,
  for six receivers that no longer resolve -- and they were resolving to a type the container need
  not have had. WordPress's exporter is the clearest: `$authors[] = get_userdata(..)` pushes a
  `WP_User|false`, `$authors = array_filter($authors)` takes the falses out, and six lines were
  reported for a `false` that cannot reach them. It was invisible until the destructuring work gave
  those arrays a second reader: a wrong answer nobody asked for costs nothing until somebody asks.

- **Two things an outside reading of 82 new findings caught that measurement did not.** A reviewer
  asked to classify what the destructuring work added found 39 true and 43 false, and the useful
  half of the disagreement is what the false ones _were_. Nine were the element-write rule above.
  Sixteen were a declaration this project deliberately believes -- WordPress declares
  `@return array<int, array|string>` and its own code then assumes the array, which is
  `?T where T expected` one step sideways and is what §5 refuses to suppress. And two were bugs of
  the new work: `[$parts[2], $parts[3]] = $pair` writes two _slots_ of `$parts` and says nothing
  about `$parts`, which was read as that name's value and retyped the array as one of its own
  elements; and `is_numeric($x)` names no `BuiltinTy`, so the predicate reader had nothing to say
  about a test that plainly rules null out -- `is_scalar` and `is_countable` are the same shape, and
  `strictly`, because `is_numeric('0')` is true and `'0'` is falsy. The lesson about the
  _instrument_ is the one to keep: a reviewer judging by what the code does at runtime will call a
  declaration-honest finding false, and this project judges by the declaration. Read the causes it
  groups rather than the counts it totals.

- **`iterable` and `callable` are not families of their own, and a filter written as equality
  dropped what answers them.** An array _is_ iterable, so `is_iterable($x)` on an
  `array<int, string>|string` keeps the array and drops the string -- read as `kind == Iterable` it
  dropped both, nothing survived, the guard became the whole answer and the element type went with
  it: the branch that plainly knows what it is iterating typed it `iterable`. The relation needs
  three answers, not two, because an object _may_ be iterable or callable and may not (a
  `Traversable`, an `__invoke`) and a string may name a function -- so those survive **both**
  branches. Dropping an object from the `!is_callable` side would claim every object callable, which
  is this bug mirrored. Nearly nothing on the corpora (−3/+2) and the reason is worth knowing: it is
  a _precision_ fix, and precision shows in hover and in what the next step resolves rather than in
  a count. One of the two arrivals was a thread worth pulling, and it is the note below.

- **`element_written` took every write before the offset, in any branch.** Every other reader of a
  write asks `facts.reach_of` first -- a write in a branch the position is not in did not run, which
  is the whole of what `writes_in_a_branch` pins for a local -- and this walk did not ask at all. So
  an `if`/`elseif` chain that rewrites `$callback` in one arm answered with that arm's value in the
  arm beside it. Nine findings on two corpora for one comparison, and it was found by reading the
  _arrivals_ of an unrelated precision fix rather than by looking for it. One arrival of its own: a
  container written in several branches has no single answer, and the last reaching write is a
  different guess from the last written one rather than a better-founded one.

- **`@return MockObject&T` is what every mock a PHPUnit test makes is written as, and both readers
  of a template declined it.** The value is two things at once and a test uses both -- the mocked
  class's methods and PHPUnit's own -- so reading either half alone reports the other: **446
  findings on one framework's tests**, `undefined-method` and `undefined-property` together. Two
  readers, because a template is bound two ways and §8 already records that as a form: lowering
  reads one bound by a `class-string<T>` **argument** and matched a bare name, so an intersection
  built no contract at all; the substitution that reads one bound by the **receiver** walked a union
  operand by operand and declined an intersection, which is the same walk with the other fold. Both
  operands of `MockObject&T` are written as names, so nothing in the syntax says which is the
  template -- ask the parameter list about each, and the real class answers nothing. Nothing is
  interned to say it. `Held::AllOf` carries the two claims where the type table, open for reading
  only, cannot -- and the _other_ operand is the declaration's own return type as well, so what has
  to travel from lowering is one bit: an intersection was written. That bit is load-bearing for a
  second reason, and it was measured: composing the hint for _every_ factory is two hierarchy walks
  per call and cost **15 ms of a 180 ms sweep** for an answer almost no declaration wants.

- **A namespace is declared by nothing of its own, and the import check asked only about classes.**
  `use Psl\Crypto\Internal;` beside `Internal\call_sodium(..)` is how a library written as
  namespaced _functions_ spells a module, and `is_namespace` scanned class-like names alone -- so
  every such import read as a class that does not exist. **211 findings on one library and none on
  four framework corpora**, because a framework writes `use function` instead. It took a fifth
  dialect to see, which is the note about corpora arriving again: `azjezz/psl` is that dialect (PHP
  8.4, `packages/*/src`, property hooks in interfaces, generics everywhere) and it is worth keeping.
  What the same reading also settled, and is worth writing down so it is not re-derived: `check` on
  the five older corpora has **run out** as a bug instrument. Symfony's 2 257
  `possibly-null-receiver` are a position twice over -- 1 642 in tests calling a `?T` return they
  know is there, and the largest non-test cluster a nullable promoted property a constructor always
  fills (`AbstractHttpTransport`, `AbstractRecursivePass`), which PHPStan reports too. WordPress's
  `incompatible-argument` is its `int|WP_Error` idiom unchecked. Laravel's remaining
  `undefined-method` is the interface-call position §8 already measured and refused. What is left
  there is not ours.

- **`use ($x)` and `use (&$x)` were one shape in the tree, and the `&` is a _write_.** Lowering
  dropped the ampersand, so nothing above it could tell a capture that reads from one that writes --
  and a name a closure writes is a name the visible assignments do not describe: `$found = NULL;`
  beside `use (&$found) { $found = $row; }` read as **definitely null**, the strongest claim there
  is. 138 findings on three corpora, `null-receiver` and `read-before-assignment` alike. A
  by-reference capture is an `ExprData::Reference` now, which is how every other by-reference target
  is written, and two readers of the capture list had to learn to look through it -- miss either and
  the name stops being bound _inside_ the closure, so every write in there reads as a local nothing
  uses. The rule is **captured _and written_**, not captured:
  `$walk = function () use (&$walk) {...}` is how a closure calls itself and only reads the name,
  and refusing that would take the callable from every recursive closure in PHP. What it still costs
  is four reports in one Laravel method where the declaration is wider than the code (`string|array`
  narrowed by an assignment the capture now discards); the answer for that is to join the closure's
  own writes rather than to decline, and it needs the closure's body as a source of writes for the
  enclosing scope.

- **The class direction of a guard had one complement and the other two directions had both.**
  `Otherwise` is the guard that let control into a write's branch, read backwards, and it answered
  for `if (!$x instanceof T) { $x = ...; }` and not for the same guard the other way round. So
  `if ($month instanceof Month) { $month = $month->value; }` -- the commonest normalising shape in a
  typed library -- put the class back beside the value the branch had just replaced it with, and the
  call below reported `takes int, and this is int|Month`. Fifteen positions gone across three
  corpora and 53 messages tightened; four arrived, and they are the modelling limit worth naming:
  **a write nested inside several guards has one skipped path and it satisfies the complement of
  every one of them**, while `Otherwise` carries one -- the innermost. And a measurement that
  decided against the obvious next step. Following a construct _nested inside_ an arm in
  `writes_fill_every_arm` is right -- `if (!$file instanceof File) { if (..) {..} else {..} }`
  writes on every path through the outer arm -- and it needs `exclusive_arms`, a scan of every
  branch node in the file, once per nested construct per lookup. `check` went **180 ms to 350-520**
  for **one** finding across six corpora. Refused, and the test says so where the third operand
  stands.

- **A leaving guard nested one branch deep is invisible to the code after that branch, and the
  frequency measurement was the wrong reason to leave it.** Fixed in `proved_by_both_ways`: an `if`
  whose arm comes back out proves what _both_ ways out of it prove, which is one level of
  `proved_by` applied to the arm's own statements against the condition's complement. Kept as a
  record because the refusal below was reasoned from a count -- one site in four projects -- and a
  false positive on code that plainly cannot be null is a bug whatever its frequency. What the count
  _did_ correctly say is how much to build: only about null, depth 3, and a write standing after the
  claim withdraws it.
  `if (!($app = $m->loadActive($id))) { if (!($app = $m->loadByUserId($id))) { return; } } $app->m();`
  reports `possibly-null-receiver`: every path reaching the call has a non-null `$app` -- the outer
  condition being false says so, and so does the inner leaving guard -- but
  `visit_preceding_statements` does not descend into a _completed_ branch, correctly in general, so
  the inner guard cannot be seen from below. The join over the writes is what would have to carry
  it: the operand for the skipped path is already non-null, and what is missing is the second
  write's own value narrowed by the guard that stands between it and the end of _its_ branch.
  Written with explicit `=== NULL` checks instead of assignments in the conditions, the same code
  reports the same way -- so it is the nesting and not the spelling. Refused on frequency: **one
  site in four projects** (35 000 files), the one that prompted this. The machinery it needs is a
  third flow mechanism -- a per-write guard search inside the write's own branch, paid per lookup --
  and it removes nulls, which is the direction §5 protects. If it recurs, the shape to reach for is
  an operand wrapped in `PathRoot::NotNull` at the branch's end rather than a wider walk.

- **A renderer is judged by a person reading it until something _writes_ its output into a file, and
  then a spelling it cannot parse loses the whole type in silence.** `TypeRef::Field` printed a
  shape key bare, which is right for every key that is an identifier and wrong for the ones a
  framework actually uses: `array{#type: string}` unquoted does not parse at all, so mago hands back
  a plain `array` and every key the shape named is gone with no error anywhere -- and Drupal's
  render arrays are nothing but `#`-keyed. Harmless for as long as the only reader was a hover card,
  a bug the moment the docblock generator wrote one into a file. What found it was round-tripping
  the exact text about to be generated (`phrost query` over a file holding it), which is the check
  to run _before_ wiring up anything that writes a type: **can this index read back what it is about
  to write.** And the rule belongs in the renderer, not in the writer -- a second writer is how one
  of them comes to be missing the quoting the other has.

- **A union keyed by a name looked through no sentinel, and `reset()` is where everybody meets it.**
  `array{date: string}|false` is what `reset()` answers for any array PHP cannot prove non-empty,
  and `keyed_of_held` required _every_ operand to name the key -- so the shape had no keys at all. A
  `false` has no keys by construction, and requiring it to name one is requiring the impossible; the
  branch where it actually arrives is what `possibly-false-return` is for. The `Nullable` arm three
  lines above had looked through a null since it was written, which is the tell: when one spelling
  of absence is looked through and the other is not, the second was forgotten rather than decided.

- **`doc-missing-throws` reported a throw the body catches on the spot, for as long as it existed.**
  A docblock says what escapes a declaration, and `try { throw new X(); } catch (X $e) {}` escapes
  nothing -- so the `@throws X` it demanded would have documented a thing that cannot happen. The
  rule had no notion of a `catch` at all: `thrown_classes` walked `throw_sites` and resolved each,
  and nothing between the walk and the report ever looked at what surrounded one. Small on the
  corpora (154 -> 152 WordPress, 49 -> 48 Laravel, 1238 -> 1236 Symfony, 0 on the reference project,
  whose code documents every throw it makes) and load-bearing anyway: wrapping a throwing call in a
  handler is _how_ PHP code deals with one, so the rate goes up with every step toward reading what
  a callee throws rather than only what this body writes. Two things worth keeping from the shape of
  the fix. The guarding span is the `try` block's own statements and not `StmtData::Try`'s range,
  which covers the clauses too -- a throw in a `catch` body is not caught by the clause it is
  written in, and one in `finally` is caught by nothing; spanning the statements makes a containment
  test exact instead of approximate, and nesting falls out of it for free. And coverage is asked
  through `throw_documented`, unchanged: "does this `catch` cover this throw" and "does this
  `@throws` cover this throw" are the same question about the same hierarchy, and a second walk
  written here would have been §8.1's oldest form waiting to disagree with the first.

- **`doc-missing-throws` read only what a body _writes_, and the reason it gave answered a different
  question than the one a person asks.** The doc-comment said it plainly -- "a callee's own throws
  are its docblock's business, and reading them here would document the whole call tree" -- which is
  true of a callee's _body_ and false of its _declaration_. So `new DateTime($input)` with no
  handler and no tag reported nothing, and neither did a call to a method documented
  `@throws \RuntimeException` three lines up. Measured before building anything: a probe with three
  methods, one throwing directly and two through calls, reported exactly the one. What a callee
  **declares**, never what its body does, so the chain is one level deep by construction and a
  method that documents nothing propagates nothing -- the way PhpStorm and PHPStan read the tag.
  Both halves of "what does this declaration throw" read the same body, which is the part worth
  copying rather than the rule: closures are out of both (`call_sites_in` beside `throw_sites`), a
  surrounding `catch` covers a call as it covers a throw, and one class is one finding however many
  calls raise it. **The cost was the design, and the index already said where it would go.** A
  caller cannot read a callee's docblock -- the index keeps ranges and reads text on demand, which
  is one disk read per call site and 34 000 of them on one project. `Lowerer::doc_throws` had
  carried a note for a year saying this would need "a sparse table beside the declarations, the way
  `ScopedAttributeLiteral` is kept off the symbols"; that is what it got. Lowering already allocated
  the name references for goto, so the only new work is keeping the ids. Priced on the reference
  project: **0.5 MB** of an 89 MB index, `check` 221 ms -> 250 ms, diagnostics p95 9.65 ms against a
  30 ms budget. Two things that were nearly wrong. A free function's row must be keyed by its
  **FQN** and a method's by its own short name plus its declaring class, because that is what
  resolving a call answers with -- keying both by `name` looked right and made every namespaced
  function's tag unfindable, and only the dump showed it. And `member_location` resolves to the
  _declaring_ class, so a tag written on an interface is found through a call on the implementation
  for free. Volume, since this is a rule that can only add findings: the reference project 0 -> 2
  655, of which 1 841 are in `tests/` and 1 513 name a PHPUnit class -- `assertSame()` is documented
  as throwing `ExpectationFailedException` and that is how an assertion works. 814 outside tests,
  led by `InvalidQueryException`, `PluginNotFoundException` and `MissingDataException`, which are
  real. WordPress 152 -> 666, Laravel 48 -> 2 122, Symfony 1 236 -> 3 141. A `checked-exceptions`
  list or a rule about test methods is the lever if that is too much; neither is built.

- **A `@throws` written on an interface did not document the class that implements it, and only one
  of the two spellings of "see above" was covered.** `{@inheritdoc}` ends both docblock rules --
  `documented()` returns `None` for it and the declaration is skipped -- but an implementation that
  writes its _own_ prose summary and no tag inherited nothing, so every implementation of every
  documented interface was reported for the contract its interface had already stated. Two lines of
  probe found it: an interface declaring `@throws \RuntimeException`, a class implementing it and
  throwing exactly that, reported. A contract is written where it is _declared_, and PhpStorm,
  PHPStan and psalm all read a parent's tag that way. `inherited_throws` walks the owner's
  contributors, skips the owner itself -- what it wrote is read from its own docblock, where the
  text is -- and reads the same sparse table a call site reads, so the two questions "what does this
  call throw" and "what does this declaration already promise" come off one stored fact. **The
  instrument lesson is the zero.** Corpora moved -- Symfony 3 141 -> 2 987, Laravel 2 122 -> 2 021,
  WordPress 666 -> 656 -- and the reference project moved by **nothing**, which §8 warns is usually
  a second blocker. It is not one here: Drupal's house style writes `{@inheritdoc}` on every
  implementation, so that project was already covered by the arm that existed, and the gap lives
  exactly where a project writes prose instead. Checking _which_ spelling a project uses is what
  turned a suspicious zero into an explained one, and it took one file.

- **A rule can be right and be asked of the wrong subject, and 69 % of its findings were that.**
  Once `doc-missing-throws` read what a _call_ declares, it reported every test method of a project
  whose standard puts a docblock on every method: **1 843 of 2 684 findings on the reference
  checkout, 1 513 of them naming a PHPUnit class**. All true by the rule and all useless --
  `Assert::assertSame()` is documented `@throws ExpectationFailedException` because that exception
  _is_ how a failed assertion becomes a failed test, and a test method has no caller in the project
  to be told about it. PHPUnit invokes it by reflection and catches every `Throwable`. The fix is
  one fact behind the framework seam and deliberately **not** a list of exception classes: which
  exceptions a project treats as part of a contract is a policy it has to choose, and PHPStan
  (`checkedExceptions`) and PhpStorm both make it configuration. A framework naming the base class
  it _runs_ is a fact instead. `FrameworkProvider::runs_and_catches` sits beside
  `discovered_supertypes`, which answers the neighbouring question -- does the framework _build_
  this -- and the two share one hierarchy climb (`inherits_any_of`) while keeping separate lists,
  because what they mean differs and the walk does not. Reference project 2 684 -> 840, Laravel 2
  021 -> 1 857; WordPress and Symfony unmoved because neither corpus has PHPUnit installed at its
  root. Nothing outside a test class moved: the kept set and the old "not under `tests/`" set differ
  by three rows, all of them the class rule being more exact than a path filter. The one finding
  still in a test directory is an anonymous class extending a _production_ class -- written by the
  test, not run by the runner, and correctly kept.

- **Two checks in one file shared a guard line, and a first-occurrence edit landed on the wrong
  one.** `MissingDocTags::run` and `UndocumentedThrow::run` both open
  `if user_facing(view, &declaration) { continue; }`, so a replace anchored on that line put the new
  exemption on the _tag_ rule -- which would have stopped asking test methods for `@param` and
  `@return`, the opposite of what was wanted, while the throws findings stayed exactly where they
  were. The tell was the measurement: 2 683 findings before and 2 683 after, on a change that could
  only reduce them. §8's substring trap in edit form; anchor on a line only the target has, and when
  a change that must move a number moves nothing, read where it landed before looking for a reason.

- **"Which class of this value owns this name" lived in four places, and three of them had two of
  its four rungs.** `owner_of_member` walks a full ladder -- the operand of a union that declares
  the member, then a single class, then an intersection's declaring operand, then a union all of
  whose operands declare it -- and its first rung exists because reducing a union to a common
  ancestor answers about a class the source never wrote. The three readers in `locals/steps.rs`
  (`member_through`, the `MethodReturn` step, the `Invoked` step) each had a hand-rolled _two_-rung
  version: a single class, then an intersection. So a union whose operands share no ancestor reduced
  to nothing and the reader gave up. The symptom is the giveaway and is why this survived: the
  member **resolved**. Goto and hover answered, because those ask the full ladder -- and the _chain_
  died one hop later, because the step that carries a return forward asked a shorter one.
  `@var Thing|MockObject` is how every PHPUnit mock is declared, so
  `$mock->expects($this->any())->method('x')->willReturn($y)` stopped after `expects` in every test
  of every project. Measured against intelephense on the reference checkout, 40 files and 2 836
  sites: **goto reference-only 18 -> 7 and hover 17 -> 6**, the whole `?->method` / `?->willReturn`
  / `?->with` family gone from both. Findings: WordPress 3 920 -> 3 933, Laravel 6 906 -> 6 910,
  Symfony 8 439 -> 8 458, Slim and the reference project unmoved -- typing something new makes the
  checks that were silent about it speak. Sampled: `ReflectionType::isBuiltin()` (it is on
  `ReflectionNamedType`), `ConnectionInterface::getParameters()` and
  `ProcessorInterface::getPrefix()`, all true and all reported by PHPStan too. Two findings went
  away, both arguments that a better-typed value stopped being `mixed`. §8.1's oldest form, and the
  thing to take from it is the _tell_: when a feature works at one position and dies at the next,
  look for a second reader of the rule that answered the first -- not for a missing case in the
  rule.

- **A wrapper's type is written only in the expression it forwards, and half of that was already
  read.** `returned_from_body` has read `return new Bar();`, `return $this;` and `return null;`
  since it existed, and stopped exactly where its own doc-comment says: "a call, a variable, a
  property -- names a type only after resolution, which is a whole index away from here." True, and
  it left the commonest wrapper unread. `SqlBase::select()` in Drupal core declares no return type,
  carries no `@return`, and is entirely
  `return $this->getDatabase()->select($table, $alias, $options);` -- so a migrate source writing
  `$this->select('t')->fields(...)` resolved the first hop and nothing after it. The answer is not a
  type but a **description**: a root and the member names applied to it, stored with the declaration
  and walked into classes on demand. Small enough for a closed file to keep, which is the whole
  point -- a closed file has no statements, and this is the only thing a caller can be given.
  Measured on core: **4 181** methods declare neither a return type nor a `@return`; 505 return a
  `new` (already read), **1 235 are a chain like this**, and the remaining 2 400 hand back an array
  literal or a local, which says `array` or nothing. `$this` is stored as **no root at all**, and
  the caller supplies one. A subclass calling an inherited wrapper gets its own override -- late
  static binding, which is what a fluent base class is written for -- and storing the declaring
  class would have answered about the parent. Priced: goto reference-only against intelephense
  unchanged at 7 _by count_ but the Drupal query-builder family gone from it; coverage 49 062 -> 49
  069 resolved and untyped receivers 401 -> 394; index 89.6 -> 90.9 MB; every request budget
  unmoved. **What it costs is worth stating precisely, because it is all one thing.** WordPress +12,
  Symfony +47, Laravel +76, the reference project **0**. Every one of Laravel's 76 is in `tests/`
  and none in `src/`: 38 are `InvocationStubber::with()` -- PHPUnit's interface declares `method()`
  returning itself and puts `with()` on a _different_ interface that the concrete `InvocationMocker`
  also implements -- and 22 are `Mockery\MockInterface::<anything>`, which proxies everything and
  declares almost nothing. Typing a mock is what reveals that a mocking library's interfaces do not
  describe how it is used; PHPStan ships `phpstan-phpunit` and `phpstan-mockery` for exactly this,
  and the seam here would be a provider fact naming the classes whose members are dynamic. Not
  built.

- **`__call` is a class mechanism, and a mocking library hands the caller an interface.** PHP's own
  way of saying "this answers anything" already excuses a class from `undefined-method`, and it
  could never cover the case that needs it most: `Mockery\MockInterface` declares a handful of
  methods and proxies the rest to a generated subclass, and an interface declaring `__call` would be
  a claim about every implementer. PHPUnit's is subtler -- its fluent builder is split across two
  interfaces that one `InvocationMocker` implements together, so `method()` returns the one
  _without_ `with()` on it and `->method('x')->with(...)` is `undefined-method` by the declarations
  alone. None of it was visible while a mock receiver was untyped. Reading what a wrapper's body
  forwards typed one, and Laravel's suite gained 76 findings -- while already carrying **558 more of
  the same shape** that nothing had connected to a cause. `dynamic_members` is the third list on the
  one hierarchy climb `inherits_any_of` owns, beside "a framework builds this" and "a framework runs
  this", and the three stay apart because what they mean does. **All 634 that went away are in
  `tests/` and none in `src/`**, which is the rule's shape rather than a filter: a mock exists
  nowhere else. WordPress, Symfony, Slim and the reference project are unmoved -- none has Mockery
  installed at its root. PHPStan ships `phpstan-mockery` and `phpstan-phpunit` for exactly this,
  which is the same admission in a different form: a library whose values answer anything cannot be
  described by its own signatures.

- **The propagated-throws rule asks for something the framework it runs on does not do, and that is
  a policy rather than a defect.** Measured on the reference checkout after the rule landed: of **5
  658** Drupal core methods whose body calls `->get(`, **5** carry
  `@throws \InvalidArgumentException` -- 0.1 %. Of the project's own 598 such methods, **1**. So the
  single largest family in its report (203 of 840) is a tag that neither the framework nor the
  project has ever written, against an interface that documents the throw honestly. None of that
  makes a finding false. `FieldableEntityInterface::get()` really is declared to throw, and a caller
  really does propagate it. What the numbers say is that _which_ exceptions a project treats as part
  of a contract is a decision it has to make, which is why PHPStan calls the list
  `checkedExceptions` and ships it empty and PhpStorm ships a default. The rule as built has no such
  list, on the project owner's explicit instruction after being shown the volume. Recorded rather
  than acted on, because the lever is one option away and the choice is not this layer's: a
  `[lint] checked-exceptions = [...]` in `phrost.toml` reading into `undocumented_throws` would
  bound it by construction. The thing to _not_ do is quietly narrow the rule, which is the same
  mistake as hiding a magic method -- deciding on somebody's behalf that a true statement is not
  worth making.

- **A true finding with its evidence left out gets called spam, and it was right to be.** The report
  was `throws \InvalidArgumentException, which the docblock does not declare`, sitting on
  `return new self(...$fields);` -- a line holding no `throw` at all. It was reported as "we are
  spamming lies". It was not a lie: six lines above, the same file's `__construct` carries
  `@throws \InvalidArgumentException` and throws it on a duplicate field, and `fromArray()` really
  can raise it and really says nothing. But nothing on the screen said so, and a reader has no way
  to check a claim about a line that does not contain the thing being claimed. A propagated throw
  now names the call that declares it:
  `throws \InvalidArgumentException, which App\AttachmentTypeFields::__construct() declares and the docblock does not`.
  A written `throw new` keeps the shorter message, because the class is on the line the report
  underlines and there is nothing to explain. **The second instance was `unused-declaration`, and it
  is the one that says how to look for more.** The test is not "is the message true" but "can a
  reader run their own check and get the same answer" -- and a reader's check for _reached from
  nowhere_ is a grep. The message is decided by two rules that read identically: the coarse one
  means the word is written nowhere, which the grep confirms, and the exact one means no call
  _resolves_ here, which the grep flatly contradicts. 45 of 123 findings on the reference project
  are names something else in the project really does call -- 64 classes declare a `toArray` -- so a
  third of the check's output was a correct statement no reader could distinguish from a wrong one.
  The clause is the fix; what generalises is the question. When one message can be produced by two
  rules, either it names which one answered or the weaker reading is the one people act on. **The
  third instance arrived an hour later and from the same reader, as a one-word question:**
  `method AttachmentTypeInterface::buildJsonSchema is reached from nowhere` -- _on an interface?_
  True, and the most interesting finding the check has produced: the contract is declared once,
  fulfilled twice, called nowhere, and the only `buildJsonSchema` call in 30 000 files is
  `$this->buildJsonSchema($type, $stored)` in a third class that has a `protected` namesake taking
  two arguments. Every grep says the method is busy. What the reader cannot see is that the
  implementations are silent only because `member_comes_from_above` exempts them from being reported
  twice -- so the message now speaks for them: _nor is any implementation of it_. That is provable
  rather than asserted, and the proof is `reaches`: an access is a claim on every declaration in the
  receiver's hierarchy that it satisfies, so a call on any implementation reaches the contract too,
  and being reported at all means no such call exists. Two things worth carrying past this case. The
  upward half of that rule -- a call on the concrete class keeping the interface's declaration alive
  -- was the thing the whole clause rests on and had **no test**, though the module comment named it
  as a real false positive. And the first fixture written for it proved the _downward_ direction
  while carrying the upward direction's name, because the call's receiver was typed as the
  interface: §8.4's trap, hit while writing coverage for a note about messages that cannot be
  checked. Then the clause itself lied by implicature. `, though the name is written elsewhere` is
  true of every contract by construction -- the implementation's own declaration writes the name --
  so beside a contract it implies a rival call that three of four real cases did not have. The two
  clauses are exclusive now. A clause that is true of every member of a class of findings carries no
  information about any of them, and reads as evidence anyway. Worth separating from the other
  complaint in the same family, which the numbers say is real: 203 of that project's 840 findings
  are `InvalidArgumentException` reaching them through `FieldableEntityInterface::get()`, a tag
  Drupal core honours **5 times in 5 658 opportunities**. That one is a policy about which
  exceptions count, and it is still unbuilt. This one was a message that could not be checked, and
  the fix is a sentence.

- **PHPStan at the user's own level is an oracle, and reading its JSON's `file_errors` without its
  `errors` makes a crash look like a clean bill.** The reference project reports **0** at its
  configured level 5. Raised to level 8 it reports 2 802, and the first two runs of that also said
  zero -- `totals: {errors: 1, file_errors: 0}`, where the 1 was
  `PHPStan process crashed because it reached configured PHP memory limit: 128M`.
  `--memory-limit=6G` and a planted `$probe = 1; echo $probe->notAMethod();` are the control;
  without the control, "0 findings" and "analysed nothing" are the same number. What the 2 802 are
  worth reading for is the disagreements, and they went four ways. **2 218 are docblock-completeness
  rules** (`missingType.*`), which is a different question from type safety. **97 of the 126
  `method.nonObject` are one shape** -- `->fetch*()` on `StatementInterface|null` -- and phrost is
  silent on all of them by a _deliberate_ framework contract, re-verified against the installed core
  while reading this: `Select::preExecute()` returns `TRUE` early when prepared, sets
  `$this->prepared = TRUE` and returns it, so `execute()`'s `if (!$this->preExecute()) return NULL;`
  is unreachable. **Several more are PHPStan being conservative where phrost narrows better** -- a
  property assigned `new X()` two lines above and invalidated by an intervening `$this->` call, a
  `$field->count() ? $field[0]->getValue() : []` whose guard PHPStan will not connect to the offset.
  And **the residue has one root**: `$node->parentNode->removeChild($node)` with no guard, and
  `$file = $value['infoArray']['file'] ?? NULL; if ($file) { $file->getMimeType(); }` where the
  union still holds an `int`. Neither is a null-check bug -- phrost is silent because the _value has
  no type_, and it has no type because it was read out of an array nobody declared a shape for. That
  is what `Held::Record` was built for, and the honest note beside it is that closing it moved
  coverage not at all: coverage counts member accesses, and most array slots hold scalars. The rule
  worth keeping: when a mature checker disagrees with this one on real code, sort the disagreements
  before counting them. Four of the five kinds here were phrost being right, and the fifth named the
  next feature.

- **A rule that hands a case to another rule has to be checked against that other rule existing.**
  `NonArraySubscript` steps aside for a possible null, and says why in its own doc comment: the
  value is "a null nobody handled, not a type that cannot be subscripted, and saying both is two
  reports for one mistake." Correct, and the check it hands the case to walks **member accesses**,
  so `$row->vin()` on a `?array` was reported for years and `$row['vin']` was reported by nobody. 96
  findings on the reference project that PHPStan reports and phrost did not. What makes this worth a
  note rather than an ordinary miss is that it was _tested_. A test named
  `a_nullable_array_belongs_to_the_null_check` asserted silence, which is right about the first half
  of its own name and says nothing about the second -- and an `expect![""]` accepted for a deferral
  is indistinguishable from one accepted for a rule. §8.4 already says an empty expectation is the
  one snapshot that can be right for the wrong reason; this is the form it takes when the reason is
  a handover. **When a comment says a case belongs elsewhere, name the rule it belongs to and check
  that rule fires on it** -- the test for the deferral is a test for the pair, not for one half.
  Found by sorting PHPStan's level 8 output rather than by reading code: 96 of its `offsetAccess`
  findings looked like an optional-key rule and were all `...|null` at the end of a long shape, past
  where a classifier written in a hurry was looking. Sorting the output is what turned "a rule we do
  not have" into "a rule we have, for one spelling".

- **Two things a borrowed corpus asked for and the answer was no, with the reason each time.** Worth
  writing down because both look like obvious wins and the cost is invisible from the fixture. **A
  container built by element writes** -- `$rows = []; $rows[] = new Item();` -- renders as bare
  `array` rather than `Item[]`. The fixture wants `array<int, int>`. But the _element_ already
  reaches everywhere it matters: through `foreach`, through `$rows[0]`, through `$rows['a']`, all
  three measured. What is missing is the container's rendered type, and buying it costs a new body
  query, a path root that keeps the empty literal's node, and the same mutation hole this session
  hit twice -- a type derived from the writes this file can see is wrong the moment a callee takes
  the array by reference. Cosmetic gain, real risk. **`union_of_held` collapsing to nothing when one
  operand names no class** is the other, and there the all-or-nothing is load-bearing. Three
  callers: completion wants the classes among the operands, and the other two want the classes _of_
  the union. `class_of.rs` uses it for "every operand declares this member, so either is a true
  answer" -- drop the `string` from `string|Alpha` there and goto, hover and every check treat the
  value as an `Alpha` when it may be a string. Splitting it into two questions to win completion on
  one rare shape is a bad trade, and the two would drift the way §8.1 says they drift. The general
  point: a fixture states a _want_, and the instrument's own documentation says a failure may be a
  feature not built. It may also be a feature refused, and the refusal is worth the same sentence
  the build would have been.

- **A guard proves things about every value it was read _through_, and one of those values was
  `$this` -- which took the check thread's stack down.** `isset($row['vin'])` cannot be true if
  `$row` is null, so the guard rules the container out as much as the element; without that rule,
  `isset($row['amount']) ? f($row['amount']) : NULL` reported `possibly-null-subscript` on a branch
  the author wrote precisely to make the read safe -- fourteen sites on the reference project, seven
  of them one method repeated. `??` is the same construct in another spelling:
  `is_array($row['k'] ?? NULL)` can only hold if the value came from the left, because the right
  operand is the null. What the first cut got wrong is that `$this->rows['lat']` is reached through
  `$this->rows` **and through `$this`**, and `covers_slot` said yes to both. A claim about `$this`
  is not merely worthless -- PHP guarantees it is an object -- it does not terminate:
  `narrowed_this` resolves what it finds with a fresh `FOLLOW_LIMIT` rather than an inherited
  budget, so a claim about `$this` asks for `$this`, which asks the guards, which answer with the
  claim. Two of them in one condition (`isset($this->loc['lat'], $this->loc['lon'])`, which is
  ordinary WordPress) and the sweep aborts with `has overflowed its stack` -- exit -6, no backtrace,
  nothing to restart from. Three things are worth carrying. The **budget reset in `narrowed_this` is
  still there**, and the `$this` exclusion is the only thing keeping it unreachable: the next rule
  that proves anything about `$this` meets it again. A **stack overflow cannot be bisected with a
  debugger here** -- `lldb` could not attach -- and the cheapest instrument was six lines of
  thread-local depth counter in `resolve_path` panicking past 300, which named the cycle
  (`narrowed_this` -> `resolve_path(This)` -> `narrowed_this`) on the first run. And the corpora
  found it where the reference project could not: the reproduction is a _property_ holding a
  nullable array with two keys tested at once, which an application writes rarely and a framework
  writes constantly.

- **Typing something for the first time is measured as a regression, and the finding it produces
  belongs to whatever gap it reached.** The rule above took Symfony's `incompatible-argument` from
  942 to 948, every one of the six in two DSN parsers, and none of them is a wrong claim: the guard
  types `$query['host']` as `string|array<...>` where nothing typed it before, and the next line --
  `if (!\is_array($hosts = $query['host'])) { throw; }` -- does not remove the `string` operand.
  That gap is real, separate and pre-existing; it was simply unreachable while the value had no
  type. The same shape ruled `!empty(...)` out of the rule for now: it is as true as `isset` and it
  is written over exactly the code a loose codebase leaves untyped, so switching it on surfaced six
  more on WordPress -- two of them plainly true (`get_page_template_slug($object)` handed a
  `WP_Term`), one of them a `mixed|false` union that should have folded to `mixed` before any check
  saw it. Worth stating as a rule: when a precision change moves a ratchet, the question is not
  whether the new findings are wanted but **which gap each one reached**, and a finding that names a
  second defect is a reason to write that defect down rather than to withdraw the first fix.

## A docblock shape may be open, so "this key is not declared" is not a finding

`array{id: int, ...}` is a shape that states _some_ of what is there, and a key this layer cannot
read -- a class constant as a key, a value whose type fails to lower -- is skipped rather than
failing the shape, which leaves the same partial answer with no `...` written. So a check that
reports a key the shape does not hold argues with every open shape in a project.

What separates a mistake from an absence is what separates it for a catalog key: **a typo nearly
hits**. `misspelt-key` reports only a key one edit from a declared one, with the catalog's own
Damerau-Levenshtein and its rule that two equally close keys say nothing. The floor is four
characters rather than the catalog's five, because a shape's keys are short by nature (`id`, `name`,
`#title`) where a service id is a dotted path, and the set compared against is a handful of fields
rather than a thousand registered ids.

Measured: zero findings on five corpora and the reference project, which is the expected result for
a prospective check -- it fires while somebody is typing the mistake.

## One field carrying two questions answers neither for a trait

`@use Holder<Widget>` resolved to nothing while the identical `@extends Base<Widget>` answered, and
the divergence is the tell. `member_held_on` read one name, `owner`, for two different questions:
what `self` means inside the member, and whose `@template` a name in it refers to. For a class they
are the same name. For a **trait** they part company -- PHP composes a trait's member as if it had
been written in the composing class, so `self` is that class and the member has no declaring class
of its own, while the `@template` tag is written on the trait.

The fix is one line and the shape is the recurring one: when a field is read for two questions, find
the case where the two answers differ. The substitution walk already followed `header.uses`, so
nothing else had to change.

Its neighbour, found in the same hour: the supertype-argument reader's prefilter looked for `@use`
**with** its `@` where it looks for `extends` **without** one. `@template-use`, which is the
spelling Psalm's own documentation uses, holds neither -- so the tag was invisible with nothing to
notice. A prefilter written per-tag rather than per-rule is where a spelling goes missing.

## A member missing from every operand of an intersection is not reportable yet

`$x->missing()` on a `A&B` parameter is silent: an intersection reduces to no single class, so the
access answers `UnknownReceiver` and the check steps aside. `best_across` already asks the question
of every operand of a _union_, and pointing it at an intersection's operands makes the probe report
correctly.

Measured, and reverted: **five new findings on Symfony, all false**, and all the same shape --
`$cache = $this->getMockBuilder(TestPool::class)->onlyMethods([...])->getMock();` followed by
`$cache->get(...)`, where `get` comes from a trait `TestPool` composes. The value resolves to a bare
`MockObject` with the real class lost on the way, so whatever the walk finds is not the class the
test is driving.

Two narrower gates were tried and neither separates the two cases: "only where a class was already
named" refuses the real intersection (which names none, that being the point), and "only where none
was" admits the mock. What would separate them is knowing the intersection came from a _declaration_
rather than from an inferred return, which nothing carries -- and the honest version of this is the
mock boundary itself, recorded in `docs/03-roadmap.md` as the larger of the two problems.

## An enum's own cases settle one entry on the constant reader's refusal list

`constant_value_type` answers only for shapes the syntax alone settles, and "a constant naming
another constant" is refused there with a good reason: what it holds takes resolving a name, which
is the def-map's job. One case of it needs no name resolved, and it is the one people write --
`const Default = self::Draft;` inside an enum, where `Draft` is one of that enum's own cases. The
answer is `self`, and the cases are in the same declaration.

Two things the fix needs. The cases are gathered **before** any constant is read, because a case may
be written below a constant that names it. And only `self::`/`static::` qualify: `Other::Case` names
a real type this cannot express as `self`, and naming the wrong class is what the narrow rule exists
to prevent. `const B = self::A;` where `A` is an ordinary constant stays refused.

**The test had to move down a layer, and the reason is a harness trap worth knowing.** Written as a
diagnostics fixture -- call a method the enum does not have and expect `undefined-method` -- it
failed, and so did the control: a _direct_ `Stage::Draft` did not report either. An enum implements
`UnitEnum` or `BackedEnum`, which the in-process fixtures do not index, so every enum in one has an
incomplete hierarchy and the member check correctly declines. Nothing about enum cases can be tested
through `undefined-method` there; ask the type instead.

## An empty array literal is not an alternative in a union

`$rows = $thing->rows() ?? [];` builds a union of what the call declares and what the literal is,
and an empty literal resolves to a bare `array`. So the element type was stated on one side and
absent on the other, and iterating the union yielded **nothing at all** -- not `mixed`, nothing: one
operand answers `Row`, the other answers no element, and a union can only promise what both promise.

An empty array holds nothing, so it contributes no element and the other operand is the whole of
what can come out. The fold is in `union_of`, where the operands are already resolved -- the
constructor in `Held::union` cannot make this judgement, because telling `Row[]` from a bare `array`
means reading the type table, and a `Stored` or `Written` operand looks the same from there. Gated
on another operand _stating_ an element (`element_of_held` answering, not `holds_elements`, which is
true of a bare `array` too), so `string|array` keeps both halves.

Measured: 65 occurrences of `) ?? []` in one real project, every `foreach` over the result untyped;
coverage's untyped-receiver row 381 -> 369. It also made twenty WordPress findings appear, all of
them `$_GET['key']` passed to a `string` parameter -- a superglobal's `string|array` reaching a
check that could not see it before, and correct.

## A type read off a body is not a declaration, and an override inherits one

A method with neither a hint nor a tag takes its type from what its `return` statements agree on,
and that answer was stored in the field a docblock's would be. There was nowhere else for it and no
reader that wanted the two apart -- until one did: a method **overriding** one that declares a type
inherits that declaration, and PHP's rule is that the inherited claim is the contract while the body
is an implementation detail of it.

Drupal's `Select::execute()` is the shape. `SelectInterface::execute()` declares
`@return StatementInterface|null`, the concrete class overrides it with no docblock, and reading the
body answered the non-nullable thing it happens to hand back. `MemberRow::doc_ty_from_body` tells
the two apart now: the climb runs first for a body-read type, and an ancestor whose own type came
off _its_ body is skipped on the way, since `Extender::execute()` calling `parent::execute()` found
the parent's body and stopped one interface short.

**Measured, and the number is the lesson.** It moved _three_ findings on the reference project and
none of the 97 `method.nonObject` sites PHPStan reports there -- because those chains break earlier:
`$this->reader->select(...)` does not resolve at all, so `execute()` is never reached. A fix aimed
at a counted population can be right and still miss it, when something between the rule and the
answer is also missing. The three it did move are real, and one of them is a docblock in the project
that says `array<string, string|int|bool>` where the code reads a nested array out of it.

## A callback's parameter pairs with a container only for a free function

`array_map($cb, $rows)` hands the callback an element of each container beside it, and the fallback
that reads that pairing was applied to **any** callee. A method taking a callback beside an array is
not that shape: Laravel's `$v->sometimes('x', ['Foo', 'Bar'], fn ($i) => $i->x === 'foo')` passes
_rules_, not elements, and reading them as the callback's parameter gave `$i` a `string` and
reported the property access on it.

Gated to a free function, which is where the shape lives -- `array_map`, `array_filter`, `usort` and
their neighbours are all in the standard library. Found only because the inherited-declaration
change above stopped masking it, which is the ordinary way a second bug surfaces: it was there all
along and something else was answering first.

## A template chased up a hierarchy can land on another template, and then the bound is the answer

`FieldItemListInterface<T of FieldItemInterface> extends ListInterface<T>` passes its own `T` along,
so `ListInterface::first(): ?T` chased through `@extends` and landed on a template rather than on a
class. The filter that catches a name nothing declares then turned it into nothing, and the bound --
which is the fallback for exactly this -- was never reached, because the chase had _answered_.

Every chain after `->first()` was a dead end: `$entity->get('field_x')->first()?->getValue()`
answered no goto, no hover and no completion, on one of the commonest single-value-field idioms in
Drupal. Ten sites in one project, and the tell was that writing
`/** @var FieldItemInterface|null */` above the same line fixed it -- the written path worked and
the inferred one did not.

The bound to take is the **landing** template's, not the one the chase started from: an interface
re-declaring `@template T of FieldItemInterface` over a parent's `@template T of TypedDataInterface`
is saying something narrower, and saying it about this very slot.

## A guard that rules out the only kind a value has must not hand the value back

`narrowed_union` kept the original when no operand survived, which says the opposite of what the
guard proved: `if (!is_array($rows))` where `$rows` is an `array` left `$rows` an `array` inside the
branch, and every check then judged an unreachable line against the type the guard had excluded.
Symfony's Yaml parser writes it --
`if (!\is_array($item)) { throw new ParseException(..., $item); }` -- and the `?string` parameter
was reported as being handed an array.

`mixed` is the honest answer, being the one type every check reads as _no claim_. Which branches are
reachable is a different question and not this layer's. Two places needed it, and the second is the
one the first missed: a value that is not a union at all never reaches `narrowed_union`, so
`of_kind` answers for it directly -- and only for a _settled_ test, since an unsettled one has ruled
nothing out.

Measured together with the bound fix above: WordPress `incompatible-argument` 1054 -> 1046 and
`possibly-null-argument` 747 -> 744, with three findings appearing across Laravel and Symfony that
are all real (`HasOne::save(): Model|false`, `preg_replace_callback(): ?string`).

## A generator's `return` is not its declaration's, and its `yield` is

`returned_from_body` reads an undeclared method's `return` statements as its type, and a generator's
`return` hands its value to `Generator::getReturn()` and to nobody else. So
`function each() { yield 1; return new Row(); }` typed the call `Row`, and every member reached
through it resolved against a class the value can never be. Nothing failed -- the wrong type is a
real class with real members, so it answers.

The half that reads the `yield`s is what makes the guard something other than a subtraction: PHP has
no syntax for saying in a signature what a generator produces, so the body is the only place it is
written down. The element type is written onto whatever the declaration already named --
`\Generator`, `iterable`, `\Traversable` -- which is the one relationship `refined_type_ref` lets a
docblock win, and a declaration that named nothing gets `iterable`, the spelling that needs no name
reference invented for a name nobody wrote.

**Measured, and the honest number is small.** `yield new X()` is 26 of Symfony's 971 files that
write `yield` at all; the shape that dominates is `yield $row`, whose type takes a resolution
lowering is a whole index away from. Coverage on Symfony is unchanged to the access -- 317 975
resolved, 11 864 untyped receivers, both arms identical -- and so is the index heap at 67.1 MB, with
`distinct types` 15 628 -> 15 658. So this is the correction plus a narrow gain, and the resolved
half of it (a `yield` of a local, read where the body is in hand) is still open.

The prefilter is the one `own_arguments` states: one search of the file for the keyword, then a
per-body span scan, then the walk. Indexing 28 851 files measures 918 ms against 924 ms with the
whole reading stubbed out -- the same number, which is what a walk that runs on one body in a
hundred should cost.

### Two of one rule's answers compared by id are never equal

Found while writing the above and older than it: `returned_from_body` and `same_element` both
compared two of `returned_type`'s answers by `TypeRefId`, and every `new Row()` allocates a type
reference and a name reference of its own. So `return [new Row(), new Row()];` read as a literal
whose elements disagree, and a guard clause returning the same class twice built `Row|Row`. What has
to be compared is what was _written_ -- for a class, its path, case-insensitively, the way PHP looks
one up. `same_written_type` is that comparison, and it is deliberately not shared with
`refined_type_ref`'s `written_alike`: that one reads a finished file and this one reads a builder
mid-flight.

## The sentinel checks had four codes for null and none for `false`

`null-argument`, `possibly-null-argument`, `null-receiver`, `possibly-null-receiver`,
`null-iteration`, `null-return`, `null-subscript` -- and on the other side `false-iteration`,
`possibly-false-receiver`, `possibly-false-return` and nothing at all for an argument. Every piece
needed for it was already there: `false_reach_of` beside `null_reach_of`,
`type_admits(ty, BuiltinTy::False)` beside `type_admits_null`. What was missing was one walk asking
the second question, and the family gap is what hid it -- an absence in a list nobody reads as a
list.

Asked in the _same_ walk as the null half rather than in a twin, because the receiver check already
reads both reaches off one site for that reason. Null wins where both are possible: a value that may
be either is one mistake and not two.

**Weak mode is what makes it a report rather than a `TypeError`, and the reason to make it.** PHP
converts a `false` to `""` for a `string` parameter and to `0` for an `int` one, with no error at
all -- so `base64_encode(get_attached_file($id))` encodes the empty string and says nothing. That is
exactly why `incompatible_arguments` cannot answer here: it asks what PHP _rejects_, and in weak
mode PHP accepts this. It already reported the half PHP does reject -- a `false` reaching an `array`
or a class -- and those move to the new code, which names what is wrong with them. Severity never
reaches `Error` for a `false` however certain, the reading `false-iteration` already takes.

Measured: WordPress **+1373/+9**, Symfony **+1762/+8**, Laravel **+433/+4**, Slim **+8**, the
reference project **+58**, and nothing else moved anywhere. Twelve WordPress sites sampled at random
and all twelve genuine (`get_permalink()`, `ob_get_clean()`, `preg_split()`, `get_attached_file()`);
the twenty-one certain ones read by hand, of which the only questionable is Symfony's
`$strictValidator->isValid($value, false, true)` -- a deliberate call to an older version of a
vendor signature, guarded by `interface_exists` in a way no argument check reads.
`phrost check --jobs 1` over the reference project measures 1.25 s with and 1.25 s without.

### Two calls to one function were two slots, and a guard about one narrowed neither

`PathRoot::FunctionReturn` carried the `NameRefId` the call was written as, which is per occurrence
-- so `if (wp_get_referer()) { wp_safe_redirect(wp_get_referer()); }` compared unequal to itself and
narrowed nothing. `PathStep::MethodReturn` states this rule outright ("carried as the value rather
than as a reference to the expression, so two calls written the same way stay the same slot") and
this half had never learned it: the same shape, in the constructor that had no class to hang a
receiver on, which is every guard a procedural project writes.

Two more places said the same thing separately and only one of them had to change -- `slot_value`
refused a step-less path and so did its caller. Both now ask `ValuePath::can_be_guarded`.

The over-merge it opens is the interesting half. Only _literal_ arguments travel in the path, so
`current($rows)` and `current($totals)` carry the same empty list and compare equal -- and the first
measurement of this turned WordPress's `current( current( $this->iterations ) )` into a wrong
`incompatible-return`, because the guard on the inner call narrowed the outer one. So a call is a
slot only when the literals account for _every_ argument it wrote, which leaves the idiom this was
for (a bare call) and the calls keyed by a literal (`option('home')`), and refuses the rest. Worth
noting that `PathStep::MethodReturn` has the same over-merge and keeps it, because a receiver
usually tells two calls apart and the corpora are armed against what is left.

### The over-merge it kept, found by a third spelling of an argument

The receiver does usually tell two calls apart, and where it does not the _arguments_ were supposed
to -- except that an interpolated one was not an argument at all. Lowering recorded a quoted string,
a number, a `::class` and a constant, and skipped `"SELECT domain FROM $table"` entirely, so a step
written with one carried an empty argument list and compared equal to a step written with a
different one. WordPress:

```php
if ( $wpdb->get_var( $wpdb->prepare( 'SHOW TABLES LIKE %s', ... ) ) ) {
    return $wpdb->get_var( "SELECT domain FROM $wpdb->site ORDER BY id ASC LIMIT 1" );
}
```

Two questions to a database, one slot, and the guard on the first narrowed the `null` out of the
second. `get_var` declares `string|null` and `network_domain_check` declares `string|false`, so the
finding the conflation was hiding is real and PHPStan reports it too.

Carrying the interpolated string as a **skeleton** -- its literal text with one marker per hole --
closes it, and the shape is §8.1's _two spellings collapsing into one_ rather than a new bug: the
distinction was gone at lowering, so no care above it could recover it. What the skeleton must not
do is make the calls _mergeable_: a hole is read afresh at each occurrence, so
`every_argument_written` counts only the arguments that are fixed where they are written and
guardability is exactly where it was. Telling two calls apart and assuming two calls are one are
different questions, and only the first one wanted the new spelling.

Found by the ratchet, as a rise. Which is the direction §8.2's last note describes -- a precision
gain arrives as a finding on code that has been running for years -- and the reason to chase a
one-count rise to its cause rather than accepting it: the cause here was the opposite of a
regression, and the count alone could not say so.

### And the same gap at `[...]`

`null-subscript` and `possibly-null-subscript` had no `false` twin either, so `unpack('C', $b)[1]`,
`stat($p)['mode']` and `wp_get_attachment_image_src($id)[0]` read an offset off a boolean and
nothing said so. Asked in the same walk for the same reason, with null winning where both are
possible. What it takes from `non-array-subscript` is the certain case: `false` is no longer in that
check's list of kinds nothing can be read from, because it is what a call hands back to mean _not
there_ rather than a kind somebody declared -- exactly the reading that check already applies to a
null.

Measured: WordPress **+118**, Symfony **+53**, Laravel **+38**, and nothing on the reference project
or Slim. Eight WordPress sites sampled at random and all eight genuine. Eighteen of the 118 sit on a
line that also writes `empty()` or `isset()`, and the presence tests themselves are not sites at all
-- those are the _other_ read on the same line, which is what a ternary's else branch is.

## A member the scope cannot touch is not the member PHP calls

PHP does not refuse an out-of-scope access on a class with `__call`/`__get` -- it _forwards_ it, to
a different method returning a different thing. The visibility check already knew: the exemption for
a magic accessor is the difference between reporting an inaccessible member and reporting every
framework class with one. The type resolution did not, so it read a declaration that never runs.

Laravel's `Router` is the shape. `prefix()` is **protected** and returns a `string`; its `__call`
hands the name to a `RouteRegistrar` which returns itself. So `$router->prefix('x')->get('y', ...)`
answered `string`, and the `get()` after it was reported as a call made on one.

One rule in two places and only one of them knew, which is the form §8 names -- so the fix is the
seam rather than the catch: `reach_of_member` answers _Declared_, _Magic_ or _Refused_ once, the
visibility check takes the third and the step walker the second. The cheap exit is first and covers
nearly every access, because a public member is reached by its declaration whatever the scope and
everything below that test is a walk of the enclosing scope.

## A factory assigns and then returns, and the assignment is in the same body

`returned_type` stopped at a variable, for the good reason that a variable names a type only after
resolution -- and for the one shape that matters most it does not:

```php
private function createContainer()
{
    $container = new ContainerBuilder(new ParameterBag([...]));
    $container->registerExtension(new DebugExtension());

    return $container;
}
```

Everything about what comes back is three lines up, in the same body, in the same file. Scanning for
it: **492 methods on Symfony, 136 on Laravel, 29 on WordPress, 3 on the reference project** -- a
shape that lives in code written before return types, which is most test suites.

What makes reading it safe is that a local has a closed set of writers inside its own body, and this
refuses every one it cannot read rather than reasoning from the ones it can: a parameter is written
by the caller, and a `foreach` target, a `catch` variable, a `global`, a `static`, a `list()`
destructuring, a compound assignment, a `++` and a `= &$other` each bind the name where no `new` is
visible. Two assignments are read as one type only when they name the same one.

The one thing it cannot see is a by-reference argument -- `$this->fill($container)` may rebind the
name if the callee declared `&$container`, and which callee that is takes an index. `slot_value`
records the same unsoundness for the same reason: what is claimed is that nothing _in this body_
wrote the name again.

**Measured.** Symfony coverage 317 975 -> **318 768** resolved receivers, untyped 11 864 -> **11
067**, assignments typed 93.3 % -> **93.8 %** -- the largest single coverage move in this session by
a wide margin. `phrost check --jobs 1` over the reference project 1.25 s -> 1.20 s and indexing 28
851 files 918 ms -> 930 ms, both inside the noise.

The findings it adds are the precision gain arriving as reports, and they were read one by one.
Symfony +14: an `extra-argument` that is a **real bug in Symfony's own test**
(`hasAlias($id, 'Alias for user checker is registered')` -- the message belongs to `assertTrue`),
plus null arguments and four calls to a concrete extension's method through an `ExtensionInterface`.
Laravel +96, of which **81 are the mock boundary this document already records**:
`new Repository(Mockery::mock(Store::class))` loses its mock-ness at the constructor, so
`$repo->getStore()` is a `Store` and `->expects()` is undefined on it. The rest are `?Route`
receivers and `Response::$original`, which Laravel's own `@return \Symfony\...\Response` does not
have. Nothing moved on the reference project or Slim.

## A one-argument `array_filter` leaves no nulls, and no declaration can say so

`array_values(array_unique(array_filter(array_map($cb, $types))))` is how PHP spells _map and drop
the misses_, and it answered `?string[]` where the author had declared `string[]` -- a false
positive on the reference project's only `incompatible-return`. The stub is not wrong:
`@return array<K, V>` is a true statement about the two-argument form, and the type language has no
way to write "V, minus the values PHP calls falsy".

So it is a rule beside `literal_returns`, which exists for the same reason: what the standard
library does that a docblock cannot express. Only the null is taken off -- `false`, `0`, `''` and
`[]` go the same way and have no operand to remove -- and only where the container already said its
keys are `int`. `array_filter` _preserves_ keys, and the only container this layer can build is
`Held::ListOf`, which answers `int` when asked what iterating it yields; rebuilding an
`array<string, ?Foo>` as one would turn a correct string key into a wrong int key. A miss rather
than a wrong answer.

Measured: `incompatible-return` on the reference project **1 -> 0** and on Laravel 17 -> 16,
`incompatible-argument` on Symfony 647 -> 646, nothing added anywhere. The idiom is written 89 times
in WordPress, 70 in Symfony and 38 in Laravel.

**The key comparison is the note worth keeping.** The first attempt compared `iteration_key_of_held`
against `Held::Builtin(Int)` and matched nothing at all: a `list<?Row>` written in the file answers
its key as a `Held::Written` pointing at the docblock's own `int`. Three spellings of one thing,
§8.1's second form, and the reader that knows all three is `kind_of_held`. The tell was that the
transform fired on no site while the guard was in and on every site with it taken out.

## An unread `@phpstan-import-type` is not nothing, it is a class the project does not declare

`type_alias.rs` said the import tag was "left exactly as unread as it always was", and a test said
so too -- which was true and was the bug. A name nothing resolves is lowered as an ordinary class
reference, so

```php
/** @phpstan-import-type Row from Source */
class Loose {
    /** @return Row */
    public function give() { return ['id' => 1]; }
}
```

read as a method declared to return a class called `\App\Row`, and the array it hands back was
reported. `$r = $this->give()` hovered as `Row`, a class that does not exist anywhere.

This is the trap a `@template` parameter sets and it already had an answer: `doc_names_a_template`
refuses a type written with a name nothing binds, for exactly the words -- "`@param T $needle` on
`in_array` would report every argument passed to it as taking a class called `T`". The import is the
same shape one file over, and had never been connected to it. The module's own note even contained
the general form, about a _local_ alias built from siblings: "a bare name in a type position is not
_nothing_, it is an ordinary class reference".

So the names are recorded -- under the **local** spelling, since `... from Source as Line` is what
the file writes and the far name never appears in it -- and a type written with one settles nothing.
The native hint then stands, which is what a reader with `array $row` beside the tag already had.

Expanding the alias across files is still unbuilt and is a different piece of work: a per-class
alias table in the index, interned into the global type table at ingest. Sized before deciding:
**one site in the reference project's own code** and 125 uses across 45 vendor files, all of them in
composer, phpstan and xdebug-handler rather than anything the project calls. The refusal costs
nothing and removes the wrong answers; the expansion is a five-crate change for a shape nobody there
reaches.

Nothing moved on any of the five corpora, which is the expected shape rather than a disappointment:
26 uses in user code across all of them, and a false positive only appears where the tag is written
_without_ a native hint beside it.

## A class the index holds nothing for cannot reject anything

`ClassHierarchy::relate` has always said so -- "a class nothing declares says nothing. Silence here
is what keeps an unresolved vendor name from becoming a finding on every call that mentions it" --
and it is asked only when _both_ sides are classes. A shape, an array or a string against a class
never reached it, so `builtin_as_class` rejected them without ever asking whether the expectation
named anything. Two halves of one rule and only one of them knew, which is the form §8.1 opens with.

What that costs is a report built out of an absence:

```php
/** @phpstan-import-type Row from Source */
class Loose {
    /** @return Row */
    public function give() { return ['id' => 1]; }
}
```

`Row` is a name nothing expanded, so it is lowered as a class, and the array the method really hands
back is "impossible" against it. Same for a `@template` nobody declared -- the reference project
writes `@return T` beside `@param class-string<T>` in three files with no `@template` line anywhere
-- and for a vendor class a checkout does not install.

Asked on both sides, because a name the index holds nothing for may not be a class at all: an
unexpanded alias is usually an _array_, so a value typed with one must not be rejected for not being
an int either.

Measured: WordPress `incompatible-assignment` 71 -> 70, Laravel 87 -> 86 and `incompatible-return`
16 -> 14, Symfony `incompatible-argument` 646 -> 645, and nothing added anywhere. Small because the
_shape_ is rare and the consequence is not: every one of the five was a report on code that runs.

Found by taking the `in_docblock` exemption off `undefined-type` for one measurement and reading
what came back -- 31 unresolved docblock names on the reference project, of which 9 are templates
nobody declared and 20 are classes a file names without importing. The check is right to stay quiet
about them; the _judgement_ was not.

## A base class calling what its subclasses implement

The template method, written the way PHP 4 had to write it: there was no `abstract`, so the pattern
is a plain class with `$this->_exec()` in it and the declaration one level down. WordPress still
ships it, and `ftp_base` was **62 of that checkout's 68 `undefined-method` reports** -- every one a
call that runs, because `$this` there is an `ftp_pure` or an `ftp_sockets`.

`outcome_for` already had three reasons to say _dynamic_ rather than _missing_ -- a trait, a class
with `__call`, a class a framework says answers anything -- and this is the fourth. Two halves keep
it from being an exemption:

- **Only through `$this`, `self::` or `static::`**, all three of which reach this layer as one root.
  A _variable_ typed as the base is a value whose class nothing here saw chosen, and widening to
  that would silence the check on every extensible class there is. The author who declared `Base`
  and called a `Pure` method is relying on a narrowing nothing wrote down, which is worth a report.
- **Only where a subtype really declares the name.** A typo has none.

What that second half is worth is visible in the file it came from: WordPress's own
`$this->my_regexp(...)` is declared nowhere, its source says
`// TODO: This should probably be glob_regexp(), but needs tests`, and it is now the one report in
that file instead of the sixty-fourth.

Asked last, after the three exemptions above, which is what makes the subtype scan affordable: it
runs only where a report is about to be made -- the arrangement `inaccessible_member` already has.
Direct subtypes only, matching `redeclared_below`; a declaration two levels down under a subtype
that inherits quietly still reports, which is where it stood before.

Measured: WordPress `undefined-method` **68 -> 5**, Symfony 1 386 -> 1 358 and `undefined-constant`
29 -> 27, Laravel 1 554 -> 1 549 and `undefined-property` 156 -> 155. Ninety-nine reports on code
that runs, and nothing added anywhere. `check --jobs 1` over the reference project is 1.19 s, the
same as before.

## A positional literal is a record too, and `[$a, $b] = $pair` was reading the union

`foreach ($cases as [$handler, $stage, $rest])` over an array of three-element literals gave every
name **the union of all three** — `Handler|Stage|Stage[]` — so the call that took the stage was
reported against a value it could never hold. Reported from the reference project, where it is how a
test's cases are written.

`['a' => 1]` already built a record and `[1, 2]` did not, which is §8.1's two spellings of one
thing: PHP's `[a, b, c]` _is_ `[0 => a, 1 => b, 2 => c]`, and one reader had the keys while the
other threw them away. The keying is PHP's own — a bare entry takes the next index, a literal
integer key moves the counter past it — so `['k' => 1, 2]` is an `array{k: int, 0: int}` exactly as
PHP builds it.

Four things came with it, and three are the kind of thing a change like this is made of:

- **The all-or-nothing rule a union needed is gone with the union.** `[new Row(), $unknown]` is an
  `array{0: Row, 1: mixed}`, exact about the slot it knows and honest about the one it does not; the
  old refusal existed because a union of the entries that answered is narrower than the truth, and a
  slot is not.
- **A spread splices.** `[...$pair, 'x']` on an `array{0: Row, 1: Row}` is an
  `array{0: Row, 1: Row, 2: string}`, integer keys renumbered and string keys kept, which is what
  PHP does. What it will **not** do is flatten a record whose slots _disagree_: that union is what
  every entry would have to be with the positions that said otherwise gone, which is the old
  approximation arriving one level in. Laravel spreads exactly such a pair and destructures the
  result, and reading the union put a `bool` among the operands of a slot holding an event name.
- **A record is judged slot by slot against an element type.** `['a']` against a `Row[]` was silent
  once the literal stopped being a `string[]`, because `value_element` has nothing to give for a
  record — each slot is its own. Every one of them has to fit, which restored two findings and added
  the one the union used to hide.
- **An integer key renders bare.** `array{0: string}` names the slot a positional literal fills and
  `array{'0': string}` names the _string_ key `'0'`, which is a different type — so `shape_key` has
  PHP's own canonicalisation now, and `'01'` stays quoted because PHP keeps it a string.

And one bug that was there all along and would have become loud: **a write two levels in is
invisible.** `$rows[$i][1] = $version` replaces one field of one element, and the slot reader
compares the write's base against the container — which is `$rows[$i]`, not `$rows` — so it saw
nothing and the literal that built the array still answered. With a union that was merely imprecise;
with a slot it said the version was _certainly_ null on a line that passes a string. Symfony builds
an array of tuples with a `null` placeholder and fills it exactly this way. A write that reaches
through the container now says the earlier claim no longer stands, which is a refusal rather than a
narrower guess.

Measured: reference project `possibly-false-argument` 53 → 50 and `incompatible-argument` 4 → 2,
Laravel `possibly-null-argument` 608 → 599, psl `incompatible-argument` 41 → 40. It rises on
WordPress by 3 and Symfony by 5, all read: `get_extended()` declares `@return string[]` and returns
three `preg_replace` results, `extract_full_block_and_advance()` declares `array[]|null` and returns
a flat record, a Telegram data provider's docblock has one array level too many, and `key($tag)` is
`string|int|null` where `addTag()` takes a `string`. Precision arriving as findings, which §8.2
records as the feature working.

## `is_numeric` rules out `false` and `is_scalar` does not

Three of PHP's predicates name no single kind and were one answer here: `is_numeric`, `is_scalar`
and `is_countable` all rule null out. What separates them is the other sentinel —
`is_numeric(FALSE)` and `is_countable(FALSE)` are false, and `is_scalar(FALSE)` is **true**, a bool
being a scalar. So
`$index = array_search(...); if (is_numeric($index)) { array_splice($x, $index, 1); }` kept a
`false` the guard had ruled out. Reported from the reference project.

What the guard vocabulary could not say is why. `NotNull { strictly }` had two settings — named null
exactly, or a truthiness test that rules out every falsy value — and `is_numeric` is neither:
`is_numeric('0')` is true and `'0'` is falsy, so reading it as `if ($x)` rules out a value it
admits. `Refused` names the three, and the one reader that needs more than "not a sentinel" asks
`proves_truth()` — a conditional return keyed on `non-empty-array` is settled by a guard above the
call, and `is_countable([])` is true, so reading the middle case as truth would call an empty array
non-empty.

**And each rules kinds out, which is a list per test and not a claim per kind.** Laravel's
`BusFake::assertDispatched($command, $callback = null)` takes `callable|int|null` and hands the int
on under `is_numeric($callback)` -- three `incompatible-argument` errors, `Closure|callable|int`
where an `int` was taken, because the guard took the null off and nothing else. No callable is
numeric (a function's name cannot start with a digit; the other two spellings are an array and an
object), so `RulesOut` lists per test the kinds _no_ value passing it can be: `is_numeric` no
object, array, iterable, callable, bool or resource; `is_scalar` no object, array, iterable or
resource -- it keeps `callable`, since `'strlen'` is both; `is_countable` no string, int, float,
bool or resource -- it keeps `callable` too, since `[$object, 'method']` is both. Each is a filter
over what the declaration listed, never an answer of its own, so a value nothing described stays so.

`is_scalar` had done this already as a `NotNull` with two `Kind` claims nested on it, and that shape
does not scale: **every nested claim is a step of the resolution budget** (`FOLLOW_LIMIT` is 12), so
seven of them would spend more than half of it on one guard -- and **the path that skipped a write
is described by the outermost claim alone** (`otherwise_of`), so
`if (!is_numeric($t)) { $t = time(); }` on WordPress's `int|bool` lost the sentinel half and put the
`false` back, one new finding in `current_time()`. One claim at all three levels (`Proved`,
`PathRoot`, `Otherwise::NoneOfKinds`) carries the refusal and the kinds together. The view reads it
through `without_sentinels`, now the one place the sentinel peel lives -- and that surfaced a second
bug: the peel took the empty shape off for every guard that takes the `false`, and
`is_countable(array{})` is true. Only a truthiness test takes `array{}` now.

Measured: Laravel −10 (all `is_numeric`/`is_countable` guards, every one a false positive),
WordPress −4 (`is_numeric($post_data)`, `is_numeric($widget_args)`), the other four corpora
unchanged, `check --jobs 1` unchanged on the reference project and WordPress.

## A write a loop carries round

`$lowest = NULL; foreach ($grades as $grade) { if ($lowest === NULL || $grade->min() < $lowest->min()) { $lowest = $grade; } }`
-- the read of `$lowest` is above the only write that gives it a value, and on every pass but the
first that write has run. `local_binding` reads the writes _before_ the position, so the name
answered certainly null, the guard beside it had nothing left to narrow, and goto and hover on
`$lowest->getMinRating` were dead. Found by `compare-lsp` on the reference project.

Always a join and never a replacement: the first pass arrives with whatever the name held before the
loop, so the answer is `?Grade`.

Four refusals carry the whole of it, and the corpora asked for each one in turn -- see
`docs/03-roadmap.md` for what each cost. The one worth restating here is the fourth, because it is a
shape rather than a case: **a carried write whose value depends on anything the loop also assigns is
a fixed point, not a value from the pass before.** `$input = substr($input, 3)` depends on itself
and `$dir = $renamed . SEP` beside `$renamed = substr_replace($dir, ...)` depends on itself one
variable away; joining either resolves the name through the loop again until the budget stops, and
what comes out is the union of everything the name ever holds. 95 findings on WordPress and Symfony,
every one of them a line that runs. What a carried write is _for_ is a value the loop settles
independently, which `$lowest = $grade` is: the loop's own target, bound by the header and touched
by no assignment.

Measured: three new findings across five corpora, all read. Two sound --
`getHeader(): ?HeaderSegment` called without a check, and pclzip's `$v_pos_entry = ftell($fd)` below
a read that hands it to `fseek`. The third is a false positive whose cause is a different rule:
`Leaving` models `return`, `throw` and `exit` and not `break`, so a write the loop can never come
back from is joined. psl and Laravel go down. 17/17 budgets.

## `break` leaves the loop, and `Leaving` only knew about leaving the function

`Leaving` decides whether a write in a branch can reach a position after it, and it models `return`,
`throw` and `exit` -- what leaves the **function**. A `break` leaves the _loop_, which is a
different question with two answers: it leaves past every position still inside that loop, and it
reaches every position after it. Read as neither, `$entry = FALSE; break;` joined a `false` into
every read below it in the same body -- a value control can never bring there.

The machinery was already written: `Leaving::of` has taken a `break_escapes` flag since switch arms
needed one. What was missing is a branch keeping **both** answers, derived once, and the one bit of
position that chooses between them -- §8.1's "a property of the tree recomputed per position", where
the tree settles what a branch does and the position settles which loop it is in.

Measured: Symfony `possibly-null-receiver` 2017 -> 2011 and `possibly-null-argument` 1158 -> 1155,
nothing rose anywhere, 17/17 budgets.

### And the same rule the other way round

The case that prompted all this -- `VfsIteratorTestTrait`'s `$fileEntry = false; break;` -- survived
that fix, and six reproductions of "the same shape" came back silent. What found it was bisecting
**three axes at once**, because only all three together produce it: a loop, an `isset`, and a branch
that escapes.

```php
$entry = $data;
while ($flag) {
    if (!isset($entry)) {   // <- the false arrives here, from below
        $entry = FALSE;
        break;
    }
    ...
}
```

The write is carried _up_ to the `isset` on the line above it -- correct by the back edge and wrong
here, because the branch `break`s and there is no pass after it. From the guard the `false` then
comes straight back, since ruling out null leaves a `false` standing. So `carried_from` asks the
same `escapes` the reach walk does, and for the same reason: what matters is whether control gets to
the top again.

Worth keeping as a lesson about the method rather than the bug. Six probes written one axis at a
time all said "works", and each one was evidence _against_ the shape being real -- which is the trap
`docs/08-performance.md` records about a probe whose two arms differ in two things, met here with
three. Symfony `possibly-false-subscript` 54 -> 53, which is the false positive the back-edge commit
had shipped knowingly.

- **`MemberTable::conflicts` had been computed for a year and read by nobody, and surfacing it
  showed the rule was not PHP's.** The field exists with a doc-comment that states the language's
  rule outright -- "Two traits declared the same member and nothing resolved it. A fatal error in
  PHP" -- and the code under it was "the name is already in the table and the last provenance was a
  trait". On a seven-row matrix run against PHP 8.5.10 that was wrong four times, and on Laravel it
  was wrong six. What PHP actually asks, each line of it run rather than reasoned: the **declaring**
  trait is what collides, not the one the class named, so two used traits that both `use Base` offer
  one method however many paths reach it; an **abstract** member is a requirement and collides with
  nothing, and the body wins; the class's **own** declaration settles it, and since own members
  merge _after_ traits it has to be asked for at the merge; and a member the **parent** holds does
  not enter into it at all, because a trait overrides an inherited method. That last one is the
  expensive one and it is a _provenance_ problem: a member the parent got from a trait is recorded
  as that trait's, which is right for navigation and indistinguishable from a trait of this class.
  The fix is not to read provenance at all -- `merge_traits` now keeps its own list of what _this_
  class's `use` list has contributed, by declaring trait. Laravel's `UuidUserStub` is the case: one
  trait of its own declares `newUniqueId`, the parent got another from a trait, and PHP resolves it
  in the trait's favour. The general form is the one §8.1 calls a rule with nowhere to live: a field
  whose _comment_ is the specification and whose only reader is the thing that avoids acting on it
  will drift, and nothing fails. The moment something reported it, four of seven rows were wrong.
  Properties and constants are left to the table and not reported: they collide by _value_ rather
  than by name, and a **promoted** property follows the constructor that declares it, so an
  `insteadof` on `__construct` takes it along. Laravel writes exactly that.

- **A `bool` is a sentinel only as a union operand, and the walk that decided it had no way to know
  where it stood.** `possibly-false-argument` reported `$x takes int, and this may be false` for a
  plain `bool $flag` passed to an `int` parameter, because the reach walk reads a bare `bool` as
  naming the `false` without spelling it. That reading is right for `object|bool`, which is how a
  docblock writes "the call may have failed", and wrong for a parameter declared `bool`, which is a
  boolean: `takesInt($flag)` in a file without `strict_types` coerces, and PHP 8.5.10 was asked for
  the whole coercion matrix rather than the rule being reasoned. The fix is one flag,
  `bool_takes_false`, and the cost of it is that the _union_ is a union in three spellings --
  `Held::OneOf`, an interned `TypeRef::Union` and a written `TypeRefData::Union` -- which is §8.1's
  three-spellings form exactly: threading it through two of the three leaves the third answering
  differently, and both of the tests that broke were that. It is also true on both sides of a
  question that looks like one: `type_admits` asks whether a declaration _accepts_ a sentinel and a
  `bool` parameter does, so the expectation side passes `true` where the value side passes `false`.
  Measured after: strict mode reports exactly PHP's 19 refusals, weak mode 8 of the 10 with the two
  undecidable numeric-string cells correctly silent. WordPress `possibly-false-argument` 1352 ->
  1355, the three new ones `string|bool` from WordPress's own `@var` and true positives of the shape
  the other 1352 already are.

- **A flag that removes a failure branch is a conditional return, and `json_encode` is the one
  everybody writes.** `json_encode($x, JSON_THROW_ON_ERROR)` cannot return `false` -- it throws a
  `JsonException` instead -- and the set declared the bare `string|false`, so every
  `md5(json_encode($x, JSON_THROW_ON_ERROR))` read as a `false` reaching a `string` parameter. 18
  findings across five projects, reported by the user on their own code. Asked of 8.5.10 rather than
  read, including the one combination that keeps a string alive beside the flag:
  `THROW|PARTIAL_OUTPUT` returns `'[null]'`, and a depth overflow throws like everything else.
  Written as `@return ($flags is 4194304 ? string : string|false)` -- the _number_, because a
  conditional tests what the call wrote and a call writes the constant's name, which the reader
  looks up where it is declared. Two things that pair with it. `constant_number` hardcoded
  `NameAnchor::Relative` and passed the written name on, so `\JSON_THROW_ON_ERROR` resolved to
  nothing while `JSON_THROW_ON_ERROR` settled -- the **fourth** reader today to have taken half of
  `NameAnchor::of`'s pair, after the chain's function root. And a flag written as its raw number
  stays unsettled, because `small_number` records only 0 to 15: a deliberate bound on what lowering
  interns, and a miss rather than a wrong answer, since the declaration's union of both branches
  still stands. What must **not** go beside it is a `@throws`. The tag has no conditional form, so
  declaring one reports every plain `json_encode($value)` as throwing: 16 on the reference project,
  and the four sampled were all calls with no flags.

## Asking PHPStan where the gaps are, and what the answer turned out to be

`tools/phpstan-diff.py` now takes `--rule <identifier>`, which lists one rule's sites with what
phrost says at each instead of the summary. A count says a gap exists; a list says whether it is one
gap or forty, and whether phrost is silent because a check is missing or because the receiver it
would need never resolved. That is the difference between a number and work.

Run at level 8 on the reference checkout: 2 720 findings, of which 2 232 are `missingType.*` -- a
docblock could be more precise -- and 35 are a position about `mixed`. **453 are what "are we worse"
is about**, and reading them by rule settles most of it:

- **`method.notFound` 62 and `property.notFound` 12 of 14 are `object::whatever()`.** A value
  declared `object` has no members PHPStan can verify, so every call on one is an error to it.
  phrost calls that `MemberAccessOutcome::AnyObject` on purpose and says nothing: the index has an
  answer and the answer is "the author wrote any object". Recorded in
  `crates/view/src/references.rs` and in the coverage note -- 345 accesses off one generated
  directory's `@param object` sat at the top of a gap list while being no gap at all.
- **`method.nonObject` 126 are `StatementInterface|null`**, every one of them Drupal's
  `Select::execute()`. That is the first paragraph of `assumed_non_null.rs`: the declaration says
  `?StatementInterface` and no run returns null, so a framework contract removes it and there is
  nothing to disclose. Measured when the contracts were written: 118 of 379 null-safety findings
  were this shape, "all of them noise".
- So **188 of the 453 are two positions phrost holds deliberately**, both already written down. A
  project that wants them has `assumed-non-null`, which discloses the _other_ kind -- the nulls a
  contract removes that are real -- and reports 130 on the same checkout, disjoint from these 126.

What is left is the tail worth reading: `argument.type` 181 with 68 already answered at the same
line, `offsetAccess.notFound` 38 with 13, `return.type` 36 with 2, and single-figure rules. The two
`DOMNameSpaceNode|DOMNode::$textContent` under `property.notFound` are the one genuinely new shape
the pass found: **a member access on a union where not every operand declares it**, which phrost
resolves through the operand that does.

The general point, and the reason to keep the tool rather than the number: **a rule another checker
reports and this one does not is a position until somebody reads the sites.** Two of the three
biggest buckets here were positions, and both were documented before the comparison was run.

## A parameter with a default is still a parameter

`sentinel_arguments` paired an argument against its parameter and stopped at the first one with a
default: `if param.is_variadic || param.has_default { break; }`. A **break**, so every argument from
that position onwards went unchecked for a null or a false. `date('F jS', strtotime($when))` is the
shape -- `strtotime` gives `int|false`, `date`'s second parameter is `?int $timestamp = null` -- and
nothing was said about it anywhere.

Two things make it a clear bug rather than a judgement. `incompatible_arguments` pairs the same
arguments against the same list and has never stopped at a default, so this was §8.1's first form:
**two constructors of one rule**, and only one of them had learned it. And the comment that
justified it in the test beside it -- "the default's value is not stored, so a parameter with any
default is declined rather than guessed at" -- had gone stale: `ParamRow::default` carries the
default _as written_, for the quick fix that writes signatures back.

So the pairing continues, and a parameter defaulting to `null` is read as taking null whatever its
hint says. That is PHP's own rule, deprecated on 8.4 and still in force, and the bundled stubs are
full of `int $t = null`.

Measured: +42 `possibly-false-argument` and +1 `possibly-null-argument` on the reference project,
and 191 / 164 / 49 / 10 more `possibly-false-argument` on WordPress, Symfony, Laravel and psl. Nine
verified by hand before the baselines were accepted -- `esc_url(get_permalink())`,
`file_exists($fs->realpath($to))`, `fopen($path)` handed to `fputcsv`, `date($f, strtotime($x))`,
`wp_die(wp_json_encode($x))` -- every one of them a real `false` reaching a parameter that does not
take one. The one that is arguable is `reset($recipients)` guarded by a later `count(...) === 1`,
which PHPStan reports too and which a declared non-empty array would not settle either -- the guard
is after the call.

Found by `tools/phpstan-diff.py --rule argument.type`: 13 of PHPStan's findings on the reference
project are exactly the `int|null ← int|false` pair, and phrost answered none of them.

## Non-emptiness is carried as a flag, and the price that kept it out was the wrong one

`non-empty-list<int>` and `list<int>` both lowered to `array<int, int>`, so PHP 8.5's
`array_last(non-empty-list<int>)` answered `?int` where the stub's conditional return
`($array is non-empty-array|non-empty-list ? V : null)` says `int`. That is §8.1's _two spellings
collapsing into one_: the distinction was gone before any reader existed.

An earlier pass priced carrying it as a new dimension on array types plus narrowing for
`count($x) > 0`, `$x !== []` and `!empty($x)`, counted the findings on lines that call `reset` and
its family -- 104 of 12 078 -- and left it. Both halves of that price were wrong. The narrowing was
already there: an empty array is falsy, so every guard that rules out falsehood already settles the
conditional (`proven_truthy`). And the findings it fixes are not on the line that calls `reset` --
they are on the line that _uses_ what came back, which is why counting call sites found so few.

What it took is `non_empty: bool` on `Generic`, in the HIR and in the interned table, set by
lowering for `non-empty-array<...>` and `non-empty-list<...>`. A flag and not a base of its own, so
every reader that asks what an array holds -- some fifty patterns, given `..` mechanically -- keeps
answering for it without knowing it exists; the two that ask whether it can be empty read it
(`declared_non_empty`, and `[]` handed to one, which no conversion fills). One degenerate form came
with it: an undecided conditional joins `non-empty-list<K>|list<K>`, which used to be one interned
type, so `TypeTable::union` folds the non-empty twin into the plain one.

Measured: 49 findings gone on four corpora -- 40 `possibly-null-argument`, 5 `possibly-null-return`,
4 `possibly-false-argument` -- and the samples read were all `array_pop(explode(...))` and its
neighbours, where `explode` is declared `non-empty-list<string>`. Four new, every one a test passing
`[]` on purpose to assert the exception (psl suppresses mago's equivalent finding on the same line).
A bare `non-empty-array` with no arguments still says no more than `array`: there is no `Generic` to
carry the flag, and inventing one is a node with no arguments every reader would have to learn.

## A closure declares a return, and none of the three return checks read it

`judged_returns` paired each `return` with a declaration the _skeleton_ holds, matched by name
range. A closure has no skeleton declaration, so `incompatible-return`, `possibly-null-return` and
`possibly-false-return` were all silent inside one -- and the module's own docs said so:

> **A closure's or arrow function's own `return`.** Its declared type is a body-local `TypeRefId`
> that is never interned, and the expected side of the judgement must be a `TypeId`. Reaching below
> this crate to intern one is the only fix, so the gap stays open and stated.

That stopped being true when `Expectation` was written for the `@var` branch, which has exactly the
same shape: a declaration that is an index into the file's own arena rather than a name the project
interned. The gap outlived the reason for it, and a note saying "stays open and stated" is a note
nobody re-reads. **A stated gap is worth re-testing whenever the thing that forced it moves.**

What it needed beside `Expectation` was one more pair: `type_admits` had a `stored_reach_within` and
a `written_reach_within` already, so `expectation_admits` is one entry over the two rather than a
second "does this declaration take a null".

Four shapes on a four-line fixture -- a method, a closure, an arrow function and a static closure,
each returning `preg_replace(...)` where `string` is declared -- and only the method reported
before. Measured: +5 on Symfony, +5 on psl, +3 on the reference project, nothing on WordPress or
Laravel, which write few typed closures. Every one verified before the baselines were accepted; the
psl five are arrow functions in a `strict_types=1` test file returning a `bool` where `int` is
declared and an `int` where `null` is, and the reference project's is the `preg_replace` PHPStan
names at the same line. Diagnostics p95 11.5 ms against a 30 ms budget and the serial sweep 1.8 s,
both unchanged.

Found by `tools/phpstan-diff.py --rule return.type` -- an anonymous function reported by PHPStan and
by nothing here, which is a shape no fixture in this repo had.

## The fourth site: a value that is invoked and may not be callable

The sentinel family asked its two questions of a receiver, a subscript and a `foreach` subject and
not of the value between `$` and `()`. `$callback()` where `$callback` is `?callable` is
`Error: Value of type null is not callable`, and nothing said so.

Two things about it that hold nowhere else in the family, both asked of 8.5.10 rather than read off
the manual. **Both sentinels are fatal**: weak mode converts a `false` _argument_ and the line runs,
which is why that code is the softer half of its pair, and a `false` callee is
`Value of type bool is not callable` whatever the mode. And **the certain half belongs to somebody
else**: a value whose _kind_ is `null` or `bool` is already `impossible-use`, which was written for
that error and names the kind it found -- so this reports only what _may_ be a sentinel, which is
the half no kind can settle. Written the other way first, the fixture reported twice on one line,
which is how the overlap was noticed.

Measured: **63 and 20 `possibly-null-callee` on Symfony and Laravel**, one `possibly-false-callee`
each, 8 on WordPress, and **zero on psl and zero on the reference project**. Verified before the
baselines were accepted -- `(new StreamedResponse($chunks))->getCallback()` invoked straight,
`$question->getValidator()` likewise, Laravel's `($this->queueResolver)(...)` -- every one a
declared `?callable` invoked with no guard, and every one PHPStan reports at the same line.

Worth recording what the population is made of, because it is the argument for the next refinement
rather than against the check: **23 of Symfony's 63 and 5 of Laravel's 20 are a `Closure::bind` or
`->bindTo()` result invoked directly.** Those return `?Closure` and produce null only when a
_static_ closure is bound to an object -- a condition an argument states and the declaration cannot.
That is the shape `ReturnContracts::add_unproduced` exists for, and naming it there would take a
third of the findings off two corpora without touching the rest.

Found by `tools/phpstan-diff.py --rule callable.nonCallable` on Laravel's own source, where 88 of
107 findings are a `?callable` invoked without a guard -- which is what `when()` and `unless()` are
written as. The tool takes `--config` and a path now, because a project shipping one configuration
per component has to be told which.

## `deprecated-use`, and the two neighbourhoods that make it about callers

The declaration says not to reach for it; the finding is about who does. Both spellings feed one bit
at ingest, so the check is three walks that already existed -- member accesses, class-like name
references, call sites -- and it costs nothing measurable: `phrost check` on the reference project
stayed at 302 ms on eight threads and 1.89 s on one, and the diagnostics p95 at 11.9 ms.

What took it from noisy to useful is two rules about _where_ a use is written, and both were found
by reading the findings rather than by reasoning:

- **A deprecated declaration may use deprecated things.** That is what a shim is: a wrapper kept for
  one release, calling what it replaced. Sheltered by span -- the deprecated classes and members of
  the file, settled once per revision.
- **A class keeping its own deprecated member working is not a caller of one.** Laravel's
  `OutputStyle::write()` assigns `$this->newLineWritten` on every line it prints, which is how the
  property stays true for whoever reads it. WordPress reported **194** uses before this rule and
  **118** after, and the 76 that went were all a class talking to itself. A *sub*class is somebody
  else and is still told -- `WP_Customize_Control::json` is read by four of them.

Two more that are about what a use _is_. A docblock is prose: `{@see ClientConfiguration}` written
in the class that replaced it is the sentence explaining the deprecation, and reporting it reports
the fix as the problem. And a name written at a file's own top level, where the declaration is also
in that file, is the file announcing itself -- Symfony writes
`trigger_deprecation('symfony/...', '8.2', '...', Middleware::class, ...)` above the class it is
about. Only the top level, so a neighbouring declaration in the same file is still a caller.

Measured, all five verified by sampling: WordPress 118, Laravel 168, Symfony 344, psl 48, the
reference project **1** -- `ReflectionMethod::setAccessible`, a no-op since PHP 8.1. The four
framework corpora are where deprecation lives and an application is where it does not, which is the
opposite of the distribution §8.2 records for the collector-and-bail shape and worth knowing for the
same reason: the instrument that says a check is noisy and the one that says it is quiet are both
right about different code.

## An unparameterised generic is not untyped, it is its bound

Reported from a real checkout, where PHPStan was silent and phrost was not:
`foreach ($element->childNodes as $child)` typed `$child` as `DOMNode|DOMNameSpaceNode` and reported
every `$doc->saveHTML($child)` beside it. The vendored stub declared `DOMNode::$childNodes` as a
bare `DOMNodeList`, and `DOMNodeList` is `@template-covariant TNode of DOMNode|DOMNameSpaceNode` --
so the property did not read as _unknown_, it read as the bound. A plausible union, a plausible
finding, and nothing anywhere saying a tag was missing.

The general shape is what to keep. A missing type annotation usually shows up as silence, which is
easy to recognise; a missing **type argument** shows up as an answer, and the answer is wide in
exactly the way a real union is. So when a finding names a union whose second operand looks exotic,
ask which declaration supplied the argument before asking what produced the value.

PHP settled the case itself, in the two minutes §9 promises: `DOMNameSpaceNode` is not a `DOMNode`
(`class_parents()` is empty), and `appendChild()` and `importNode()` both throw
`TypeError: must be of type DOMNode, DOMNameSpaceNode given` -- so a namespace node cannot be in a
tree and only `DOMXPath::query()` over a namespace axis makes one. The stub's own modern twin
`Dom\Node::$childNodes` already carried `@var NodeList<Node>`; it is an omission on the legacy half.
Ten findings on Symfony, one on the reference project. See the stub set's README for the edit and
`the_added_type_tags_survive` for the tripwire.

### A tag written inside a namespace resolves inside it

Writing the A/B that pins this -- a tagged property against a bare one -- failed on the fixture and
not on the rule, twice. `@template-implements IteratorAggregate<int, T>` inside `namespace App;`
names `App\IteratorAggregate`, which nothing declares, so the arm answered nothing and read as a
missing feature. The stub files get away with the same spelling because they wrap everything in a
braced global `namespace { ... }`, which is also why `dom.php`'s classes are indented and why a
class scanner anchored at column zero once found none of them.

So a fixture copying a stub's docblock has to add the leading slash the stub did not need. The
general form: a tag naming a type is resolved against the imports and namespace of the file it was
written in, and a fixture is a different file from the one the shape was copied out of.

## A free function's own template in its return answers nothing, and the key half is why

Found by taking `mago-prelude` from 1.48.0 to 1.50.0, whose whole step is 17 lines and three
declarations. Two of them cost nothing on any of the five corpora. The third taught `preg_replace`
to preserve its subject's keys:

```php
 * @template K of array-key
 * @param string|array<K, string> $subject
 * @return ($subject is string ? string|null : array<K, string>|null)
```

which is right, and which phrost reads as **nothing at all** -- so the native `string|array|null`
stands and every `trim(preg_replace('/\s+/', ' ', (string) $x) ?? '')` is reported
`$string takes string, and this is array|string`. WordPress +13, Symfony +9, Laravel +1, the
reference project +10.

The first reading blamed the conditional, since the template sits in the branch a string subject
does _not_ take. A four-arm probe says otherwise, and it is the note in §8.2 about a probe whose
arms differ in two things, applied early enough to cost nothing:

| return                                                                                   | answer      |
| ---------------------------------------------------------------------------------------- | ----------- |
| `array<K, string>\|null`, no conditional at all                                          | **nothing** |
| `($s is string ? array<K, string>\|null : string\|null)` -- template in the taken branch | **nothing** |
| `($s is string ? string\|null : array<K, string>\|null)` -- template in the other branch | **nothing** |
| `($s is string ? string\|null : array<string>\|null)` -- no template                     | `?string`   |

So the conditional is innocent and the first arm is the whole bug: a **free function** whose return
names one of its own templates resolves in nothing. `held_of_branch`'s `ReturnBranch::Ty` asks
`supplied_for_branch`, which opens with `if at.receiver.is_empty() { return None; }` -- a receiver
is where a _class's_ template argument comes from, and a free function has none -- and then
`names_something_real` refuses `K` for being a name nothing declares. Correctly, at that point:
answering it as written would put `K` in a tooltip.

What is missing is the branch kind that would have caught it, and the shape is one this document
already names: **the element half exists and the key half does not**.
`ReturnBranch::ArgumentElement` says "the return is what a container argument holds" and there is no
`ArgumentKey` saying "the return is a container keyed the way a container argument was". Upstream's
improvement is _exactly_ a key-preservation claim, which is why it landed on the missing half rather
than somewhere forgiving.

Left undone deliberately, with the cost written down: `preg_replace` stays at 1.48.0's unkeyed
spelling, recorded in the stub set's README as the one place this project is behind upstream rather
than ahead. It is worth taking the moment the key half exists, and the four rows above are the test
to write first.

## A disclosure fired where a contract applied, not where it decided anything

`assumed-non-null` is the check that says out loud which sentinels a framework contract took off a
declaration, so a project running phrost beside PHPStan at level 8 can see what was judged for it.
It fired at **130** sites on the reference project. Reported from outside as a bug in the _type_ --
"`getFileUri()` is read as never null, though `FileInterface::getFileUri()` declares `?string`" --
which it is not: the type is read exactly as the contract says, and the disclosure is the contract
saying so. The real complaint is the count, and it is a fair one.

What settled it is the project's own vendored PHPStan over the 54 files carrying the notes, at level
8, matched site by site:

|                                                                             |         |
| --------------------------------------------------------------------------- | ------- |
| phrost disclosures                                                          | **130** |
| PHPStan reports a null problem on the same line                             | **19**  |
| ... plus one or two lines down, through the local the value was assigned to | **+4**  |
| disclosures where nothing depended on the decision                          | **107** |

The 107 are `sprintf('%s', $file->getFileUri())`, `'name' => $file->getFilename()`, a concatenation,
a `(string)` cast, and twelve where the author had already written `?? ''` or `?->` on the very line
being disclosed. A contract applies to _every_ call of a contracted method; it only **decides**
something where the sentinel would have been refused.

So a disclosure now survives only where the sentinel, had it stayed, would have been refused. The
part worth keeping is how that question is answered: **not** by a rule here about which positions
refuse a null -- that would be a second constructor of the six `possibly-null-*` checks, and §8
names the shape. It is answered by reading the file twice. `IndexView::with_contracts_lifted` builds
a twin of the same view whose `contracted()` is the identity; whatever the ordinary checks report
against the twin and not against the real view _is_ the set the contracts suppressed, by
construction. Adding a seventh kind of refusal is one line in `sentinel_refusals` and no new rule
anywhere.

Two details that are load-bearing. The twin is a whole **view** and not a flag on the call, because
resolution is memoized and a path resolved with contracts lifted is a different answer from the same
path resolved with them applied -- one cache for both would hand whichever asked second the other's
answer. And what stands _with_ the contracts applied is subtracted, because not every refusal is
about a sentinel: `$this->count = $file->getFileUri()` is refused either way, and the contract
decided nothing there.

Result on the reference project: **130 -> 18**, and PHPStan level 8 corroborates all eighteen -- 16
on the same line, 2 one line down. Cost, measured as the serial wall time of `check` over the whole
project, which §8.2 says is the quiet sample: **4.26 s before, 4.28 s after**. Flat, because a file
with no contracted call pays nothing and a file whose contracted calls are all in tolerant positions
stops after the first extra read.

### And the three it does not disclose, which are somebody else's gap

PHPStan reports a null problem at three sites the filter now drops:

```php
fileUris: [$file->getFileUri()],
clientId: $user->uuid(),
```

Both are **named arguments**, and `sentinel_arguments` stops at the first one of those -- "past a
name or a spread, position has stopped meaning anything" -- as does `bind` and as does
`incompatible_arguments`. So the contract suppressed nothing there: the argument check cannot see
the site at all, with the contract or without it. The filter is right to be silent and the gap is
real and separate -- three findings on one project, and named arguments are the direction PHP 8 code
is moving. Worth pricing on the corpora before building: the pairing has to carry a name to a
parameter by name rather than by position, which is a change to `bind` and not to any check.

The general shape, and it is not about contracts: **a report that fires wherever a rule applied,
rather than where it changed an answer, is measuring the rule and not the code.** The tell is a
count that does not move when the code gets better. The instrument that separates the two is the
rule's own absence -- run without it and diff -- which is cheaper to build than a second rule
describing when it matters, and cannot disagree with the checks it is about.

## `{@inheritdoc}` carried the return and not the parameters

Asked of the reference project: where are we behind PHPStan at level 8. Its own vendored phpstan
reports **2 645** findings there, and the ranking matters more than the total:

|                                                     |         |
| --------------------------------------------------- | ------- |
| `missingType.*` -- a docblock could be more precise | 2 214   |
| `mixed`, which is a position rather than a rule     | 35      |
| the rest, which is what "are we worse" is about     | **396** |

Of those 396, **96 are PHPStan being wrong and phrost being right**:
`Cannot call method fetchAssoc() on StatementInterface|null` and its four neighbours, which is
`Connection::query()` and `Select::execute()` -- read out of core in `providers/drupal/database.rs`
as `NullFact::Unreachable`, not assumed. Another **72** are one position about a bare `object`
receiver, which phrost stays silent about deliberately. **38** are optional keys in array shapes.
**23** are `numeric-string`, a refinement phrost does not model.

What is left, and what this note is about, is **45 sentinel-shaped arguments and returns** -- a
`string|null` or a `string|false` reaching something that takes neither -- where phrost has the
check and said nothing. A four-arm probe splits them into three causes, and each arm differs from
the control in exactly one thing:

| arm                                                                | fires?                                     |
| ------------------------------------------------------------------ | ------------------------------------------ | --- |
| positional argument, parameter natively `string`                   | **yes**                                    |
| the same, written as a **named argument**                          | no                                         |
| parameter typed by a `@param` **on the declaration itself**        | **yes**                                    |
| parameter typed by a `@param` reached only through `{@inheritdoc}` | no                                         |
| the same `@param`, called through the **interface** that writes it | **yes**                                    |
| `resource                                                          | false`into a builtin's`resource` parameter | no  |

The third row against the fourth and fifth is this note. `MemberTable::insert` already carries two
things from an overridden declaration onto an override that repeats the signature and not the
docblock -- the contract, and the return's `doc_ty` -- with a comment naming the Drupal idiom it
exists for. It carried nothing for the **parameters**. So `FileSystemInterface::deleteRecursive()`
writes `@param string $path` over an untyped parameter, `FileSystem::deleteRecursive()` writes
`{@inheritdoc}` over the same one, and a caller reaching the implementation -- which is what
`\Drupal::service('file_system')` hands back -- saw a parameter with no type at all. Two
constructors of one rule, with the two halves of one signature as the two constructors, and the half
that was missing is the half a framework's callers actually reach.

### What stops the carry, and why each stop had to be measured

Built naively it added **14 findings across five corpora and 8 of them were wrong**. The two
restrictions that fixed it are both instances of things this document already records.

**A tag that was written is an answer, whatever came of it.** Laravel's `Collection::when()` is
documented `@param bool $value` on `Enumerable` and
`@param (\Closure($this): TWhenParameter)|TWhenParameter|null $value` on the `Conditionable` trait
that implements it. The trait's tag names a template, which lowering refuses -- correctly, since
lowering it would put a class called `TWhenParameter` in a tooltip -- and the empty slot it leaves
is indistinguishable from an undocumented parameter. Reading it as silence carried the interface's
`bool` down over it: six false `incompatible-argument` on Laravel's own type assertions. The fix is
a bit, `ParamRow::documented`, set from a new `ParamData::doc_type_refused`; it fits the padding
beside three existing bools, so it costs nothing. The general form: **an empty slot has two causes
and only one of them is silence.**

**On the input side a native type is not silence either.** Symfony's
`BooleanToStringTransformer::transform(mixed $value)` implements a `@param TValue|null` on a
`@template`d interface; carrying over the `mixed` put `?\Symfony\Component\Form\TValue` -- a class
nothing declares -- on a parameter its author had opened to everything. So the carry fills only a
slot that is **entirely** empty, where the return half may refine a native hint. That asymmetry is
the rule and not a shortfall of it: a return may only get more precise, and a parameter is
contravariant, which is the fourth and fifth time this document has had to say it.

After both: WordPress 0, Slim 0, Laravel +1, Symfony +2, the reference project +2 -- and every one
checked. Symfony's two are `normalizer_normalize()`'s `string|false` passed to
`AbstractString::trimPrefix()`; Laravel's is `?Application` into an inherited `@param Application`;
the reference project's are `deleteRecursive(realpath(...))` and `addCacheableDependency($user)` on
a `?User`. **The user fixed both of theirs from the editor while this was being measured**, which is
the confirmation no corpus can give -- and a reminder that a count on somebody's working tree moves
for reasons that are not the code being measured.

### The template leak this walked into and did not fix

`doc_names_a_template` reads only the _declaration's own_ docblock for `@template`, so a `@template`
written on the **class** is invisible to it and `@param TValue $value` on `DataTransformerInterface`
lowers to a class that does not exist. That is a pre-existing leak -- any receiver typed as the
interface already got it -- and the `mixed` stop above is what keeps the carry from spreading it.
Fixing it properly means threading the enclosing class's template names into member lowering the way
`TypeAliasTable` already is, and it is a semantics change with its own measurement: it takes types
_away_, on every generic class in every project.

### Still behind, in order of what it would buy

- ~~**Named arguments.**~~ Closed; the note below has it.
- ~~**`resource`.**~~ Closed; the note below has it.
- ~~**Optional keys in array shapes.**~~ The `array{}` half is closed; the note below has it. What
  is left of the 38 is 7 where a lone shape does not list the key at all, which is a position rather
  than a gap.

## Three walks stopped at the first named argument, and a call written by name was judged not at all

The second of the three causes behind the 45 sentinel-shaped findings PHPStan reports on the
reference project and phrost did not. `sentinel_arguments`, `incompatible_arguments` and
`written_back_arguments` each opened their pairing with

```rust
if argument.name.is_some() {
    break;
}
```

with a comment saying position has stopped meaning anything -- true, and not a reason to stop. PHP
has filled parameters by name since 8.0, and the rule is small: positional arguments fill parameters
in order, a named one fills the parameter it names, and the grammar forbids a positional argument
after a named one, so the two never have to be reconciled. `break` also takes down every argument
_after_ the first name, which on a call written entirely by name is the whole call -- Laravel's
`ListenCommand` builds a `ListenerOptions` out of nine named arguments and nothing was said about
any of them.

Fixed as one `paired()` helper the three walks share, rather than three copies of the rule: the
shape §8 calls two constructors is what put the same `break` in three places to begin with, and a
fourth walk arriving later should get the pairing rather than a decision about it. It still declines
what it cannot know -- a spread, and a name no parameter has -- and an unknown name does not stop
the names after it, because a name carries its own position and one bad one does not move the
others. Named arguments are matched **case-sensitively**, which is PHP's rule and the one place in
the language where a declaration's spelling is load-bearing.

+14 across five corpora, every one read: WordPress 0, Slim 0, Laravel +11, Symfony +3, and on the
reference project one that is a **runtime `TypeError`** --
`$lm->loadLogs($id, 'Notification Manager', fetch: \PDO::FETCH_ASSOC)` against `?FetchAs $fetch`,
which is Drupal 11 having replaced the integer fetch modes with an enum. Nine of Laravel's eleven
are that one `ListenerOptions`; Symfony's are `json_decode(json: file_get_contents(...))` and two
tests passing a union where a declaration takes one arm.

`a_named_argument_and_a_spread_stop_the_pairing` had pinned the old behaviour and is rewritten
rather than deleted: the spread half is still true, and the named call in the same fixture is now
the control that proves which half the test is about. That is worth noticing in general -- **a test
named for two rules will keep passing when one of them turns out to be a gap**, and the tell is that
its name reads as a fact about the language when only half of it is.

## `resource` was a name resolving to no class, and a union holding one lowered to nothing

The third of the three causes. PHP's own library writes `@param resource $stream` 320 times in the
bundled stubs and `resource` was not in the vocabulary, so every one of them was a name the index
holds nothing for and was **dropped whole**. `fwrite(fopen($path, 'r'), 'x')` passed `fopen`'s
`false` into a parameter about which nothing at all was known.

Worse than one tag at a time, and this is the part worth carrying: `lower_doc_type`'s union arm is
`let left = self.lower_doc_type(union.left, ...)?;`, so a union **one of whose operands does not
lower does not lower either**. `@param resource|CurlHandle $handle` lost the `CurlHandle` too. The
same `?` runs through `Nullable`, `Intersection` and the container arms. It is the right default --
half a union is a claim nobody wrote -- and the cost of a missing kind is therefore not proportional
to how often the kind is written, it is proportional to how often it appears **beside** something
else. Three `unused-import` findings on WordPress disappeared when this landed, because the
`StreamInterface` in `@param resource|StreamInterface` became a use of the import.

### What PHP said, and it changed the design twice

`resource` is not declarable, so a docblock is the only way in -- and the first version put it in
`BuiltinTy::from_name`, which is the function every _name_ goes through. That compiled clean. Asked
of the runtime instead (8.5.10):

```
$ php -r 'function f(resource $x) {}'
Warning: "resource" is not a supported builtin type and will be interpreted as a class name
$ php -r 'class Resource { const X = 1; } echo (new Resource)::X;'   # runs
```

So the word is **class-like wherever a declaration uses it** and a type only where a docblock does.
`from_name` was the wrong door and `class Resource {}` became unreachable through it; one fixture in
`class_of_an_instance` declares exactly that class and is what caught it. The kind is now reached
from `Type::Resource | ClosedResource | OpenResource` in the docblock lowering and from nowhere
else. §8 already warns that adding a `BuiltinTy` variant changes behaviour silently; what this adds
is _where_ to ask -- the runtime, in one line, before choosing the door.

The second answer PHP gave is the fit rule. A resource is the one entry in the vocabulary that weak
mode does not soften:

```
s(string) i(int) b(bool) a(array) o(object)  <- a file handle
s: TypeError  i: TypeError  b: TypeError  a: TypeError  o: TypeError
(string) $fh === "Resource id #5"   (int) $fh === 5
```

Every declared parameter is a `TypeError` in weak mode as well as strict, and an explicit cast still
works -- which is a different question and is why the cast rules stay where they are. So
`(Resource, _) | (_, Resource) => Fit::Rejected`, one arm, no scalar table.

`is_resource` went into `type_predicate` in the same change, not a later one: a kind nothing can
guard is a kind that reports every guarded use, and the first real project is where that is found
out.

### What it costs, and the one family that is a project's decision rather than a defect

+850 across the five corpora: WordPress +187, Laravel +85, Symfony +535, Slim 0, and the reference
project +43. Read rather than counted:

| family                                                                                                        | where                                            | true?                                       |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------- |
| `$stream takes resource, and this may be false` -- an unguarded `fopen`                                       | Symfony 278, WP 40, reference 41                 | yes, and PHPStan level 8 says the same      |
| `$hs is declared resource, and this is resource\|object` -- `hash_init()` returns a `HashContext` since PHP 8 | WP                                               | yes, stale PHP 5 code                       |
| `$handle takes \CurlHandle, and this is resource\|CurlHandle`                                                 | WP 45, plus `GdImage` 27 and `FTP\Connection` 26 | **true under PHP 8 and written on purpose** |

The last family is the one to be honest about. WordPress documents `@param resource|CurlHandle`
because it supports PHP 7.2 and up, where the value really is a resource; under the stub set's PHP 8
the resource arm never occurs and the union is stale. phrost has no `phpVersion` setting, so it
reads the stubs' version and says so. That is a **decision a project makes**, not a defect in either
side, and the ratchet is where it gets made.

On the code the reference project's own PHPStan is pointed at, this is **7 findings, 6 of them
corroborated at level 8** -- the other 36 are in `.local/php-scripts/`, ad-hoc scripts outside the
paths phpstan.neon scans. That split is the one worth quoting when the corpus totals look alarming:
a corpus counts everything and a person reads one directory.

## `array{...}|array{}` lost its keys three times over

The fourth cause, and the one where the same mistake was waiting at every layer. A project writes

```php
/** @return array{type: string, address: string, city: string, ...}|array{} */
public function getShippingAddress(bool $formatType = FALSE): array {
```

to mean "the whole thing, or nothing" -- `return [];` in one branch and the full record in the
other. It is the largest single family PHPStan reports on the reference project and phrost did not:
**29 of the 38 `Offset 'x' might not exist`**, every one of them written by hand in the project's
own `@return`.

Three collapses stood between that docblock and a reader, and each was a rule that knew one spelling
and not its neighbour:

1. **`array{}` became a bare `array`.** In `lower_doc_type`, with the comment "nothing knowable
   inside it is still an array" -- true of a shape whose fields failed to lower and false of one the
   author wrote empty, which are two different things arriving at one arm. And again in
   `TypeTable::shape`, which opened with the same `if fields.is_empty()`. Two constructors of one
   collapse.
2. **`has_arguments` did not count a union.** A docblock that says what a container holds beats a
   native `: array` hint, and `TypeRef::Shape` counted while `TypeRef::Union` of shapes did not --
   so the bare hint won and took every key with it. The comment beside the shape arm describes this
   failure exactly, one spelling earlier: "reading it as a competing claim let the bare `array` win
   and threw every key away."
3. **`keyed_of_held` read only `Held::OneOf`.** A union arrives in three spellings and a `@return`
   in another file is the _interned_ one, so the arm that took a key of a union spoke only for the
   spelling a docblock never produces. `element_of_held` opens with the line that fixes it --
   `union_operands` before the match -- and this now does too.

Any one of them alone kept the whole thing silent, which is why the first two fixes measured nothing
and looked wrong. The tell was arithmetic rather than reasoning: rendering the value through a
deliberate type error printed `array` where `array{a: string|null}` printed for a lone shape, and
that separated "the key read is broken" from "the type never arrived".

### What the empty arm means, and the guard that makes it liveable

PHP's own rule: reading any key off `[]` is `null` with a warning. So an empty-shape operand
contributes `null` to the key's type, and `$address['city']` is a `?string`. Every _other_ miss
still ends the read -- a populated shape that does not list the key may simply not list it, which is
the position `misspelt-key` is built on and is unchanged. `array{}` is the one spelling with nothing
left to be incomplete about, and that is the whole of what separates the two.

The guard is not optional. Measured before it existed: `if ($row === []) { return; }`,
`if (empty($row))` and `if (!$row)` all still reported, because none of them took the empty arm off.
`array{}` is **falsy**, exactly as `null` and `false` are, so it belongs in the same truthiness peel
-- one `without_empty_shape` beside `without_null`, on the branch that already removes the `false`.
All three spellings are one `PathRoot::NotNull` underneath, so one peel silences all three. The one
false positive this change produced on the reference project was exactly a guarded site, and it is
the finding that said the peel was missing rather than the rule being wrong.

### What it costs and what it buys

+2 on the reference project, 0 on all four corpora, both true and both PHPStan-corroborated:
`getMaxDiscount($address['type'], ...)` and `RegionData::getCode($shippingAddress['zip'])`. That is
far short of PHPStan's 29, and the difference is what the two tools claim: PHPStan reports the
_read_ ("might not exist"), phrost reports where the value it produces is then **refused**. Most of
the 29 feed `sprintf('%s', ...)` or an array literal, which take a null without complaint, so phrost
is right to be quiet -- the same split the `assumed-non-null` narrowing was about, measured the same
way.

The findings are the visible tip. What actually changed is that `array{...}|array{}` now carries its
keys everywhere -- hover, completion, goto, inlay hints and every downstream check -- where it had
been a bare `array` since the day the shape was added.

Serial `check` over the reference project went 4.12 s to 4.28 s, about 4 %, for the union read on
every keyed subscript and the peel on every truthiness guard. Diagnostics p95 12.3 ms against a 30
ms budget.

## PHP has four shapes of call and only three were call sites

`phrost_body::site_of` matched a method call, a static call, a function call and a `new`, and ended
`_ => return None`. `ExprData::IndirectCall` -- `$fn()`, `($f)()`, `$handlers[$k]()` -- has been in
the HIR all along and six other readers already use it; it was simply not a **call site**. So every
check about what a call passes went silent at the first closure, `$fn(` offered neither signature
help nor a parameter hint, and `$digitsOnly($customer->getPhone()['number'])` on the reference
project was judged by nothing.

The probe that found it is four rows, each differing from the control in one thing:

| how it is called                             | judged? |
| -------------------------------------------- | ------- |
| `$this->takes($x)`                           | **yes** |
| `$fn = static fn (string $v) => $v; $fn($x)` | no      |
| `function f(\Closure $fn) { $fn($x); }`      | no      |
| `$fn = $this->takes(...); $fn($x)`           | no      |

### Why it needed a seam rather than an arm

Three of the four shapes name something the index has **stored**, and their parameters arrive as
`ParamRow`s whose types are interned. The fourth names a _value_, and when that value is a closure
written in the same file its parameters live in that file's own arena and can never be interned from
a reading view. Adding an arm would therefore have meant a second path through `sentinel_arguments`,
`incompatible_arguments` and `written_back_arguments` -- three more places for one rule.

`compat::Expectation` is the seam that already exists for exactly this problem: its own doc says it
is what "every check that has the written expression in hand should use, and there are four of
them", because a `@var` in this file is an index into its arena where a property's type is an id. A
closure's parameter is the fifth site with the same problem and the same answer. So
`view::parameters::Parameter` is `Declared(ParamRow) | Closure(ParamData)`, it hands out an
`Expectation` rather than a `TypeId`, and the three walks, `paired`, signature help and the
parameter hints all read it. `expectation_admits` and `fit_written_value` already spoke both
spellings; what was missing was anything that _had_ both.

### What PHP said about the counts, and it removed a check rather than adding one

`missing_in` and `extra_in` both decline for an invoked call, and the runtime is the reason in both
directions:

```
function f($a) {}  f(1, 2, 3);          // fine
$c = function () {};  $c(1, 2);         // fine
(new K)->m(1, 2);                       // fine
```

Measured on 8.5.10: extra arguments to anything user-defined are silently ignored, so the upper
bound was never a runtime claim. And the lower bound would be a claim about a callee identified
through a chain of assignments, which is the least certain callee there is -- Laravel's
`Http\Factory` writes `$response = $callback;` then
`while ($response instanceof Closure) { $response = $response($request, $options); }`, and the
literal that chain lands on declares no parameters. The type checks degrade to silence on a wrong
identification; a count check asserts. That one false `extra-argument` is what the rule was written
from.

### What it found

+25 across five corpora: WordPress 0, Laravel 0, Slim 0, Symfony +23, reference project +2. The two
on the reference project are the sites PHPStan names. Symfony's are a
`$assertNormalizedProperties( ..., $normalizer->normalize(...))` fourteen times over,
`$injectToolbar($response->getContent())` where `getContent()` is `string|false`, and two `.github/`
scripts. One is wrong -- `$resolveName($id, $city)` where `$id = str_replace(':', '/', $zone)` reads
`array|string`, because nothing narrows `str_replace`'s conditional return when its subject is an
`int|string` array key. That is a precision gap this change _exposed_ rather than caused: the same
value passed to a declared method reports identically.

### The second source, added in the same pass

An **invokable object** is the other half, and PHP's own rule makes it a member lookup:
`$handler($request)` is `$handler->__invoke($request)`. Its parameters are stored, so they arrive as
`Parameter::Declared` and bring the class's templates with them exactly as a written
`$handler->__invoke(...)` would. One more finding on Symfony -- `$handler($msg)` where
`EarlyExpirationMessage::create()` is `?EarlyExpirationMessage` -- and it is the shape modern PHP
writes controllers and handlers in, so it will pay on any project using them.

### The third source, and it needed no field after all

A **first-class callable** looked like it needed the callee's path carried on `Callable`, which is a
field on a type that sits in every memo key. It does not: the path is already there, in the
_binding_. `$fn = $this->takes(...)` binds a value whose root is `PathRoot::Callable(of)`, and `of`
is the call that was never made -- receiver, then the method's return. `$fn = strlen(...)` is the
same shape with a free function at the root instead of a method at the last step. So
`parameters_of_callee` splits the last step off and asks the same `parameters_through` the other two
sources ask, and the walk that reads the binding is gated on the value already being a
`Held::Callable`, which an invokable object is not.

**Zero on all five corpora**, and kept anyway -- which is the opposite call from the two
element-write rules below, so the difference is worth stating. Those added findings elsewhere and
bought nothing where it mattered: cost without benefit. This adds no finding anywhere, costs no
measurable time, and closes a limit that was written down -- and it turns on signature help and
parameter hints for `$fn(`, which no corpus measures and a person sees every time they type one.

What is left is a callable with no declaration behind it: a parameter typed `callable` or `\Closure`
is whatever the caller passed, and a guess there is how a wrong parameter list gets into a report.

All seventeen budgets met, diagnostics p95 11.8 ms against 30, serial `check` 4.08 s against 4.28 s.

## A through-write was a flag, and two attempts at the wrong rule before the right one

`$order['Field2'] = json_encode($order['Field2']);` on the line above `strlen($order['Field2'])` --
`json_encode` hands back `string|false` and the `false` was not reported. Three attempts, and the
first two are worth as much as the third.

**First attempt: the `foreign` rule.** `element_written` refuses a slot write when the container is
a parameter, because "a parameter arrives full, so a write into one of its slots is one contributor
among however many the caller put there". True for `$row[] = $x`, and not for a write that _names_
the slot being read -- that one overwrites whatever the caller put there. Lifting it for a named
key: **+35 on WordPress, +2 on Symfony, and 0 on the reference project.** The WordPress findings
were mostly real -- `$redirect['path'] = parse_url($url, PHP_URL_PATH)` really does put a `null`
there -- but the site that started it did not move at all.

**Second attempt: the record rule.** `$order = [...]` makes a `Held::Record`, and `indexed_of_held`
answers from it and never reaches the write walk. Ranking a later named write above the literal: the
probe fired, and the reference project still did not move.

**What it actually was.** Three hundred lines above, `$order['Field2']['altCustomer'] = $screening;`
-- a write _through_ the container. `element_written` records that as a `bool`, and a `bool` has no
position, so **every** slot write for the rest of the body was discarded. The rule is right and its
shape was wrong: a through-write makes stale what came _before_ it, and a slot write after it
replaces the whole slot, taking whatever was one level inside with it. `Option<TextSize>` instead of
`bool`, and the filter compares positions.

Measured alone, with the first two attempts reverted: **+2 on the reference project** -- exactly the
two sites PHPStan names -- **+1 on WordPress**, and nothing on Laravel, Symfony or Slim. The two
attempts that measured nothing are reverted, and what they cost is the reason this is written down:
a rule can be correct, probe green, and buy nothing, because a _different_ rule is the blocker --
which §8 already records, and which cost two builds and two full corpus sweeps to meet again.

The tell, in hindsight, was available from the first measurement: the probe fired and the real site
did not, twice. That is the signature of a blocker rather than a missing rule, and the cheap next
move is to ask the real site what it holds -- `phrost query` said `array $order`, not a record and
not a parameter, which rules out both attempts in one command.

## The template leak was in the check, and the first fix went to lowering

`@template TValue` written on an interface, `@param TValue|null $value` on its method, and
`$t->transform(FALSE)` reported as
`Cannot pass false as $value, which takes ?\Symfony\Component\Form\TValue` -- a class nothing
declares. Recorded two notes ago as a leak in lowering: `doc_names_a_template` reads only the
_declaration's own_ docblock for `@template`, so a class's are invisible to it.

**Fixing it there was wrong, and the test suite said so in one run.** Threading the class's
templates into the refusal took five generics tests down and added twenty-one `undefined-method` on
Laravel, twenty on Symfony. The reason is in the sentence `doc_type_beside_with_aliases` already
writes: it refuses "a template nothing here binds". A _method's_ own template is one -- no receiver
can fill it. A **class's** is the opposite: the receiver is exactly what binds it, and the
substitution works by finding the name in the lowered type and replacing it. Refusing it at lowering
leaves nothing to substitute, so every generic class stops resolving.

So the type has to stay and the _judgement_ is where the question belongs -- and there it was
already answered, in one of the two places that ask it. `incompatible_arguments` has run every
expectation through `substituted_expectation` since templates existed: the receiver's arguments are
put in place, and `judgeable` refuses what is left when nothing bound them, because a `Named` that
resolves to nothing "rejects every scalar for not being a class and accepts every class because
nothing can be related to a name that resolves to none -- wrong in both directions".
`sentinel_arguments` read the parameter as written and never asked. Two constructors of one rule,
and `bound_expectation` -- added for the named-argument work one note up -- is the seam that now
serves both.

**−37 across three corpora and none added**: WordPress −12, Laravel −16, Symfony −9, Slim and the
reference project unchanged. Every one of them a message naming `\T`, `\TValue`, `\TSend` or
`array<\K, \V>`, which is a sentence no reader can act on. Among them are the two
`?\Symfony\Component\Form\TValue` findings the `{@inheritdoc}` note above had to write a `mixed`
rule to keep out -- so the stop that note added is now belt beside braces rather than the only thing
holding.

The general form, and it is the third time this document has recorded it from a different angle:
**where a wrong answer is produced is not where the rule belongs.** The leak was visible in lowering
and lowering was the wrong layer, because what lowering produces is load-bearing for a _different_
reader. The cheap test for which layer is the one the suite ran: a fix in the wrong one breaks
something that was working.

## A bare `object` receiver, counted rather than argued about

PHPStan reports 74 findings on the reference project about a member reached through a bare `object`
-- `Call to an undefined method object::customer()` and its neighbours -- and phrost says nothing at
every one, on the stated ground that "`object`, `mixed`, `iterable` and `callable` are all types an
object inhabits". That is a position, and a position is worth counting before it is argued about.

|                                                              |        |
| ------------------------------------------------------------ | ------ |
| a parameter or local the project declared `object`           | **69** |
| a `$this->x(): object` accessor whose body returns a service | **5**  |

The 69 are one idiom: `public function commercialOrderForm(object $order)` with
`@param object $order` above it, 32 of them in one file. Nothing is hidden there and nothing can be
inferred -- the author wrote `object` in both places. Reporting them is PHPStan's position and
staying quiet is phrost's, and the only thing that changes either is somebody deciding.

The 5 looked different and are not:
`/** @return object */ protected function utilitiesManager(): object { return \Drupal::service('utilities.manager'); }`
-- a declaration that says `object` twice above a body that knows exactly what it hands back.

### What was built for the 5, measured, not shipped -- and then shipped once the third piece existed

Two gates, because `object` is the widest object type PHP has and a class is strictly more precise:
lowering reads the body where every written source says only `object`, and the view prefers a
described body over an `object` hint -- the same rank `refined` already gives a _docblock_ beside
one, asked for the third source. Probed: `: object` with `new Manager()`, with `@return object`
beside it, and with `$this->factory->make()` all resolve where none did.

**Zero on every instrument.** Five corpora unchanged; the reference project's coverage identical to
the access -- 50 027 resolved, 117 `declared object` -- before and after. The reason is in the
fourteen `): object` methods that project has: four return `new class ... {}`, three return
`\Drupal::service(...)`, one returns `$this->entityManager->getRepository($entityClass)`. **Not one
of them names a class or a chain this could carry.**

What the three service accessors need is `ReturnedPathData` carrying a catalog key: a static call
with a literal argument, which is a stored shape and so a codec change and a cache version. Priced
rather than built at first, because the two gates that would feed it measured zero without it and
shipping them would be shipping a rule for a shape that does not occur.

### The key, and what it cost

`ReturnedRow` was built for `SqlBase::select()` -- a body that names no type and _describes_ one: a
root and a list of member names. `return \Drupal::service('utilities.manager');` is the same idea
with the class in none of those places and all of it in the argument, because the chain describes
`Drupal::service()`, which describes every service there is. So the row carries the first step's
first literal -- what a catalog contract means by "the argument" -- and `walk_returned` asks the
catalog before it asks the member, because `service()` is declared `object` and improving on that is
what the contract is for.

The first step's and no other. A key deeper in a chain decides a class the walk has already resolved
its way to, and carrying one per step would put a list on every row for a shape nobody writes: the
idiom is a one-step chain off a container.

|                                    |                                                                                                                                 |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| index heap, reference project      | 104.0 MB -> **104.1 MB**                                                                                                        |
| member accesses resolved           | 50 027 -> **50 037**                                                                                                            |
| receivers stuck on a bare `object` | 117 -> **111**                                                                                                                  |
| findings                           | **+1**, and it is `$this->utilitiesManager()->fillPdfAsync(file_get_contents($path), ...)` -- a `string\|false` into a `string` |
| five corpora                       | unchanged                                                                                                                       |

The +1 is the whole argument in one line: the receiver was an `object`, so the call through it was
judged by nothing, and the `false` went in unremarked. Six fewer `declared object` receivers is
three accessors times the calls made through them, and each of those now has goto, hover and
completion where it had none.

What is still not reached is the other eleven `): object` methods on that project -- four returning
`new class ... {}`, one returning `getRepository($entityClass)` with a _variable_ argument. An
anonymous class has no name to store and a variable key is not a literal; both are honest limits of
a stored description rather than missing rules.

The general point is the one about where a count comes from: 74 reads as a large gap and is 69
declarations somebody wrote on purpose plus 5 that a stored-shape change would reach. Neither number
is an argument for the position; both are what an argument about it would have to start from.

## A reduction that looked more specific was hiding five rules, and one of them was in it

`$row = $this->find(); if ($c) { $row = new Row(); }` held a `Row`. The join reduces operands that
all name one class, because `Row|Row` is a union nobody wrote and every reader that takes a union
apart pays for an operand that adds nothing -- and `?Row` beside `Row` names one class, so the
reduction kept the write and threw the null away on the path the branch did not take. Nothing
failed. The answer looked _more_ specific than the truth, which is why it survived a year.

The fix is one line of intent: where the operands disagree about the null, keep a nullable one. The
reduction is a claim about the class and was never meant to be a claim about whether the value is
there. `??` and `?:` are unaffected, because their left operand has its null taken off by the
operator before the reduction runs -- `load() ?? new Row()` really is a `Row`.

What that cost is the interesting half. **The reduction was the last line of five rules, and it had
been covering for the four above it.** Each of them answered "nothing proved" about the path that
_skipped_ a write, and each was invisible because the reduction then dropped the operand they had
failed to narrow. Unmasked one at a time, on five corpora:

- **The skipped path was read as the complement of what entering proved, and those are different
  questions.** Entering `if ($best === NULL || $row->min() < $best->min())` proves nothing, because
  either disjunct may be the one that held; _skipping_ it proves `$best !== NULL`, because De Morgan
  says both failed. Every find-the-best loop in PHP is written that way. `Scope::narrowing_skipping`
  reads the condition at the skipped polarity instead, which subsumes the old complement -- a
  `!$x instanceof T` guard answers `OfClass(T)` either way -- and answers the `||` besides.
- **What a branch contributes is the value where control _leaves_ it, not what the assignment put
  there.** `$em = $this->find(); if (!$em) { throw ...; }` hands the join a non-null, and reading
  the write alone handed it the `?T` the call declares. Symfony's `UniqueEntityValidator` is written
  exactly so, and eight findings on it were the tell. `MaybeWrite::ends` carries the branch's end
  and the narrowing in effect there wins, when it is later than the write -- which is what tells it
  apart from the branch's own entry guard, that being about the value the assignment replaced.
- **A loop write's skipped path includes one where the loop never ran.** Two spellings: the read
  standing outside a loop the write is in, and the read standing at the top of the body with a
  carried write below it. Both mean the value from before the loop gets through untouched, so no
  guard written inside describes it.
- **A guard whose claim is written on the callee had no spelling.**
  `if (!is_wp_error($thing)) { $thing = new WP_Error(); }` is how WordPress normalises one, and
  `@phpstan-assert-if-true` is where the class comes from -- but the vocabulary a skipped path is
  stated in holds a null, a class and a kind and nothing for a tag. `Otherwise::Asserted` carries
  the whole path instead and the reader resolves it. Seven findings on `wp-login.php` alone.
- **And a condition that is an assignment was matched only when the assigned name was the subject.**
  `if ($handlerIsNew = null === $handler) { $handler = new static(); }` is Symfony's
  `ErrorHandler::register`, and the truth of an assignment is the truth of what was assigned. Only
  where the value is itself a _test_: `if ($refund = $order->getRefundInfo())` tests what the call
  returned, and reading it as a claim about `$order` makes the call's receiver the subject of a test
  about its result -- which displaced the `instanceof` beside it and lost a method the subclass
  declares.

Two more that the same session settled and that belong beside these. **Two skipped-path claims that
differ used to cancel out**, and an `if`/`elseif` chain normalising a union produces exactly that --
Symfony's `FixedWindowLimiter` replaces `$window` when it is not a `CalendarAlignedWindow` and, in
the next arm, when it is not a `Window`. Neither claim survives the other and what both say is that
the value was there, which is the only part of the meet there is a spelling for; seventeen findings
on one file. And an **assertion cannot be compared with a claim in this vocabulary at all**, so
where one of the two can be stated it is kept rather than the pair cancelling -- sixteen findings on
`SlidingWindowLimiter`, which is the `FixedWindowLimiter` shape with `isExpired()` in the second
arm.

Measured, five corpora, every new finding read: the reference project **+5, all true and two of them
PHPStan's own**; Laravel +7, all `possibly-null-callee` on `findOr($id)` shapes where calling
without a callback and not finding the row is a fatal; Symfony +19; WordPress **-25**; Slim
unchanged. Budget untouched -- `phrost check` over the reference project measured 1.98-3.18 s
against a 2.34-2.89 s baseline, and all seventeen request budgets met.

The honest remaining limit is that `Otherwise` carries **one** claim, so a guard that proves two
things about the path around it -- `if (!$rows || is_wrong($rows))` -- carries whichever the walk
found and loses the other. Four findings on WordPress are that, and the shape to reach for when it
matters is a variant that holds several.

## `$x === 'draft'` proves `$x` is not null, and only the relational half of that was written

`CompareOp::null_holds_whatever_it_faces` answers for `>` and `<=` -- `null > x` is false for every
operand and `null <= x` is true for every one -- and answers `None` for the equality operators,
because whether null satisfies one depends on what it faces. That `None` was read as "nothing to
say" rather than as "ask the operand", so a guard comparing a nullable against a literal narrowed
nothing at all and every use below it was reported.

Only the strict spellings, and PHP is why: measured on 8.5.10, `null == 0`, `null == ''` and
`null == []` all hold, so `$x == 0` says nothing about null -- while `null === <anything but null>`
is false. And only the operands whose _spelling_ settles it: a literal that is not `null`, an array,
a `new`, and an operator whose result the operator itself decides.

A class constant is the interesting refusal. `Stage::CONFIRMED` is an enum case and can never be
null, but the layer that reads guards has no index and `const X = NULL;` is legal -- 7 of 4 225
class constants across the five corpora are one. Measured by admitting `ClassConstAccess` anyway, it
is worth **one finding** on five projects, which is not what an unsound claim costs. What would earn
it is a claim that carries the other operand's _type_ rather than a guess about its nullability, and
that is a `Proved` variant nothing needs yet.

Three finds away: `in_array($x, [Stage::A, Stage::B], true)` and `switch ($x) { case Stage::A: }`
narrow nothing either, for the same reason and with the same fix available.

## `possibly-absent-key`: the read PHPStan reports and phrost had nowhere to say

phrost reports a key that may be absent where the value is _used_ -- `$address['city']` passed to a
`string` parameter is a `possibly-null-argument` -- and PHPStan reports the **read**. On the
reference project twenty of twenty-eight `offsetAccess.notFound` findings are keys read straight
into an `array<string, mixed>` argument, so there is no use to report and phrost said nothing about
code it had fully understood: `phrost query` at those sites renders the union with its empty arm.

So the check is the read, and only where the declaration _states_ the absence. Two spellings say it:
`array{}` as one arm of a union -- which is how a project writes "the whole thing, or nothing" --
and a field written `key?:`. A populated shape that simply does not list a key still says nothing,
which is the position `misspelt-key` answers under; an empty arm beside it must not overturn that,
so the empty arm speaks only for a key some _other_ arm names.

Four things were needed to make it liveable, and each was measured:

- **One site per key, not per value.** `subscript_sites` keeps the first read of each value in each
  scope, because what a value holds is one fact and twenty-nine reads of it are one thing to say.
  Whether a _key_ is declared is a fact about the key, so `keyed_subscript_sites` is the same
  collection kept one per key.
- **The three ways a body tests for a key all silence it**: `isset($row['vin'])`,
  `array_key_exists('vin', $row)` and `$row['vin'] ?? NULL`. Per _scope_, which is the reading
  `existence_tests` takes of the same question and for the same reason -- the guarded read is often
  nowhere near the test. Per file it must not be: a local is a different value in every method, and
  a generated template that coalesces `$address['address']` in twelve of them silenced the
  thirteenth, which is a finding PHPStan reports and this dropped.
- **The superglobals are out.** `$_SERVER` declares nearly every key optional and rightly, but that
  shape is the runtime's documentation of what it may provide rather than a claim an author made.
  Measured before the rule existed: **105 of WordPress's 117 findings** were `REQUEST_URI`,
  `HTTP_HOST` and their neighbours, which is what a check nobody would leave on looks like.
- **And one real gap it exposed.** `$a = $this->parts(); if (empty($a)) { $a = [...]; }` leaves the
  populated arm on both paths, and four findings said otherwise. `PathRoot::NotNull` takes the empty
  shape off along with the `false` and `Otherwise::NotNull` -- the _skipped_ path's half of the same
  rule -- did not. §8.1's oldest form, and the fix moves nothing on four corpora and four findings
  of a check that reads shapes.

Measured with it forced on: the reference project **25**, of which 18 are PHPStan's own sites;
WordPress 22, Symfony 30, Laravel 0, Slim 0. Asked for rather than always on, because a project that
reads a `key?:` without a guard has done nothing PHP objects to -- the read is null and the code may
want that. `phrost.toml`'s `on = ["possibly-absent-key"]` turns it on.

The ten sites PHPStan reports and this does not are other shapes: a positional
`array{}|array{0: string, ...}`, an `array|null` container -- which is `possibly-null-subscript`'s
question, not this one -- and a computed key whose type is a union of literal strings.

## A local is a different value in every scope, and one set of names was not

`$element = $form_state->getTriggeringElement();` is a `?array`, and `$element['#array_parents']` on
the next line was reported by nobody. phrost knew the type -- hover renders `?array $element` at
that exact position -- and `possibly-null-subscript` fires on the same three lines in a scratch
project. What silenced it was **another method of the same file**: a Drupal widget's `formElement()`
writes `$element['#prefix'] = '<div>'`, and `write_targets` kept "every value the file writes an
element of" in a set keyed by the [`ValuePath`] alone. `Local("element")` is one key however many
methods write it.

The rule the set exists for is right -- a write _creates_ the container, so the declaration before
it says nothing about what is there -- and it is right per **body** for a local. A property is the
other way: `$this->rows['k'] = 1` is the same slot wherever it is written, so it keeps the file-wide
reading it always had. So the key is `(Option<BodyId>, ValuePath)`, with the body carried only for a
local.

Measured: the reference project **+1**, and it is the site PHPStan reports as
`Offset '#array_parents' might not exist on array|null`. WordPress +12, Symfony +8, Laravel +4, Slim
0 -- every one of the twenty-four read and every one a genuine `foreach` over an `array|false` or a
subscript of a `?array`: `parse_url($uri)['query']`, a `preg_split` result, a `?SplObjectStorage`.
`phrost check` over the reference project is unchanged at 2.2-2.4 s.

Two readers shared the set and both had the leak -- `subscript_sites` and `iterated_subjects` --
which is why the `possibly-*-iteration` rows moved too. The same shape was fixed in `tested_keys` an
hour earlier in the same session for the same reason, which is §8.1's oldest form arriving twice in
one afternoon: **when a fact about a local is stored without the scope, it is a fact about every
scope.**

## A guard on a property reached the call and not the local the call filled

```php
if (!$this->order instanceof PremiumOrder) {
    return [];
}
$signupInfo = $this->order->getSignupInfo();
$members = $signupInfo['members'];
```

Goto and hover on `getSignupInfo` answer correctly -- `PremiumOrder`, returning
`?array{date: string, …}` -- and `$signupInfo` one line down had **no type at all**. The two answers
come from different walks, which is §8.1's oldest form: `chain_held` asks `slot_value` about the
_receiver_ and so sees the narrowing, while `resolve_path` asks it about the path **as a whole**.
For `$this->order` that is the guarded slot and the answer is right; for
`$this->order->getSignupInfo()` it is a path no guard is written about, and the step walk then read
the property's _declared_ type and found no such method on it.

So the step walk asks the same question of each prefix that ends in a property -- the only step a
guard can name -- and prefers a guard over the declaration exactly as `resolve_path` does for the
whole. Only a guard, never a write: a write to the slot is what the declaration-reading walk is
already producing, and preferring it here would read it twice.

The finding count understates this. On five corpora it is four new diagnostics; what it actually
buys is that hover, completion and every check see the local at all -- `$signupInfo` renders its
whole shape now where it rendered "no declared type". `phrost check` over the reference project is
unchanged at 2.2-2.8 s.

Of the four: the reference project's is the site PHPStan reports as
`Offset 'members' might not exist`; two are amphp's
`ReadableResourceStream::getResource(): ?resource` passed to `fseek`, which only became visible once
the `instanceof` narrowing reached the call. The fourth is a **false positive and a pre-existing
limit the change unmasked**:

```php
if (!$this->inner instanceof MessageTypeAwareSerializerInterface) {
    $type = $envelope->getMessage()::class;
} elseif (null === $type = $this->inner->getMessageType($encodedEnvelope)) {
    throw …;
}
$this->shouldSign($type);
```

Both paths assign `$type`, but one of them assigns it **in the `elseif`'s condition**, and
`writes_fill_every_arm` counts arms rather than conditions -- so the parameter's `?string` joins in.
It is the honest shape to fix if the pattern recurs: a condition evaluated on exactly the else path
is an arm that writes.

## `mixed|null` is `mixed`, and three readers took it apart

PHP will not compile `mixed|null` or `?mixed` -- "Type mixed can only be used as a standalone type",
run against 8.5.10 -- because `mixed` already holds every value. A docblock writes both, and every
reader that answers a sentinel question walked the union operand by operand: the null beside the
`mixed` became a claim of its own, where a bare `mixed` claims nothing. WordPress's
`@return mixed|null` on `WP_REST_Request::offsetGet` made **136** `$request['url']` possibly-null
arguments the moment the subscript was read; Laravel's `Authenticatable|mixed` refused eight nulls.

Three readers, which is §8.1's first form: whether a value can be a sentinel (`held_absorbs` and its
written and interned twins), whether a declaration admits one (`type_admits`, `expectation_admits`),
and the compatibility judgement, which split `?mixed` into a refused null and an unknown rest. All
three learn it; the union itself is **not** collapsed. Collapsing it in lowering was tried first and
broke `@return Row|mixed` completing as a `Row`, which a test already pinned -- the weaker claim is
the true one for a _judgement_, and the class is still what a reader navigating wants.

Which made one refusal in `TypeTable::refined` stale. A docblock mentioning `mixed` beside a native
`mixed` was dropped as noise, because taking `array|mixed|null` once turned into a nullable every
argument was reported on; with the readers folding the union that cannot happen any more, and the
refusal was only costing navigation. BookStack's `setting(): mixed` writes
`@return mixed|SettingService`, and its 119 `setting()->get(...)`-shaped calls resolved nothing.
Taken now where the docblock names a class beside the `mixed`: BookStack 97.3 -> 97.6 %, and no
finding moved on any of ten projects -- the judgement half, checked rather than assumed.

## An enum is a `UnitEnum`, and the hierarchy did not know

The member table has merged `UnitEnum`'s and `BackedEnum`'s members into every enum since enums
existed (`merge_enum_interfaces`); `ClassHeader`, which every hierarchy question reads, listed only
what the source wrote -- and PHP refuses `enum E implements UnitEnum`, so nothing ever did. So
`takes(\UnitEnum $case)` refused **every enum there is**, and a `\UnitEnum|string` parameter was
silent only because the string arm could not rule an enum out. It surfaced the moment a closed class
started being refused as a `string`: two constructors of one rule, found by one of them changing.

`ClassHeader::implied` carries the interface, apart from `implements` -- a hover card shows what was
written, and the hierarchy reads both. Fixtures do not index the stubs, so a test declares
`interface UnitEnum {}` itself.

And the rule that exposed it has an exception worth knowing: a **stub's** final class is not closed
the way a PHP one is. `GMP` is final, declares no `__toString`, and `f(gmp_init(5))` into a `string`
works -- its cast handler is C, and a stub describes methods, not handlers. So `class_answers_kind`
answers no only for a final class written in PHP.

## `bool` is `true|false` spelled once, and ruling out one half kept both

Every sentinel narrowing works by taking a union apart and dropping the operands the test ruled out
-- so `int|false` past `?:` or `!== false` is an `int`, and a `bool` past the same test stayed a
`bool`, because there was no `false` _operand_ to drop. That is §8.1's degenerate form exactly: the
guard ran, answered, and changed nothing. `bool_without` (in `of_kind`) is the one rule, for a lone
`bool` and for one inside a union.

It hid behind something else. `$handler ?: SIG_DFL` over a `bool|resource` read as "may be false"
only once `SIG_DFL` had a type -- before global constants were typed the whole `?:` resolved to
nothing and nobody judged it. Fixing it moved ~40 corpus findings from `possibly-false-argument` to
`incompatible-argument`, and that is the rule working: the `false` warning was the first thing the
check said and it stopped there, so a `bool|array` into an `array` never reported that the `true`
half is a `TypeError` too, and Symfony's `getParameter()` (`array|bool|string|...`) into a `string`
never reported the `array`. Read the moved findings before accepting: every one sampled was a
declaration saying what the check now says.

## A docblock beside a nullable hint lost either the element type or the null

`/** @var DOMNamedNodeMap<DOMAttr> */ public ?DOMNamedNodeMap $attributes;` is how the stubs write
`DOMNode::$attributes`, and it resolved to `?DOMNamedNodeMap` -- the element type gone, so every
`foreach ($el->attributes as $attr)` read `$attr` as a bare `DOMNode` and `$attr->value` reported
`undefined-property` (WordPress's SimplePie, twice). The cause was an arm order: `argues_about`
matched `(_, Generic { base })` first and compared the base against the `?Map` wrapper, so the
`Nullable` arm below it never ran. Reordering alone would have been worse, because the docblock then
wins and **drops the null the runtime allows** -- which `?array $ids` beside `@param list<int> $ids`
already did, silently, everywhere. Both halves are one claim, so lowering now puts the `null` back
on a docblock that says what the value holds (`keeping_the_null`), before either reader of the
declaration sees it.

Two exclusions, each found by a test or a corpus rather than foreseen: a `mixed` hint admits `null`
and is not a nullable -- `@return T` beside `: mixed` is not `?T` -- and a bare name is left alone,
because beside `?NodeParentInterface` it may be a class template lowering cannot see, whose argument
settles its own nullability (Symfony's `->end()` chains, five findings until it was excluded).
`@param-out` is excluded too: what is written back is not bounded by what the hint accepts. What the
change added on the corpora is nullable declarations read as nullable -- Symfony's
`public ?array $groups = null` with `@var string[]`, twelve findings, all of them `null` the code
really can hold.

## A `@mixin` was read as an ancestor in four places, and a loop through one as a broken hierarchy

Eloquent's `Model` forwards every call it does not declare to a fresh `Builder<static>` from
`__call`, so a framework states it as an _implied_ `@mixin` (`ImpliedMixin`) and the class is read
as if it carried the tag. Turning that on moved Laravel's "incomplete hierarchy" count from 123 to
2544, and the cause had been there all along: `Contracts\Eloquent\Builder` is `@mixin` the very
`Eloquent\Builder` that implements it, so the flatten walk met a class already on its stack and
marked the table incomplete. A loop _through a mixin_ is not broken code -- the class it skips is
the one being built, and its members land in that table. It is now a **cut**: not incomplete, not
memoized for another walk, healed when the walk leaves the class it was cut at. 123 -> 3.

A forwarded member was also indistinguishable from an inherited one, so four readers treated the
mixin as part of the hierarchy PHP checks. `bad-override` compared a model's own `where()` with the
builder's (31 findings on Laravel, 15 of them from _written_ tags that predate this);
`#[\Override]`, override completion and "not implemented" would have done the same. And
`inherited_ty`, climbing for a declaration, took `find()` off the interface's mixin --
`Query\Builder::find(): ?stdClass` -- for every `Eloquent\Builder::find()`, which is 19 findings
about `stdClass` that were all wrong. `ResolvedMember::forwarded` is the one bit all of them ask
now.

Two things the change needed from the readers. What a forwarded call returns is the mixin's method
_on the mixin's instance_: `User::where()` returning `$this` is a `Builder<User>`, never a `User` --
so both declaration readers delegate to the mixin with the model as its argument. And the
conditional-return reader substituted a receiver's arguments into a bare template only, where the
declared-return reader had a whole ladder (`$this`, `list<T>`, unions, `?T`, `Collection<T>`): that
is the _two constructors_ shape, and `substituted_by_receiver` is the ladder both now go through.
`find()`'s own test, `$id is (Arrayable<array-key, mixed>|array<mixed>)`, was refused whole because
`array<mixed>` looked like a claim about contents and a class beside a kind is no test the
vocabulary had; `OfKindOrInstance` is that test, and an `int` passes neither half.

What moved on Laravel is typed code speaking: 199 `possibly-null-receiver` on `->first()`,
`->fresh()` and `::find(1)`, each declared `?Model` -- PHPStan says the same at level 8.

## `$this` in a macro was the service provider

`Collection::macro('x', function () { return $this->count(); })` is written in a provider's `boot()`
and runs as the collection: `macro()` is `@param-closure-this static $macro` and binds it before the
call. Reading `$this` as the class the closure is written in reported 34 `undefined-method` and
`incompatible-argument` on Laravel -- `FoundationServiceProvider::validate()`,
`SleepTest::duration()` -- every one a call on the macro's real `$this`. The tag is a fifth type per
parameter (`ParamRow::closure_this_ty`), and `IndexView::closure_this` asks it of every closure
around the offset, innermost first, because a closure inside a bound one captures its `$this`. The
bound class is the _scope_ too: `bindTo($this, static::class)` is how `macro()` binds, so a macro
reading a protected member of what it was registered on is not `inaccessible-member`, which it
became for one run before `scope_at` learned the same answer.

## A docblock an override carried lost the class that wrote it

`class G implements Getter { public function get(): mixed {} }` under `/** @return T */` carries the
interface's docblock onto the override -- `Flattener::insert`, the `{@inheritdoc}` rule -- and
carried the _type_ without its _owner_. `T` was then looked up among the implementing class's
templates, found none, and rendered as a class called `T`; an `@implements Getter<Item>` above never
reached it. The same override with no native hint answered `Item`, because `inherited_ty` climbs to
the declaration and returns who wrote it: the _two constructors_ shape again, one path carrying
where a type was written and the other not. `ResolvedMember::doc_from` is the owner, chained through
each carry. On Laravel, `$this[Kernel::class]` on the `ArrayAccess` container read as a class `V`
110 times and now reads as nothing supplied; on Symfony, Monolog's
`@template-implements ArrayAccess<..., int|string|\DateTimeImmutable|array<mixed>>` reached
`$record['extra']` for the first time and 13 `incompatible-argument` findings in its tests are that
declared union speaking.

## A container of a template was substituted only when the value carried arguments

`@return Coll<int, TModel>` reached through a receiver that supplied nothing answered _nothing_:
`applied_with` returned early on an empty argument list, so neither an `@extends` between the two
nor the template's bound was ever asked -- while a bare `@return TModel` beside it fell to the bound
correctly, and `list<T>` had its own copy of that fallback. In a union the slot kept the name, so
Laravel's `Factory::create()`, `Collection<int, TModel>|TModel`, handed `$users[0]` a class called
`TModel`. One slot rule now (`template_or_bound`) serves the container, the list and each operand of
a union. Laravel resolved 87.1 -> 87.3 % (+410 accesses). Two things surfaced behind it: a method
reached only through a `@mixin` is answered by `__call` and is no claim about an argument _count_
(`->map->toArray($request)` on the higher-order proxy), and `possibly-null-argument` named the
declaration (`$key takes \TKey`) where it had judged against the substitution -- both argument
checks now render one `Expectation`.

## Three more readers of a declaration that skipped the substitution

After the two notes above, coverage's _top unknown classes_ still listed template names, and each
was one more reader of a supertype's declaration that did not go through the ladder. An override's
untyped parameter under `@param TSet|null $value` climbed to the declaration, kept its owner, and
returned the type unsubstituted. A conditional return's branch gave up when the receiver carried no
arguments, and `names_something_real` passed `?TRelated` because it tested only a bare name -- the
_degenerate form_ of §8.1 again. And a callback's accumulator written
`TReduceInitial| TReduceReturnType` passed a guard that refused only a single undeclared name.
`declared_through` is now the one ladder for a member's type, an inherited parameter and a branch.
Left open: `class-string<T>` is not substituted at all -- `$reflection->getName()::m()` reads `T` --
because `Held` has no form for a class-string and adding one is a vocabulary change (5 sites on
Laravel and Symfony, no finding depends on it).

The instrument worth naming: **`phrost coverage`'s top unknown classes, read for names that look
like templates**, found all five of these in one listing. A leaked template is never reported by a
check -- it resolves to no class, so every check declines -- and the listing is the only place it
shows.

### And a fourth: what a callback is handed

The same listing, a year on, on three Laravel applications: `Illuminate\Database\Concerns\TValue` 19
times on Monica and twice on BookStack. `chunkById(callable(Collection<int, TValue>, int))` is
written on the `BuildsQueries` trait, which Eloquent's builder uses as `BuildsQueries<TModel>`, so
on a `Builder<Book>` the callback is handed a `Collection<int, Book>`. `first()` beside it,
returning `TValue|null`, already went through the ladder; the callback's parameter was read off the
signature as written, so `$books` was a collection of a class called `TValue` and every `$book->` in
the loop an unknown class. A callable's parameter is a fourth reader of a declaration, and now goes
through `declared_through` with the receiver's arguments and the class whose `@template`s the
signature names (`CalledDeclaration::owner`). Two things came with it. A static call a model
forwards to its builder -- `User::chunk(100, fn ($users) => ...)` -- is the builder's method on a
`Builder<User>`, which the return path already read through `implied_forward` and the callback did
not: the last six of Monica's. And `callable($this)` on a `Coll<Item>` now hands back `Coll<Item>`
where it handed back `Coll`, which is what the same receiver's `@return $this` answers.

Laravel +85 accesses resolved, Monica +37 and BookStack +7; the leaked name is gone from all three
listings. What the newly typed callbacks made the checks say was the precision kind --
`getRealPath()` returning `string|false` into `Str::after()`, a `{--path=*}` option's `null`
elements, `newFromBuilder()` documented `array<string, mixed>` and handed a cursor's `stdClass` --
with one exception, two findings on Laravel's `Str::password()`: `->filter()` with no callback drops
the falsy elements, and only `array_filter` is known to (`filtered_elements.rs`). And one false
positive it exposed rather than made:
`method_exists($guard = $app['auth']->guard(), 'setDispatcher')` vouched for nothing, because the
check compared the argument's path with the receiver's and an assignment has none -- it now looks
through to the name it wrote, which also cleared the same shape in Symfony's
`DebugHandlersListener`.

## A numbered entry beside a written key is a key of a map

A literal's integer key says nothing about the key type a writer meant -- most are PHP numbering a
bracketed list, and the record cannot tell `[0 => $a]` from `[$a]` -- so `['a', 'b']` handed to an
`array<string, V>` parameter is not reported, where PHPStan does and is noise on framework code.
That position stands. What it was too wide for is the hybrid: `['disabled', 'href' => $url]` writes
a string key, so it is a map, and its positional entry is an int key of that map on every run --
neither `list<string>` nor `array<string, V>` holds it, and a union of the two did not reject it
because the map half forgave the int. A literal's keys are all there, so one the expectation cannot
hold is rejected outright. Four findings on seven corpora, every one a declaration its caller does
not keep (`iterable<string, ...>` handed `['A', 'B' => [...]]` in Symfony's own tests, a `'123456'`
key PHP turns into an int), none on an application; typing-conformance 96.0 -> 96.4 %.

## One argument to a key-and-value container is the value

`@return Collection<Page>` gives Laravel's `Collection<TKey, TValue>` one argument, and read by
position it filled `TKey`: the page was the key and the value nothing, so
`foreach ($chapter->getVisiblePages() as $page)` typed nothing -- and neither did the Blade
templates that loop is handed to, which is how it was found: PHPantom offered 792 completions on
`$page->` in BookStack's export partial and phrost none. PHPStan, asked on BookStack's own checkout,
answers `Page` for the value and `int|string` for the key, and `mixed` for `$pages->first()` -- the
lone argument is the value for _iteration_ and nothing more, which is the reading
`@implements IteratorAggregate<Command>` already had here (`IteratedSlot::written_position`, one
rule for the stored, the written and the applied spelling). The key it does not say is no claim,
where it was a wrong one. BookStack +23 accesses and two findings gone; nothing else moved, because
one-argument collections are an older style -- eleven on BookStack, four on Laravel, none on Monica.

## A refinement lowering drops cannot settle a conditional return

PSL writes `Iter\first` as `@return ($iterable is non-empty-array|non-empty-list ? T : T|null)`, and
its own static-analysis fixtures call it on a function declared `@return non-empty-list<int>`.
phrost reports `possibly-null-argument` at all six such calls in
`packages/iter/tests/static-analysis/`, and PHPStan does not.

The test lowers to `ConditionTest::NonEmpty`, which settles only on a guard proving the argument
truthy -- the rule that keeps `reset($rows)` under `if (empty($rows)) { return; }` from being
`false`. A _declaration_ saying the value is non-empty is evidence of the same thing, and it never
arrives: `non-empty-list<int>` lowers to `list<int>` and `non-empty-array` to `array`, so by the
time a call asks, the two spellings are one and the claim that settled the branch is gone. §8.1's
_two spellings collapsing into one_, and the fix belongs where the collapse is -- keeping
non-emptiness through lowering -- rather than in the condition, which has nothing left to read.

Found by the psl ratchet on 2026-10-06, beside the `impossible-use` that 6a561bc3 fixed; the rest of
what that ratchet showed rising was real: `resource|false` passed on unchecked out of `fopen()` and
`brotli_compress_init()` (29), and `get_version_details()` returning `PHP_MAJOR_VERSION`'s `int`
under a `non-empty-string` it documents. The baseline was re-taken with these six known.

## A template's default is read for a method and deliberately not for a class

`@template TDefault = never` beside `@param TDefault $default` and a parameter written
`$default = null`: PHP passes the `null`, but the template was declared to be _nothing_ when the
argument is left out -- that is the whole reason to write the default. phrost read the parameter's
default instead, so `$map->get('k')` answered `?TValue`, and every method called on it was a
`possibly-null-receiver`. `mago-prelude` 1.52 rewrote `Ds\Map::get` from a conditional return into
exactly this shape, which is what made it a prerequisite rather than a nicety. The template's
keyword default now wins over the parameter's (`left_out` in `argument_binding.rs`), and the dump
renders the stored answer -- `or never when left out` -- which it did not before:
`ArgumentType::absent` was a stored field no invariant could see.

It moves nothing on the corpora today -- 656 000 answers byte-identical on Laravel and Symfony --
because the `= never` defaults there sit on templates bound by a callable's return, a different
branch.

The **class** half was built and taken out again, and the reason is worth more than the code. A
class template's default is right for a type _written without arguments_, and phrost cannot tell
that apart from two things the default is wrong for. Inside the class's own body the template is
open -- `$this` is a `NodeBuilder` of every possible `TParent` -- so Symfony's
`@template TParent of ...|null = null` made `$this->parent` the default `null`, and the guard
`null !== $this->parent` beside it left nothing. And a slot supplied with something phrost does not
read is not a slot supplied with nothing: Symfony's config builders return
`ArrayNodeDefinition<$this>`, the `$this` argument is unread, and the default turned 38
`undefined-method ... NodeParentInterface::scalarNode()` findings into 39
`null-receiver ... on null`. Both were false; the bound's was the _less confident_ wrong answer,
which is the one §8.1 says to prefer. The defaults that actually differ from the bound in five
corpora are those fourteen Symfony ones -- `TValue = mixed` and `T of object = object` say what the
bound already said.

So the blocker was the unread argument, not the default -- and the first diagnosis of _why_ it was
unread was wrong, which is worth recording. `$this` as a type argument was read; a three-line probe
said so at once. What lost it were two other things, each found by a matrix rather than by reading:

- **Two `@extends` hops with a bound.** The chase up a hierarchy translated a parent's template into
  the class's own through `substituted`, which answers a value with _no_ arguments and so, landing
  on a template, hands back its bound. `ScalarNodeDefinition<NodeBuilder>` reaches
  `NodeDefinition::end()` through `VariableNodeDefinition`; one hop or no bound worked, and only the
  combination failed -- a template without a bound got through by accident, because every class in
  the chain names it `TParent`.
- **`static` inside an argument on a receiver with arguments.** `children(): NodeBuilder<static>`
  resolved the word to the bare class, so `ArrayNodeDefinition<NodeBuilder>` came back as a builder
  of a plain `ArrayNodeDefinition`, and the `end()` that climbed out fell to the bound. The
  whole-return `@return $this` had handed the receiver back with its arguments for a long time; one
  level in, nobody had asked.

Both fixed, Symfony's config chains resolve through their nesting and 35 of the 38 are gone. The
three left are the frontier moving rather than new: `prototype('array')` picks its branch with
`T is 'array'`, a conditional on a _template_ bound by the string argument (since read, and those
three are gone too).

**Revisited with both fixed, and still not taken.** The own-body half has a clean answer: `$this` in
a class whose templates declare a default resolves to the class _applied to its bounds_, so the open
template answers its bound while a type written elsewhere without arguments takes the default -- and
`NodeBuilder::append()`'s `$this->parent` stopped reading `null`. What is left is the default doing
exactly what Symfony declares: `addConfiguration(NodeDefinition $builder)` with
`@param ArrayNodeDefinition $builder` is an `ArrayNodeDefinition<null>`, so `$builder->end()->end()`
calls a method on `null`. True to the declaration and wrong about the code, which relies on the node
being nested at runtime -- and the bound's `NodeParentInterface|null` is the _less confident_ of the
two wrong answers, which §8.1 says to prefer. Measured over five projects: one false finding gone,
two made more confident, none added that is true. The working patch -- `TemplateRow::default`, the
cache field, `unsupplied()`, and the applied `$this` -- is cheap to re-apply the day a project's own
code leans on a class default.

### `TreeBuilder::getRootNode()` needs a literal type, and is priced out

After the chain fixes above, 55 of Symfony's remaining 67 config-builder findings are one idiom:
`(new TreeBuilder('name'))->getRootNode()->children()`. `TreeBuilder` is
`@template T of 'array'|'scalar'|... = 'array'`, bound by the constructor's `$type`, and
`getRootNode()` picks its return with `T is 'array' ? ArrayNodeDefinition<$this> : ...` -- a
conditional on a _class_ template. Reading it needs three things at once: the class template's
default, a conditional subject that is a class template rather than a parameter, and a literal
`'array'` that survives from the `new` to the call. phrost has no literal types -- the bound lowers
to `string`, and `new TreeBuilder('x', 'array')` is a `TreeBuilder<string>` -- so the third is a new
dimension on every type, which §8.1 says to treat as a semantics change. Measured before building:
47 `getRootNode()` calls in Symfony's own source and none in any application corpus. Not worth the
dimension at that price.

## A literal handed to a template bound nothing, and `array_key_first` lost its test at lowering

PHP 8.5's `array_first([$person])` read as `mixed`. A literal resolves as a record -- `[$p]` is
`array{0: Person}` -- and a record has no _element_ by design, so every contract branch that read
`element_of_held` came back empty. `value_contained` is the seam now: the element where there is
one, and the union of a shape's values where there is not, used by every branch reader and by the
generics substitution, which had its own copy of the question.

Three more things were behind it, each the kind §8.1 describes:

- **Two constructors of one rule, at lowering.** A contract reduces a template to an argument in
  three places, and the third -- a _branch_ of a conditional -- knew only element templates. So
  `array_key_first`'s `(... ? K : null)` was refused as a branch, the whole conditional with it, and
  what got stored was the agreed "`K` or null" that no guard could take the null out of.
  `array_first`, the same declaration with a `V`, kept its test. `bound_by_the_container` is the one
  answer now.
- **Two spellings of "is this key an int".** The shape renderer had PHP's rule (`'01'` and `'-0'`
  stay strings, `'-1'` does not) and the shape-key reader had "all digits". One rule now,
  `phrost_model::is_an_integer_key`, settled by asking PHP for a table rather than the manual.
- **A literal is non-empty as written, not forever.**
  `$stack = [$p]; array_pop($stack); array_first($stack)` can be null, and a binding is not
  withdrawn by a call. So the literal's non-emptiness is refused once any call has had the value
  since the write -- which parameters are by-reference is the callee's to say. A declared shape with
  a required key is non-empty by its type and needs no such care: that is what a declaration is for.

Measured over six projects: three Laravel `upsert` false positives found and fixed before landing (a
guard on one path, `$values = [$values]` on the other -- both non-empty, and asked only of the
guard), coverage gains like `array` -> `string[]` and `string|int|false` -> `int|false` for
`array_search` over a positional literal. One finding stays and is not this change's: WordPress's
`Services_JSON` pushes arrays onto `$stk = array(SERVICES_JSON_IN_ARR)` with `array_push`, which no
reader of a local tracks, so `end($stk)` reads as `int|false` where the code holds an array.

## A literal was the whole answer for a variable written into afterwards

`$args = ['--force']; $args[] = $path;` read as `array{0: string}`. An element write is not a
binding -- it creates a variable nothing else bound, and otherwise never replaces the assignment --
so the literal was every word about the variable for as long as it lived. A text scan finds about
one literal in four assigned to a local and written into afterwards, on Drupal and WordPress alike.
It surfaced as `end($stk)` reading `int` on a stack that had been pushed arrays.

The body layer finds the writes (`slots_filled_since`: reach-checked, a loop's writes carried round
only when the literal is outside it) and the view folds them in (`locals/filled.rs`). Two rules were
found by the corpora rather than by thinking, and both are the same lesson -- **an answer that
widens to a union is worse than one that admits it does not know**:

- **A write through a slot** -- `$raw_types['NOT IN'][] = 'note'` -- first made the whole variable a
  bare `array`, so the `foreach` over it lost its string keys and an `incompatible-argument` that
  the key rule had just removed came back. It changes that one slot; the keys stand.
- **Past the cap** a record first became a map of the union of its values, and a map answers every
  key with all of them: `$payload['html_content']` read as the attachments and the headers too. It
  is a bare `array` now, and the slot reads go back to the writes that answer them one by one.

Measured over six projects: four findings added and all four true by the declarations involved (a
`?string` in a shape a parameter requires a `string` in, `getHtmlBody()`'s `resource`,
`wp_get_object_terms()`'s `WP_Error` handed to `in_array`) -- except one, a `substr_replace` whose
conditional return is refused at lowering because its else branch is `array<K, string>`, which is
that function's gap and not this one's. Serial `check` unchanged on two projects (1.71-1.74 s, 5.5
s).

## A branch mentioning a template was refused whole, and `substr_replace` read as `array|string`

`substr_replace`'s stub is `($string is string ? string : array<K, string>)` beside
`@template K as array-key`, and lowering refuses a branch that mentions a template anywhere, because
lowered as written it names a class called `K`. Refusing the branch refused the conditional, so
every call -- on a string, which is nearly all of them -- read as the declared `array|string` and
each `strlen()` or `str_contains()` around one was an `incompatible-argument` **error**: 34 on
Symfony, 3 on WordPress. A nested template now stands for its declared bound, which is the weaker
claim the declaration makes for whichever `K` it is.

The rule found its own limit on the first corpus run. Messenger's `Envelope::all()` is
`(... ? ... : list<TStamp>)` beside `class-string<TStamp>`, and with the bound in place every stamp
read as the bare `StampInterface` -- five `undefined-method`s where the class the call names had
been answered. The refusal had been _load-bearing_: it was what handed the template to the call-site
substitution, which reads `class-string<T>`, `T $x` and `T[] $xs` and does better than any bound. So
a template an argument binds one of those three ways keeps the refusal, and the bound is used only
where nothing downstream reads the binding -- the keys of an argument, or nothing at all. The same
shape as the `NeverReturns` note in CLAUDE.md §8.1 from the other side: before replacing a refusal
with an answer, ask what the refusal was handing the question to.

## A required key a record lacks is a rejection now, because the record is whole

`record_against_shape` answered `Unknown` for a key the shape requires and the record does not hold,
and said why: the record held only what its literal wrote, and `$r = ['a' => 1]; $r['b'] = 'x';` is
the commonest way to build one. That limit was the fold below it, and the fold exists now, so the
key nothing put there is a finding. A key written in a branch counts as present -- the fold puts it
in the record -- which keeps the doubt on the quiet side.

Eleven findings over six projects, and every one is a docblock that disagrees with its callers:
seven in a Symfony test helper declaring `scrollOffset` for an array the renderer and every caller
spell `scroll_offset`, three in Symfony's `ArrayShapeType` declaring an `optional` key its own code
reads with `?? false`, one in the reference project's unit test handing `validateString()` three of
the twelve keys its `FieldType` alias requires. Proved by sabotage that the two quiet cases in the
fixture -- a record completed a key at a time, and one completed in a branch -- are quiet because of
the fold: without it both are reported.

## Two narrowings that took off less than their guard proved

Found together, because typing Laravel's helpers (`app()`, `config()`) made the checks around them
speak, and two of the first things they said were wrong.

- **`$x ?? ''` over a docblock union.** `@param View|string|array|null $content` arrives nested --
  `View|(string|(array|null))` to the parser -- and `without_null` took the null off the _outer_
  union's operands only. So `$content ?? ''` read as possibly null. It recurses now, the way the
  `Held::OneOf` arm beside it always did: 50 `possibly-null-argument` findings gone on Laravel, 11
  more on Symfony and WordPress. A degenerate form a constructor does not fold out, as §8.1 says to
  expect.
- **`match (true)`'s later arms.** An arm runs when every earlier condition failed, and the guards
  said so -- over the arm's _body_. But a guard's proof reads the value where its condition is
  written, so the default arm was proved from the second arm's `is_array` read at that arm's
  condition, where the first arm's `is_null` was not in effect, and the null came back. The earlier
  negations cover a later arm's conditions as well now, which is when PHP evaluates them.

## Three spellings of a conditional return that each dropped the whole contract

- **`X is list{}`**, the empty-shape test, had no reading. Mockery writes `shouldReceive(...$names)`
  as `($names is list{} ? HigherOrderMessage : Expectation)`, and for a variadic parameter the call
  settles it exactly: anything written there fills it. `ConditionTest::Empty`, its own test rather
  than `NonEmpty` with the branches swapped, because the two are proved from different evidence.
- **A branch that is itself a conditional** lowered to nothing, and the contract with it. Laravel's
  `app()` is `(... ? T : ($abstract is null ? Application : mixed))`; whichever way the inner test
  goes it is one of its branches, so a nested conditional inside a contract is their union. _Only_
  inside a contract, where a call picks between it and the other branch: read as a union at the top
  of a tag nothing settles, WordPress's `get_pages()` became `int|WP_Post` for calls that are always
  `WP_Post`, and the corpus said so with seventy findings.
- **`@phpstan-template`** does not contain `@template`, which is what the prefilter every template
  reader shares looked for. Fixed as hygiene: a template read as a phantom class rejects nothing
  (`an_undeclared_class_rejects_nothing`), so no finding moved.

Laravel: 178 325 -> 179 380 member accesses resolved, 31 636 -> 31 783 assignments typed; findings
-55 +22, every addition a declaration now speaking (`app(Factory::class)->event()` on an interface
that declares no `event()`). WordPress -11 +24, the additions `WP_Theme::get('Version')`'s own
`string|false`. Cache version 40.

## WordPress decides its getters' returns by the shape of `$args`, and none of it was read

`get_posts()` is
`($args is array{ fields: 'ids', ... } ? int[] : ($args is array{ fields: 'id=>parent', ... } ? array<int, int> : WP_Post[]))`,
and thirty-two WordPress functions test a key of an `$args` array that way -- `fields`, `echo`,
`count`, and `exit` on `wp_die()`. None of it had a reading, so the whole return was nothing and
`$post->post_title` after one was untyped. Three pieces:

- **`ConditionTest::Shape`**: one key, certainly present, holding one of a few literals. A call
  settles it from the array it wrote, or from the literal its variable was assigned _with what was
  written into it since_ -- `$args['s'] = $search;` above `get_posts($args)` is the ordinary shape
  and says nothing about `fields`, so a fill at another key does not refuse the reading; one at the
  tested key, or at a key nothing names, does. A string where an array is tested is not that array.
- **`Contract::TestChain`**: a chain of tests of any kind, each link with its own argument --
  `get_children()` tests `$args` and then `$output`. A link nothing can test ends the chain with the
  union of what is left rather than refusing it: `paginate_comments_links()` ends on
  `$args is ''|array`, after two tests that settle every call. `LiteralCases` stays for the chain
  the completion text reader settles from characters.
- **A contract's `never` branch puts the name on the never-returning list.** `wp_die()` is
  `($args is array{exit: false} ? void : never)`, and it passed the name prefilter on WordPress only
  because a customizer method of the same name declares `@return never` -- a fixture without that
  method kept the null `if (!$x) { wp_die(); }` had ruled out.

Measured: WordPress 103 findings gone and 4 added -- the gone ones nearly all
`if (!$tax) { wp_die(); }` guards that now rule the `false` out, the added ones `reset()` on a
possibly empty list of ids and a legacy `get_calendar(true, false)` the annotation says returns
`void`. Symfony and psl lost four `string|array|float` findings that the previous commit's nested
union had made of `microtime()`, which a chain now settles. Laravel unchanged in findings; seven of
its accesses stopped resolving, and they were `app('config')->get()` read as the `Application` --
the union had been wrong there. Serial `check` unchanged (WordPress 5.44 s, the reference project
1.70 s). Cache version 41.

## `wpdb::get_results()` had no type, and three gaps lined up to make it so

`$wpdb->get_results(...)` was untyped at 92 WordPress call sites and `get_row()` at 36. The tag is
`($query is non-falsy-string ? ($output is 'OBJECT' ? list<stdClass>|null : (...)) : null)`, a
conditional return replaces the plain `@return array|null`, and nothing settled it -- so the member
had no return type at all. Each of three gaps was enough on its own; closing any two left the sites
untyped, which is §8's "a rule can be right and still buy nothing because a different rule is the
blocker", three deep.

- **The test.** `non-empty-string` and `non-falsy-string` (and `truthy-string`) were both `string`
  to every type phrost kept -- since 2026-10-25 they are types of their own, and the test reads the
  argument's type first; see "`non-empty-string` is a type" below -- so they were settled from what
  the call _wrote_: a quoted string by its text, an interpolated one by the text around its holes
  (whatever fills a hole, `"SELECT * FROM {$t}"` is not falsy), and a concatenation when any piece
  of it passes -- joining strings never empties one or shortens one to `'0'`. `null` and a number
  pass neither. An escape proves the string non-empty and nothing more, since `"\060"` is `'0'`.
  Anything else settles nothing.
- **The nesting.** The rest of the chain sits in the branch where the query test _passed_, so a
  chain that only continued in the else branch could not hold it. A link now carries `negated` -- it
  picks its branch where its test fails -- which also reads `is not` in a chain, where it used to
  refuse the whole.
- **The reader.** `$output = OBJECT` defaults to a constant, which lowering cannot read, so the
  link's `by_default` was `None` and every call leaving `$output` out settled nothing.
  `LiteralCases` had read constants and defaults through `string_passed_or_defaulted` all along; a
  `Literal` test in a chain had its own reader that knew neither. Two constructors of one rule,
  divided along whether the chain was all strings on one argument -- now one reader. It also fixes a
  coincidence the other reader leaned on: a constant written at the call was compared by its _name_,
  which equals its value for WordPress's constants and for no rule of the language.

**An unsettled link whose branch is only the absence no longer settles nothing.** 76 of the calls
pass `$wpdb->prepare(...)`, declared `string|null`, and no reading of the query settles them. But
the branch the test would pick is `null`, so the call is that or whatever the rest of the chain
settles, which is the rest's answer with a null beside it -- what `Settled::Either` already says for
a non-empty test. A single conditional reads the same way, `is not` included. `?mixed` came out of
this for a promise's `wait($unwrap)`, whose `TValue` the receiver never named: `mixed` admits the
null already, and a _stored_ `mixed` is a spelling the nullable constructor cannot see through --
the degenerate form again, and folding it also cleared two older `?mixed` in psl and WordPress.

Measured: WordPress 179 assignments typed that had none, 286 accesses now on a known `stdClass` and
21 resolved, none lost; **+56 findings, every one a null WordPress declares** -- 24 `foreach` over
`?array<int, stdClass>`, where `OBJECT`'s own branch is `list<stdClass>|null` ("the connection is
not ready"), and the rest the `prepare()` null and the `stdClass` rows handed to `WP_Post`
parameters. §8.2's precision gain arriving as findings. The other five corpora unchanged in
findings. Serial `check` +1 % on WordPress (6.66 -> 6.74 s, more typed values to check) and
unchanged on the reference project. Cache version 42; `cold == warm` on WordPress, 9 134 both ways.

## `tap()` came back untyped for two reasons stacked, and a null test settles by type now

Laravel's `tap($value, $callback = null)` is `($callback is null ? HigherOrderTapProxy : TValue)`
with `@param TValue $value`, and 61 assignments from it were untyped in Laravel's own source. Two
gaps, and each was enough alone -- the first hid the second:

- **The branch.** A template a parameter is declared as outright is whatever the call passed --
  `ArgumentType`. The reader of a `@return Bar<TValue>` slot (`argument_branch`) asked
  `bound_by_a_parameter`; the reader of a conditional's branch (`return_branch`) asked about
  elements, keys, containers and class-strings and not this, so `TValue` fell to the template
  refusal and took the contract with it -- even `tap($x)` with no callback said nothing. Two
  constructors of one rule again, divided along whether the template sat in a slot or in a branch.
  Only a branch that is exactly the template: `TValue|null` would be a second claim beside what was
  passed, and `ArgumentType`'s sentinel slot means the parameter's _default_, which a branch does
  not read.
- **The test.** `is null` read only a written `null`, so `tap($x, function () { ... })` settled
  nothing. A value whose type has no null in it fails the test now, through `argument_held` and
  `takes_a_null_off` -- the closure, the `new`, the typed parameter. One that admits null, `mixed`,
  or nothing known still settles nothing.

The second also settled stub conditionals nobody had looked at: `getenv($name)` with a typed name is
`string|false` rather than `array|string|false`, and Doctrine's `convertToPHPValue()` on a non-null
value is the value's class rather than its nullable.

Measured: Laravel 70 assignments typed that had none and 90 accesses now on a known receiver, none
lost; findings -1 +4, all four real -- `tap(ob_get_contents(), ...)` returned where `string` is
declared, `tap(tmpfile(), ...)` where `resource` is, `__($key)` handing `array|string` to
`Stringable`, and `PendingProcess::start()` returning a `FakeInvokedProcess` where its docblock
declares the concrete `InvokedProcess` it is a sibling of. Symfony -5, the `getenv()` unions and a
nullable `DatePoint`. The other corpora unchanged in findings; `check` unchanged on Laravel.

## Arithmetic over a union answered nothing, and the silence was hiding what its joins said

`arithmetic_result` took one `BuiltinTy` per side, so `strrpos($s, '|') + 1` (`int|false`),
`$offset + $page` (`?int`) and `$i * $rate` (`int|float`) were all untyped -- and a computed number
in real code is a union more often than not. The module doc said so on purpose: "a bool or a numeric
string PHP would happily coerce is `None`: the coercion is real but the result is not something to
state as a fact." Asked of PHP 8.5 it is a fact: `null + 1` is `int(1)`, `true * 1.5` is
`float(1.5)`, `"5" + 1` is an int and `"5.5" + 1` a float. So each operand is now read as the set of
kinds it may be (`locals/arithmetic.rs`, through `union_operands` with the null asked separately,
since a `?int` lifts its null into a wrapper the operands no longer show), and the result is what
every pair makes. An array beside any other kind stays refused -- `array|int + 1` is a `TypeError`
on one path, and answering with the half that works would read as the code being fine.

**`++$x` is not `$x + 1`**, which the path had modelled it as. PHP leaves a bool alone, steps `null`
up to `1` and leaves it `null` going down, and counts a non-numeric string like a column name (`'z'`
becomes `'aa'`). Folding the step into the arithmetic would have claimed every new coercion for it,
so it is `PathRoot::Stepped` and answers only for an int or a float, as before.

**Typing a value makes the joins it sits in speak.** Every new finding was a declaration that had
been there all along, unread because one operand of a join was untyped and a join with an unknown
operand answers nothing: `DateInterval::$days` is `int|false` and the reference project returns it
through a ternary whose other side is `-$interval->days`; `array_search()`'s `false` reaches
`array_splice()` in Laravel's kernel through the branch that did no arithmetic; `ini_get()`'s
`false` in WordPress's `recurse_dirsize()` through the branch that did no `-= 1`. When a rule types
one operand of a join, read the findings as the _other_ operands' declarations before reading them
as the rule's.

The one that is not a declaration speaking is psl's `Month::from($n - abs($t))` handed `int|float`:
`abs()` is `($num is int ? int : float)` and settles to `int` for an int -- but `$t` came from
`$this->getMonth()` inside a trait, which resolves to nothing because the trait's
`@require-implements DateTimeInterface` is not read, so the conditional fell back to its declared
`int|float`.

Measured against the previous commit: assignments typed WordPress +101, Symfony +48, the reference
project +13, psl +4, Laravel +6; 23 accesses moved from no receiver to a resolved one; none lost.
Findings +1 on each of five projects and +3 on WordPress, all of the kind above. `check` on
WordPress with `--jobs 1` unchanged (7.14-7.21 s against 7.17-7.66 s).

## `$this` in a trait is the trait and what it requires, and four readers could not see an intersection

psl's date-time traits are `@require-implements DateTimeInterface` and call `$this->getMonth()` a
hundred times; every one was `Dynamic` -- a trait's `$this` is whoever composed it -- and so
untyped. The tag (and `@require-extends`, and `@mixin`) is now read where a mixin is: an interface's
table takes the members in, which is exactly what `@require-extends` on an interface means, and a
trait's table does not, for the reason `merge_mixins` already records -- every composing class
copies it, and `ProphecyTrait`'s `@mixin TestCase` merged there took completion p95 from 0.39 ms to
0.72. Instead `$this` inside a trait that writes one is `Held::AllOf[trait, ...]`
(`IndexView::this_held`), one lookup per `$this` in such a trait and nothing anywhere else.

Making `$this` an intersection found every reader that reduces a receiver to one class with
`class_of_held` -- which answers `None` for an intersection by design, "a caller that wants members
asks `intersection_of_held`". The step walker had learned that long ago (`owner_of_member`, whose
comment records the same blindness for unions). Four had not: the declaration a callback's parameter
is read from (Laravel's `BuildsQueries` is `@mixin Builder`, and
`$this->chunk($n, function ($items) ...)` lost `$items` the moment `$this` became both), the
signature arguments are checked against and hints are drawn from, goto on a call, and the
discarded-result check. They were blind to _every_ intersection -- a `MockObject&Repo`, a property
declared `A&B` -- so the calls on those were never checked at all.

They go through `declaring_class` now, which is `owner_of_member` **without its union rungs**, and
the difference is the point. Which operand of a union _answers_ a name is right for goto and for the
type a call hands back; it is wrong for a signature. Taken whole, the ladder checked
`$this->factory->create($name)` on `UuidFactory|RandomBasedUuidFactory|...` against the operand
whose `create()` takes nothing -- 19 findings on Symfony, the `extra-argument`s all false. An
intersection is every operand at once, so whichever declares the member declares it for the value.

Two spellings again, in the access outcome: an intersection a guard _built_ names no class and
arrived as `AnyObject`; one a parameter _wrote_ arrived as `UnknownReceiver`. Both are now settled
by the best of their operands, as a union already was -- which made a written `A&B` with a member
neither declares a `MissingMember` for the first time, and with it three `undefined-method`s in
Symfony's translator decorators that were guarded by
`$t instanceof Translator || method_exists($t, 'getFallbackLocales')`. `existence_checked` read an
`||` as vouching for nothing, rightly for `method_exists(...) || $flag`; an `||` whose every operand
is either a `method_exists` for the name or an `instanceof` on the same value vouches now, and the
same rule removed an older false positive in `RecursiveContextualValidator`.

Measured against the previous commit: psl 131 accesses `Dynamic -> Resolved` and 26 more off no
receiver, 21 assignments typed; Laravel 136 + 84 and 33; none lost. Findings: psl -2 (the `abs()`
false positive the arithmetic note above names, and an import used only by the tag) +2; Laravel +8,
Symfony +12 -1 -- calls through an intersection checked for the first time. Most are declarations
(`Connection::getName(): ?string` through `ManagesTransactions`'s mixin,
`ArgumentMetadata::getType()`, a `?string` property), a few are the forward-compatible
`/* $param */` that `extra-argument` already reports elsewhere, and two are relational: `$user` is
null only when the checker is a guest checker, and the `xor` above
`ContextListener::hasUserChanged()`'s `getSalt()`. `check` on Symfony with `--jobs 1` unchanged
(14.15-14.20 s against 14.25-14.73 s); `cold == warm` on psl.

## A conditional return nothing settles is both of its branches

`@return ($value is string ? string : array)` lowered to _no type_, so every call whose argument
settled nothing -- a variable nobody typed, a `mixed` read out of a config array -- answered
nothing, while the contracts beside it said in two places that "the declaration's union of both
branches is then what it already said". PHPStan 2.2, asked, answers the union: `two($mixed)` is
`array|string`, `wpurl($s, $int)` every branch of the chain. It lowers that way now
(`type_lowering`, a `never` branch dropped), and the prefixed tag that lowers to nothing at all
gives way to the plain one written for readers that cannot hold it -- but one naming the
declaration's own template is still refused, and the refusal stands rather than handing over:
`wp_unslash()` writes `@return string|array` above a `@phpstan-return` that hands back whatever it
was given, and taken for a `$_POST` value the plain claim was **314** `incompatible-argument`s on
WordPress about arrays nobody passes.

The union is only as good as the settling in front of it, and switching it on was the instrument
that found the settling's gaps -- each one a call PHPStan settles and phrost answered with every
branch: `wp_parse_url()`'s `$component is -1` (a minus in front of a number, and the same minus in
the default), `get_post_field()`'s `$field is 'ID'|'post_parent'|'menu_order'` (one branch reached
from three strings), and `range(1, 12)` leaving `$step = 1` to a `$step is float` test that read
string and keyword defaults and returned before a number. Each was its own commit before this one.
And a fourth thing the narrower answers exposed, which was not about conditionals at all:
`isset($url['host'])` ruled the null out of `$url` and not the `false` -- `parse_url()`'s own
sentinel -- and `!empty($url['host'])` ruled out neither. Reading both took 190 findings off
WordPress.

What is left on WordPress is a dozen calls PHPStan answers the same way -- `get_posts($parsed_args)`
on an array whose keys come from `wp_parse_args()` is genuinely `int[]|array<int, int>|WP_Post[]`
until something relational says `fields` was not set -- and a few it settles where phrost does not
yet: a test against a class (`sanitize_post()`'s `$post is WP_Post`), a class template bound by a
constructor argument (SimplePie's `Psr7Compatible`). Laravel +7, all `?string` from `preg_replace()`
returned where a method's conditional now declares `string`; Symfony -7 `undefined-method` on
`NodeDefinition` +6. Coverage: Laravel 115 assignments and 180 accesses gained, one lost to
`FOLLOW_LIMIT` -- a loop-carried `$targetMonth` that fitted in twelve hops and now needs thirteen,
the silent failure `view.rs` warns of.

## A conditional naming its own template stood for nothing, and then for too much

`wp_unslash()`, `Arr::wrap()`, `enum_value()` and `resolveValue()` write their conditional return
over the declaration's own `@template`, and a template names no class -- so the tag was refused
whole, the contract with it over one branch no reader could take apart (`array<key-of<T>, ...>`),
and 211 calls on three corpora answered nothing. Three pieces, each found by measuring the one
before:

- **The branch nothing takes apart is kept as the container it is.** `array<key-of<T>, ...>` is an
  `array` however `T` was bound -- weaker than the truth, not different from it -- so the contract
  stands and `wp_unslash($string)` is a `string`. Not where a `class-string<T>` binds the template:
  the call-site substitution reads those, and Messenger's `all(Foo::class)` is a `list<Foo>` there.
- **A call the contract cannot settle gets the declaration with each template standing for
  `mixed`.** Standing for its _bound_ was tried first and is wrong in a quiet way: the bound is true
  of every call and is still a claim about this one that nobody made. `array_keys($reasons)` over
  keys nothing tracked read as `list<int|string>` off `K of array-key`, and two
  `incompatible-argument`s appeared on the reference project where PHPStan, which tracks the keys,
  has none. A `mixed` branch is then the whole answer -- `string|array|mixed` judged as a union put
  its `array` against a `string` parameter, which is the 314 the plain-tag note above records, back
  by a second door.
- **`array<mixed, mixed>` is the bare `array`.** A third spelling the erasure produced, and judged
  as written it was an array _keyed by `mixed`_ -- which `iterator_to_array()` on a traversable
  nothing described then failed against `array<string, Provider>`.

And `scalar` joined the test vocabulary, because `resolveValue($config)` on an array is
`array<array|scalar>` and the union of its branches put a `bool` into every Symfony config pass.

Only a `@return`: a refused `@param-out` leaves the variable what it was, and `usort($rows, ...)`
erased came back `list<mixed>` from a `Row[]`. Measured against the commit before: Laravel 247
assignments and 116 accesses typed, WordPress 155, Symfony 8; findings Laravel +3 (a `find()` that
may be null, a property `DateTimeInterface` does not declare), WordPress +1 -3, Symfony +1 (the
declared `array<array|scalar>` against `load()`'s `array<array>`), none on the reference project.

The step after, measured first and taken second: answering an unsettled conditional with _both
branches read for the call_ (`tap($repo, $maybe)` is `HigherOrderTapProxy|Repository`, as PHPStan
says, where the erased declaration is `mixed`). Measured before anything else it moved 2 038 Symfony
accesses, and the measurement was hiding a bug: `class-string<FormFlowTypeInterface>` was the bare
`ClassString` test, so every `create(TextType::class)` passed it and read as a form _flow_. The
bound is compared now (the class itself descends from it or does not -- there is no subclass to
leave room for), which put 196 of those calls back to `FormInterface` with no finding moving; what
the per-call union then adds is the `create(static::TESTED_TYPE)` calls a class constant leaves
unsettled, as `FormFlowInterface|FormInterface`. WordPress rewords 140 findings to the branch types
(`string|string[]` where the declaration said `array|string`); Laravel gains 24, every one a
`find($id)` whose `$id` nothing types read as the `?Model` it may be -- PHPStan's answer at level 8.
A null inside either branch is lifted out before the two are joined: an interned `?string` is a
spelling `Held::one_of` cannot see into, and `?string|?string[]` is the degenerate form no guard
narrows.

## `$this` in a trait is the classes it is composed into

A trait's body never runs as the trait: PHP copies it into each class that `use`s it, so `$this` is
one of those. Read as the trait alone, Laravel's `InteractsWithPivotTable` reached nothing past its
own members -- `$this->parent`, which its one host `BelongsToMany` declares two classes up, had no
goto, no hover and 474 fewer completions than phpantom offers. `this_held` now reads a trait with no
`@mixin` / `@require-*` tag through its hosts (`trait_hosts`, transitive through traits that compose
it, at most sixteen): one host is the answer whole, since its table holds everything the trait put
there; several are `Trait & (H1 | H2 ...)`. The trait stays in the intersection on purpose -- read
as the union alone, every call to one of the trait's _own_ methods was a call on several receivers,
which the argument checks decline to judge, and `EnumeratesValues::whereNull($key = null)` passing
its null to `whereStrict(string $key)` went unreported.

Three things broke on the way, and each is a shape §8.1 already names. **A degenerate form a
constructor does not fold out:** `also_of_class` asked `class_of_held` of an intersection, got
nothing, and _replaced_ the value with the guard's class -- so `$this instanceof Contract` inside a
trait lost every member the trait declares. A guard is one more operand of an intersection now. **A
reduction that hides a rule:** `narrowed_this` reduced the guarded `$this` to one class, and an
intersection reduces to none, so the guard's answer fell back to the enclosing class; `this_held`
reads the guarded value whole. And **two readers of one fact:** visibility asked "which class is
this code in" separately from `$this`, so a host's private property read from the trait was reported
inaccessible; `scopes_at` is the trait plus its hosts. `intersection_of_held` also flattens an
operand that is itself a union, and the `> 1` filter in `access_of` stays: an intersection that
yields one class has an operand nothing resolved, and letting that one class decide "missing"
reported `CloseHandleInterface::writeAll()` on psl's `pipe()` pairs.

What it costs on the corpora is findings, and they are the feature working: Laravel +40 / -36,
Symfony +62 / -28, each one a check that already speaks in class bodies now reading a trait body --
`$this->definition->setAbstract()` on Symfony's `Definition|Alias|null`, a version polyfill calling
a PHPUnit method that no longer exists, and one Laravel silences with
`@phpstan-ignore arguments.count`. Coverage gained 1 043 accesses and 114 assignments on Laravel,
677 and 29 on Symfony, and lost three `$this::class` reads in multi-host traits.

## A closure bound where it is written

`Closure::bind(function () { $this->initializeBundles(); }, $kernel, $kernel::class)` is how
Symfony's debug commands reach into the kernel, and
`(fn () => $this->signalHandlers)->call($this->registry)` is how Laravel reads a registry -- and
`$this` there was the class the closure is written in, so every member of the real object was a
missing one. No declaration can describe it, because the object is an argument; `closure_this_at`
reads the bind beside the `@param-closure-this` it already read, innermost closure first, and the
view resolves the bound value where the call is written. `Closure::bind` is matched on its receiver:
`bind` is also what Laravel's router and Symfony's container call a method, and the router hands its
closure over as the _second_ argument of one.

## The property `isset()` asks about is the question, not a use

`isset($job->retryUntil)`, `empty($job->delay)` and `$job->retryUntil ?? null` read a property PHP
does not warn about, declared or not, visible or not -- `false`, `true` and the fallback, run
against 8.5.10 -- and `undefined-property` / `inaccessible-member` reported each one, on the line
that is the author's own existence check. `MemberAccess::asked_if_set` carries the context the null
reach already read (`null_suppressed_ranges`), and both checks step past it: 52 findings on three
corpora, none added. A method call inside the same `isset()` is still evaluated and still reported,
and so is the plain read after it.

## A class a docblock names and nobody declares

`undefined-type` left every docblock name alone, "until templates are modelled" -- and they had been
for months, so `@return Status` in a namespace where `Status` is three constants,
`@mixin Missing\Helper` and `@property Unknown $x` were all silent. `doc-undefined-type` reports
them, as a warning: 0 on the reference project, 59 on Laravel, 74 on Symfony, 67 on WordPress, 6 on
psl -- the optional packages a checkout does not install (`Pusher\Pusher`), fixtures naming `Bar`,
and a namespaced WordPress library writing `Stringable` without a leading slash.

What it had to learn not to report is the docblock language's own names: templates
(`templates_declared`, every `@template` spelling in the file -- a superset, the quiet direction),
`self`/`static`/`null`, dashed pseudo-types, and a `@see`, which `NameRefData::cross_reference` now
marks because `cross_reference.rs` had written "safe because docblock names are exempt" and that
stopped being true. And two lowering gaps it exposed: `@extends Base<RowShape>` and `@mixin` were
lowered with an empty alias table because the class's aliases were built later, inside
`class_members`; and an alias built out of a `@phpstan-import-type` saw no imports, so it lowered to
nothing and its uses read as a class. Building the table once, first, and holding an alias that
lowers to nothing the way an import is held fixed both -- and six Laravel `incompatible-argument`s
that were the same alias read as a class.

`lint.off` now switches off a type check's code as well as a syntax rule's (`AskedFor::off`): a
check on by default could otherwise be switched off by nobody.

## A substitution that stops after its first hop

`Relation::getResults()` returns `TResult`, and `HasMany<TRelatedModel, ...>` fills that with
`Collection<int, TRelatedModel>` -- a type written in the _receiver's own_ template. The ladder in
`substituted_by_receiver` found the first hop and stopped, so a `HasMany<Comment, Post>` answered
`Collection<int, TRelatedModel>`: a container of a class nothing declares, on every relation Laravel
has. The second hop puts the receiver's arguments into what the ancestor was filled with.

It needs the argument _as written_, and that is the second half of the bug: `BelongsTo` fills the
same slot with `?TRelated`, which names nothing real until the receiver's `User` is in it --
`substituted_template` filters on `names_something_real` and fell back to a bound, so the single
relations answered nothing while the many-relations worked. When a substitution feeds another one,
the filter for a _final_ answer is the wrong filter for an intermediate one.

Moved nothing on the five corpora, which are framework source and do not walk relations; on a
Laravel application it is what `$post->comments` and every relation property stand on.

## A bare generic hint is refined by the call its body returns

Laravel's documentation writes
`public function author(): BelongsTo { return $this->belongsTo(User::class); }` -- no docblock, so
the hint says _what_ comes back and only the call says of which model. A returned chain is now kept
beside a class hint where the one `return` is a keyed call on `$this`, and the view takes the body's
answer when it is that same class with its arguments filled, never otherwise: a hint is a guarantee
and a return may only get more precise.

Two costs priced. Index memory on the reference project was 110.4 MB with and without the rows, so
the narrow gate (a key on the first call) holds. And `returned_path_from_body` records a name
reference for a root it resolves, so calling it to throw most answers away recorded `new Relay()`'s
name twice and reported it undefined twice at one position -- the gate is checked on the syntax
first, before anything is lowered.

The same holds for a class a _docblock_ names bare, and for a body that returns a **subclass**
rather than the class with arguments. Laravel's factory stubs before version 10 wrote
`@return \Illuminate\Database\Eloquent\Factories\Factory` over every state method, whose body is
`return $this->state(...)` -- a `static`, so on a `UserFactory` the class that
`@extends Factory<User>`. Read as the bare `Factory`, `User::factory()->unverified()->create()` was
a dynamic `Model` in 86 of Monica's test reads. The chain is now kept where a docblock names a class
and the body is one call on `$this` (the syntax gate again, so no name is recorded and thrown away),
and the view takes a subclass in whatever spelling the step hands back -- a `static` arrives stored,
scoped to the class it was reached through. A class with no templates is a whole claim and stays as
declared. Monica resolved 97.7 -> 97.9 %; Laravel lost two `undefined-method` findings on a
paginator contract whose concrete class the body returns. Index memory +0.1 MB on Laravel and the
reference project.

And a chain rather than one call. BookStack's
`revisions(): HasMany { return $this->allRevisions()->where(...)->orderBy(...); }` failed twice
over: the syntax gate looked only at the outermost call, whose object is another call and not
`$this`, and the walk handed each step the previous one's _class_ without its arguments -- so
`where()`'s `static` on a `HasMany<PageRevision, Page>` came back a bare `HasMany`, and `->first()`
a `Model`. The gate now follows the chain down to the `$this` it starts from, a chain of several
calls is kept with or without a key on its first (one unkeyed call is a wrapper and buys nothing),
and each step's arguments travel to the next. BookStack 97.1 -> 97.3 % resolved, Laravel +201
accesses; findings renamed to their models, one Symfony `MockObject::match()` false positive gone.
Index memory unchanged on four projects.

## An arm that answers with `return` skips the steps after it

`resolve_path` matches the root, then walks the steps written after it; `walk` matches each step and
the arm's value is the next receiver. A `return Some(found)` inside either match hands back the
_whole path's_ answer, so everything written after that point is silently dropped -- and the answer
still looks right, because it is the right type for the part that was read. Met three times in one
day: `app('redirect')->back()` read as the `Redirector`, `Post::popular()->get()` as the builder,
and `(new $table[$key]())->label()` as the instance. The shape to look for is any `return Some(`
between a `match` on a root or a step and the code that applies what follows; an arm answers with
its value, and a labelled block is how a long arm breaks out early.

## A path root that keeps a literal has to agree with the type it used to be

`new $class()` over `$class = '\\Countable'` needs the string's text, and a body keeps none -- a
literal's path was `Builtin(String)`, which every string shares. `PathRoot::StringLiteral(expr)`
keeps the expression, and resolving it is still `string` to every reader but one. The reader it
broke was not a resolver: `literal_element` decides whether every entry of an array literal holds
the same _type_ by comparing their paths for equality, so two literals stopped agreeing and
`[PREG_INTERNAL_ERROR => 'a', ...][$code] ?? 'x'` lost its `string` -- found only by the coverage
dump, two assignments on 37 000 files, with no finding moving. When a root gains identity, every
place that compared roots _as types_ is a second spelling of the old one; the fix is to let two
literals agree on `string` there, not to give the identity back.

The class a value names comes in two strengths, and `class_spelled_by_path` asks for one of them: a
`Foo::class` or a `'Foo'` written into the value names that class exactly, while an instance's
class, a `class-string<T>` and a table read by key name a bound. `uninstantiable-class` asks for the
exact one, which is how `$c = Product::class; new $c();` is reported again without the factory over
`class-string<Product>` being reported with it.

## A sharper answer exposes the approximation it is built on

A falsy array is `[]`, and spelling that `array{}` instead of keeping the declared `array` is
plainly more precise -- and it immediately reported `$ret['_route']` _inside_
`if ($ret = $this->doMatch())` on Symfony, and typed fifty-three of WordPress's SimplePie
assignments as empty. Neither was the new rule. A call is read as a slot so that
`if ($this->thing()) { $this->thing()->run(); }` narrows, and a `MethodReturn` step keeps only
_literal_ arguments, so `get_source_tags(ATOM_10, ...)` and `get_source_tags(ATOM_03, ...)` are one
slot and the second call inherits the first one's falsehood. While a falsy array stayed `array` that
inheritance said nothing; once it said "empty" it said something false. The emptiness is now claimed
only where the guard's own path reaches its value through no call (`reads_no_call`), and the report
on top of it only where the guard was on the very local being read. When a sharpening moves answers
nobody expected, look for the approximation underneath before blaming the sharpening -- the same
lesson as the `?Row` join in CLAUDE.md §8.1, from the other direction.

## A contract mixed into its own implementation

An implied `@mixin` from Laravel's `Contracts\Cache\Repository` to the concrete repository is the
right judgement about the binding, and it made `phrost check` abort on the whole Laravel corpus with
a stack overflow. The concrete class implements the contract, so its contributors include the
contract, whose implied mixin is the concrete class again; a member the concrete class itself only
_forwards_ -- `lock()`, through its own `@mixin Store` -- is marked `forwarded`, and
`returned_by_method_contract` followed the forward to the class it was already in, forever. The
member table survived it because `flatten_via` keeps a stack; the forward did not, because it was
written for a model and a builder, which never point at each other. `implied_forward` now refuses a
mixin the value already is. The tell for the next one is the same: a seam that hands one class to
another is a graph, and a graph a framework fills in can have a cycle nobody wrote down.

## Two writes in one branch were two operands of a join

`if ($b) { $c = explode(',', $c); $c = $c[1]; }` reached the join as both writes, because each on
its own is only _possible_ past the `if`, and the answer was `array|string` -- reported against
every string parameter the name was handed to. Present since before the history was squashed, on all
five corpora, and invisible to every A/B because both arms of one always agreed.
`BodyFacts::overwrites` drops an earlier write when a later one sits in no branch it is not also in
and nothing can carry the earlier value past it: a `break` or `continue` between the two, a handler
of a shared `try` the read is in, a loop header. It is asked in `Scope::maybe_written`, the one join
locals and property slots both go through.

Fixing it sharpened answers, and four more rules turned out to have been hiding behind the wrong
operand -- the §8.1 shape, read straight off the diff:

- **A loop binding ignored the branch rule.** `foreach_binding` ranked by position alone, so
  `if (...) { foreach ($terms as $term) {...} } else { f($term); }` read the loop's element in the
  `else`. The binding walk now skips the arm of an `if` the position is not in.
- **A call statement's claim survived a write after it.**
  `sort($term); $term = implode(',', $term);` kept the by-reference `array`, and an
  `assertInstanceOf` kept its class over a reassignment, because `passed_to_calls` never asked what
  `narrowing_at` asks of a guard: `assigned_between`.
- **A sentinel vetoed a union's element.** `list<string>|false` iterated typed nothing while
  `?list<string>` typed a string; `false` and `null` now step aside in `element_of_union` as the
  nullable arm always did.
- **The path around a strict null test had no word.** `if ($s !== null) { $s = ...; }`, `isset` and
  `!is_null` prove null where the branch was skipped, and `Otherwise` mapped that to `Unproved`. It
  is `OfKind(Null)` now, and the view answers no path where the value cannot be null.

That last one exposed an older wrong answer under it. In
`if (null === $n) { $n = 1; } elseif ($n > 9) { $n = 9; }` the `elseif` says nothing about null, so
`narrowing_skipping` falls back to the outer test for that write -- whose complement is the _first_
arm, which wrote. `other_arm_writes` makes a claim stand aside when the arm it describes is written
on every way through.

### Closed: the path around a write is a set of paths

`if ($a) { if ($x === null) { $x = 'd'; } } return $x;` answered `string`. The write is skipped
where `!$a`, which tests nothing about `$x`, as well as where `$x !== null`, and `otherwise` read
only the guard the write sits under. `crate::skipped_paths` now walks every way through the `if`
tree the writes are in: an arm whose write runs on every way through it has none, an arm that leaves
has none, an arm without a write contributes its entering claim and what its own statements prove,
and statements one after another multiply. The two connectives are kept apart -- on one way every
claim held, so any is true and the kept one is the one saying the value was there; across ways only
what all of them imply survives. Anything but a tree of `if`s (a `switch`, a `try`, a loop, a write
in a condition or behind `&&`) gives the question back to the reading by one guard per write, which
is unchanged.

All six shapes the open note listed hold, and the conjunction made one test say more:
`if (!isset($v)) {...} elseif (is_int($v)) {...}` is skipped only where `isset` held and `is_int`
did not, so the value was there -- the old fold had called two different claims nothing. Measured
against the commit before: WordPress -2 (two findings back on `has_block`'s `$post`, which is null
past the `if` when `get_post()` finds nothing; four gone, among them `WP_Network`'s cached value,
which past its `if` was an object with an id or a number), Laravel -1, Symfony -1 -- and one back on
`AttributeLoader`, where `$attributeName` is null on the way around and only `$attributeMetadata`
being set says otherwise, a correlation between two names nothing here tracks. Coverage dumps move
the same way: three calls on Laravel's `Router` resolve, because the way around its `elseif` chain
is where the response was already a `SymfonyResponse`. `check --jobs 1` is no slower on the
reference project or Symfony.

## A docblock refined a bare container and never a union holding one

`array|false $lines` beside `@param list<string>|false $lines` kept the hint, and so did every
`@return list<T>|false` beside `: array|false` -- 135 declarations in the stub set alone (`scandir`,
`preg_split`, `file`, `class_uses`), so iterating any of them typed nothing while the `?array`
spelling of the same claim typed the element. Both readers of a declaration had the rule for a lone
operand and neither had it for a union. Both have it now, operand by operand, in
`refines_each_operand` beside `refined_type_ref` and in `phrost_ty`'s `refine.rs`: every container
in the hint is given its arguments and every other operand is restated. A docblock that drops one
the runtime admits (`@param list<Row>` beside `array|false`) or adds one it refuses is a competing
claim and the hint wins, as it did.

Two things came out of it, both the forms §8.1 names.

- **The written reader was one rule short of the interned one.** The interned `has_arguments` had
  learned that `array{...}|array{}` beside `: array` is one claim in two branches; the written one
  had not, so a parameter's callers read the shape and its own body read `array`. Caught by asking
  both halves the same question in one fixture, which is what `a_docblock_over_a_native_union` does
  -- the body is the written reader and the call the interned one.
- **A stub defect that had been inert.** The set declares `class_uses`, `class_parents` and
  `class_implements` as `list<...>|false`, against PHP's own `['T' => 'T']` and against its own
  prose. Nothing read the tag until the union rule did, and then Laravel's `trait_uses_recursive`,
  which builds a map with `+=`, read as returning a list. Fixed in the set and recorded in its
  README. A rule that starts reading data nobody read before is a stub audit, and the first findings
  it makes belong to the data until shown otherwise.

Measured against the commit before it: WordPress, Laravel, PSL, Laravel.io, Monica and the reference
project unchanged; BookStack -1 (`scandir` gives strings, so `str_replace` hands back a string);
Symfony +14, all on newly typed values. Twelve are `SimpleXMLElement::xpath()` and `attributes()`
answering `null` inside loops whose outer `xpath()` was already reported. Two are a guard on
`reset($steps)` that does not narrow `$steps` (`FormFlowCursor`), which PHPStan 2.2.17 reports from
the same declaration. And two are false positives PHPStan does **not** have, which is the open gap
this exposed: `MemcachedAdapter` rewrites every string element of `$servers` as an array by its key
inside a `foreach` (`$servers[$i] = [...]`, with `continue` for the ones already arrays), and
PHPStan reads the array after that loop as holding arrays where phrost still reads the declared
`array|string` elements. Both shapes were reproduced in one scratch file and put to the reference
project's vendored PHPStan before either sentence was written.

## Two old docblock spellings PHPStan reads and phrost read as written

Found by putting BookStack's own vendored PHPStan (2.2.16, with Larastan) at level 8 beside phrost
over `app/` and reading what phrost said where PHPStan said nothing within a line -- a project with
an analyser configured is an oracle no fixture corpus is, because both tools read the same
declarations. Both rules were checked against PHPStan on scratch files before either was written,
and both are settled at lowering, so the written reader and the interned one take the same answer:

- **`@param Row[]` beside `iterable $rows` is `iterable<Row>`**, and so is `array<Row>`; with the
  key written (`array<int, Row>`, `list<Row>`) it stays the array it says, and `?iterable` keeps its
  null. Taken as written the docblock won as an array and every Eloquent collection handed in was
  reported. `iterating_what_the_hint_accepts`, beside `keeping_the_null`, for `@param`, `@var` and
  `@return` alike.
- **`@param Row[] ...$rows` is a `Row` per argument**, and so is `array<K, V> ...$x` in PHPStan
  though here only the keyless one -- `list<Row>` is materialised as `array<int, Row>`, which
  PHPStan does _not_ unwrap, and the two cannot be told apart after lowering. Not where the hint is
  itself a container. `each_argument_of` in `params.rs`.

BookStack -5, Symfony -1, nothing added anywhere. What the same comparison showed that is **not**
phrost's to fix, so the next reader of it does not chase these:

- **`string|false` handed to `string` is PHPStan's leniency, not our noise.** `tempnam()`,
  `scandir()` and `getRealPath()` return what PHPStan prints in parentheses -- a _benevolent_ union,
  accepted where any operand is. 23 of BookStack's phrost-only findings; reported under
  `checkBenevolentUnionTypes`, and phrost's position on `false` is the strict one.
- **Larastan reads a console command's `$signature`.** `$this->argument('oldUrl')` is a `string` for
  a required argument and `$this->option('database')` a `?string`, where the method declares
  `array|string|bool|null`. 6 findings on BookStack and up to 93 on lines of Laravel's own commands,
  many of which declare their inputs in `getArguments()` instead. Recorded in the roadmap.
- **BookStack moves its config to `app/Config`** by overriding `configPath()` in its own
  `Application`, so the user model `config/auth.php` names is not found and `auth()->user()->id`
  reads `Authenticatable::$id`. Three findings, all in tests, and an idiosyncrasy.

## A convention for containers, applied to things that are not one

`foreach ($articles as $article)` over Laravel.io's `Article::all()->random(100)` typed `$article`
as `Article|Factory`. `random()` in that Laravel declares `static<int, TValue>|TValue`, so the union
is honest, and its `Article` half was iterated: the walk went up the model's hierarchy, reached the
`HasFactory<TFactory of Factory>` trait it uses, and took the last template's bound by the
convention that a container puts its value there. But PHP iterates an object that is not
`Traversable` by its visible properties, so no template of anything it extends or uses says what a
pass holds. `iterates_its_properties` is now asked before the convention, on all three spellings
that carried a copy of it -- a class, a written generic, an applied one -- walking `extends` and
`implements` only, since neither a trait nor a mixin makes a class iterable.

The same reading had been answering a question that is not about iteration at all. The constructor
rule for `@param T[] $items` read _any_ generic's last argument as a container's element, so
`@param MorphTo<*, TDeclaringModel> $relation` handed a bare `Relation` bound `TDeclaringModel` by
iterating the relation, and `@param Builder<TRelated> $query` handed a `Builder<User>` was right
only because `TRelated` happened to be last. A class that is no container says what fills `T` by
_position_: in the arguments the value was written with, or in what its class gives the parameter's
class through `@extends` (`argument_in_slot`). And a parameter typed `Closure(Connection): T` binds
`T` to what the closure handed over returns (`callback_held`) -- BookStack's
`(new DatabaseTransaction(fn () => $transformer->transformChapterToBook($chapter)))->run()` is the
`Book`, where the constructor rule had no arm for a callable and `run()`'s `@return TReturn` fell to
`mixed`. Psl lost nine `$graph is int[]` findings on a `DirectedGraph<string, null>`; Laravel's
paginators now carry their arguments; Laravel.io gained one finding the bad union had hidden,
`markSolution(Reply)` handed `random()`'s declared union.

Symfony's TypeInfo writes `@param list<T> $types` on a variadic `Type ...$types` and lost a
`IntersectionType<TypeIdentifier>` that was the convention's garbage. Not replaced with a variadic
rule on purpose: PHPStan, asked directly, reads that tag as each argument's type, reports it
incompatible with the native `object`, and binds nothing (`Bag<mixed>`).

## A template a written `@mixin` supplies, and which mixin is asked first

`$user->posts()->first()` answered nothing on every Laravel project. `Relation` writes
`@mixin Builder<TRelatedModel>`, the builder `@use BuildsQueries<TModel>`, and `first()` is the
trait's `@return TValue|null` -- so the value is the relation's own `TRelatedModel`, two tags from
it. An implied mixin (`Model` behaves as `@mixin Builder<static>`) has its own route, the call
re-asked on the mixin with its arguments; a written one had none, because the index kept a mixin as
a bare name and the substitution chase walked only what PHP inherits.

Now a parameterised `@mixin` is lowered into `supertype_arguments` beside the `@extends` tags -- the
ids `docblock_mixins` already produced, since lowering a tag twice records each name in it twice and
a missing mixin was reported twice -- and `type_argument` walks a written mixin as a parent. Methods
returning `$this` are untouched: they resolve against the receiver, which is what Laravel's
decorated forwarding hands back (`$posts->where(...)` is still the relation).

The first version walked mixins after everything inherited and typed 192 of BookStack's relation
reads as `stdClass`. `Relation` implements the Eloquent builder contract, which extends the query
builder's contract, which is `@mixin \Illuminate\Database\Query\Builder` -- and that one
`@use BuildsQueries<\stdClass>`. Reached through an interface, it answered before the relation's own
mixin. So the chase is two walks: inheritance alone first, which changes no answer it gave before,
and only where that finds nothing a second in which a class's own mixins come before anything it
inherits. BookStack resolved 96.4 -> 97.0 % of its member accesses; 212 findings appeared, all in
`tests/`, all `->first()` on a relation dereferenced without a check, which is the strictness the
`::first()` on a model already had.

## A shape is sealed, and three readers had to learn what a key set is first

`array{foo: int}` and `array{foo: int, ...}` lowered to one node, so a key the shape did not name
could never be a finding -- the comment on `record_against_shape` said so. They are two now
(`TypeRef::Shape { open }`), sealed unless the shape says `...` or lost a key at lowering, and a
value's keys are judged against a declared shape in one function, `keys_against_shape`, whether the
value is a literal, a shape written in the file, or one from the index.

**Who seals.** Psalm and Mago always; PHPStan only under `bleedingEdge` (PHPStan 3's default). Plain
PHPStan 2.3 at level 5 accepts `array{foo: int, bar: 2}` for `array{foo: int}` -- checked against
the reference project's own vendored copy, which is why its CI is silent on what phrost now reports
there. `[]` for a shape with a required key is the exception, reported at level 5 without the flag.
Sealing was the user's call (2026-10-22), knowing it costs the reference project ~36 findings, all
of them docblocks such as `@param array{send?: mixed, provider?: mixed} $options` handed an array
with other keys -- each fixed by adding `...` or the keys, which satisfies both tools.

**What the rule could not see until three readers changed.** Each was invisible while extra keys
were never judged, and each read as a false positive the moment they were:

- _A key written after the declaration._ An element write on a parameter declared `array` made the
  binding "the write that created an array" (`locals.rs`, the `created` fallback), throwing the
  declared shape away -- so `$row['bar'] = 2` left `$row` a bare `array`. For a sealed shape the
  parameter binding is kept, and `local_held` reads the shape as the record it is and folds the
  writes in exactly as it does for a literal (`sealed_shape_as_record`). An open shape is not
  folded: it can neither be shown to lack a key nor to carry one, so the stale key set costs
  nothing.
- _A key `unset` took out._ `unset` had no HIR at all -- a placeholder statement. It is
  `UnmodelledKind::Unset` now, its targets are body-fact sites of their own (apart from writes, so
  no guard on the variable goes stale), and `FilledSlot::Removed` takes the key out of the record
  when the removal certainly ran. `unset($row[$key])` removes nothing, which errs the way a write in
  a branch does.
- _A shape written in the file._ `describe_written` answered `Unknown` for a shape, so a parameter's
  own `array{foo: int, buz: float}` handed on was the one spelling nobody compared. It is a
  `ValueTy::Record` with each key's `optional` and the shape's `open` now.

A sealed shape's optional key is a key it may hold: `array{title?: mixed}` handed to
`array{owner?: mixed}` is refused, as PHPStan's bleeding edge refuses it. An open value against a
sealed expectation answers `Unknown`, never `Rejected`.

## A self-out is a `@param-out` about the receiver, and a chain off a local never asked either

`@phpstan-self-out self<U>` (and `@phpstan-this-out`, `@psalm-this-out`, the bare spellings -- mago
folds them into one tag value) is stored as `Contract::ReceiverOut`, its slots read by the reading a
`@return Bar<T>` already has (`applied_arguments`), and applied in `contract_narrowed` like a
`@param-out`: the call-statement walk reports the receiver as one more way of being handed over
(`PassedAs::Receiver`). Two decisions are load-bearing. `PassedPaths` records a receiver only for a
method name in `ProjectSkeleton::receiver_out` -- the `NeverReturns` shape, since every receiver of
every call is most of a file and a path with statements recorded is one the walk runs for. And a
receiver reached through a call (`$repo->items()->transform(...)`) is never recorded: the next
`$repo->items()` may be another object. The contract slot holds one promise and a return contract
ranks first, which costs exactly Laravel's `PendingRequest::async()`, where both say the same.

**The trap was not in the feature.** The conformance shape passed on the first build, and
`$users->transform(...); $first = $users->first();` still read the old element: `held_after_steps`
asks the contract question about the _whole_ path, and no call before `$users->first()` was handed
`$users->first()`. The bare `$users` -- what goto, hover and every member check resolve as a
receiver -- answered with the call's claim and the chain off it, which is what an assignment holds,
answered without; the same was true of every `@phpstan-assert` and by-reference write all along
(`assertInstanceOf(Logger::class, $logger); $h = $logger->getLogger()` typed nothing). A local root
with steps now resolves as the bare local first, through the memo. It moved findings only through
those older contracts: Laravel +15, Symfony +8, BookStack +1, all type-level true positives PHPStan
reports too (`AbstractStream::getHost()` after asserting an `EsmtpTransport`, `parse_str` results
handed to `string`), and WordPress -2.

Reaching the chain exposed two older approximations in the assertion reader, each fixed where it
lives: a tag naming the class the value already is _replaced_ it, so
`assertInstanceOf(Builder::class, $results)` turned a `Builder<User>` into a bare `Builder` and
every `$results->first()` a `?Model` (`narrows_to` now keeps the value unless it carried no
arguments); and a kind tag whose argument is a template -- PHPUnit's
`@phpstan-assert iterable<T> $haystack` -- put a class called `T` into every element read off the
value.

Left out, measured: the same question at an intermediate _property_ (`$this->items->first()` after
`$this->items->transform(...)`; the whole `$this->items` is narrowed already). On Symfony it bought
no answer and lost two, because an asserted subclass drops the type arguments of what it narrows --
`assertInstanceOf(ArrayCollection::class, $c)` on a `Collection<int, A>` leaves a bare
`ArrayCollection`, and `also_of_class` does the same for `instanceof`. That loss is the thing to fix
first; then the prefix question is free.

**The conformance verdict cannot be passed by the right answer.** `assertions_this_out_self_out`
marks line 58 (`takesIntBox($box)` after the replace) only as `E<phpstan>`, `E<psalm>` and
`E<phpantom>`, with no plain `E`, so an engine without its own marker is required to stay silent
there -- the unmarked engines that pass report line 53 alone. phrost now prints PHPStan's two
findings, lines 53 and 58, and still reads `Fail` until the suite carries an `E<phrost>` at 58.

## Two readers of a value's type arguments, and a union one level deep

Found by reading BookStack's coverage list, where `$chapter->pages->first()` and
`$shelf->books()->first()` were the top two untyped assignments.

- **One argument on a container is its value -- for a `foreach`, and not for a member.**
  `@property Collection<Page> $pages` gives a `<TKey, TValue>` collection one argument.
  `IteratedSlot::written_position` read it as the value; `supplied_argument_at`, which binds a
  member's return, read it by position as the key. Two constructors of one rule, divided along
  _which question_ was asked of the same value, so `foreach ($chapter->pages as $page)` typed the
  page and `->first()` typed nothing. One rule now, gated the same way (`!iterates_its_properties`):
  a class PHP iterates for what it yields. BookStack +42 accesses.
- **A template inside an intersection inside a union.** `BelongsToMany::first()` returns
  `(TRelatedModel&object{pivot: TPivotModel})|null`, which interns as a nullable intersection.
  `substituted_union` took the top level apart and kept an operand that was itself a composite
  whole, template name and all -- `names_something_real` answers `true` for anything that is not a
  `Named`, so the unresolved name went through as an answer. Nested composites are the same question
  again now, and an `object` beside a class folds out (`object{...}` lowers to `object`). BookStack
  +77, Laravel +34, no answer lost on eight projects.

Neither showed on the reference project, whose relations are Drupal's: a Laravel app was the only
instrument that could see them, which is §8.2's note about framework corpora and applications again.

## An override's refused `@return` read as one it never wrote

A `@return` naming the declaration's own template stores no type -- lowering refuses it rather than
intern a class called `TMapValue` -- so the slot it leaves is empty, and an empty slot is also what
an override that wrote nothing leaves. The flattener carried an overridden contract onto any
override with none of its own, and so carried `Support\Collection::map()`'s
`static<TKey, TMapValue>` over Eloquent's `map()`, which writes
`Support\Collection<TKey, TMapValue>|static<...>` exactly because the subclass cannot hold arrays.
Every `->get()->map(fn () => [...])` read as a collection of models holding arrays, and
`prepend([...])` on it as an `incompatible-argument`.

The parameter half had this fix for a year (`ParamRow::documented`, Laravel's `when()`); the return
half did not -- _two constructors of one rule_, split along which side of the signature.
`MemberRow::doc_ty_refused` is the same bit for the return, and a contract now carries only onto an
override that wrote no `@return`. It surfaced only because the contract for a class with arguments
had been read without the call's scope, so `static<...>` named nothing and the carried contract was
inert; giving it the scope armed both. Six false `undefined-method` on PSL and six on Symfony went
with it -- overrides narrowing their parent's return (`MutableSet` under `SetInterface`, ORM's
`ClassMetadata` under Persistence's) that the parent's contract had widened back.

The override's union became a contract of its own afterwards: `ReturnsApplied::alternatives` holds
the other classes one argument list fills, and the reader drops an alternative whose arguments break
a template bound -- `TModel of Model` holding an array is when Laravel hands back the base
collection. Monica's `->get()->map(fn () => [...])` reads as `Support\Collection<int, array{...}>`.

## `object{...}` was an `object`, and `path_of` of an assignment is its value's path

`object{pivot: Pivot}` lowered to the bare `object`, so `BelongsToMany`'s
`TRelatedModel&object{pivot: TPivotModel}` folded to the model and `->pivot` reached `__get`. It is
its own node (`TypeRef::ObjectShape`), deliberately not a flag on the array shape: every reader of
`Shape` reads an array -- subscripts, iteration, literals -- and a flag is a spelling each of them
would answer for without knowing it exists. Filled for one value it is a `Held::Object`, and
`object_shape_property` reads it before any class table. Two things followed that the next change
near here should expect. An intersection of one class and object shapes has to stay that class for
every single-class reader (`class_of_held`, `class_of_stored`), or `?(Vault&object{...})` silently
loses its null check -- the first build dropped 35 true `possibly-null-receiver` that way and read
as an improvement. And the substitution has two entry points, the declared-return ladder and a
contract branch's intersection operands; the object shape had to go into the shared
`substituted_by_receiver`, or a `findOrFail()` message carried `TPivotModel`.

Typing `->pivot` exposed a second bug. `assertNotNull($button = $flow->getClickedButton())` proved
its assertion about `path_of(<assignment>)`, which is the path of the _value_ -- the call -- and not
the variable: so the variable kept its `?T`, and the call's slot stayed narrowed after the variable
was reassigned. A statement call now hands over the target, positioned at the call's end, after the
write it holds (Symfony -32, Laravel -4). The same change on the _condition_ path made things worse
-- `if (endsWith($v, $after ??= $before))` read `$after` before the `??=` -- because a guard's base
value is taken at the condition's start; that path keeps the old reading until it can place a proof
after a write inside the condition.

## A strict `in_array` found the needle, and what it found it among is a type

`in_array($x, $list, true)` holding says `$x` is identical to one of `$list`'s elements, so it is of
their type -- a claim with two values in it, like `array_key_exists`'s, and built the same way:
`Proved::ElementOf` names the container, and the view meets the needle with the element type it
reads off the container's declaration (`found_among`). The elements win where they are narrower (a
`mixed` or `?User` needle in a `list<User>` is a `User`); otherwise the needle's alternatives that
provably meet no element go, and its null goes exactly when no element can be one. Only strict --
`in_array('1', [1])` holds -- and only the side that held: a needle not found is anything, the
elements' own type included. A literal haystack of scalars stays `in_array_of_non_objects`'s, which
answers first on both sides; the two now read "strict" through one function. Measured: six WordPress
`incompatible-argument` gone, every one a `$_GET['post_type']` read as `string|array` inside
`in_array(..., get_post_types(), true)`, and nothing else moved on seven projects.

## `=== [null]` failing pins the slot that differs, and nothing less certain does

`if ($row === [null]) { return; }` said nothing past it: the identity against an array was read only
as the non-null claim it makes where it holds. Where it fails it says the value is not that array,
and PHP compares arrays slot by slot, keys and order included -- so when every slot but one already
holds the literal's value, the one left is the one that differs (`Proved::NotIdenticalTo`, read in
the view by `not_identical_to`). `[$val]` with `?string $val` is then `array{0: string}`. Scoped to
what is sound: strict only; a literal of scalar literals under keys it settles; a value the view
holds as a `Held::Record`, because only a record states every key it has -- a list or an open shape
can differ by a key it never named; keys equal to the literal's in order; and a pinned slot or a
subtracted value only for `null`, `true` and `false`, the three literals that are a type of their
own. A string in the literal names one value of many and settles nothing either way. Measured: no
finding moved on seven projects -- the idiom is rare, and the conformance case is where it came
from.

## A union of one kind answered no kind, and a conditional settled neither branch

`kind_of_held` answered `None` for every `Held::OneOf`, on the grounds that several kinds are no
single kind -- true, and wrong for a union whose alternatives are all _one_ kind in different
spellings. `$s = preg_replace(...)` in one branch and not the other joins a built `string` with the
interned `?string` the call returned; `union` cannot fold the two, so the join stays a union of two
strings, and `($subject is string ? string : array<string>)` asked of it settled neither branch --
`string|string[]` for the next `str_replace`, an `incompatible-argument` wherever that went. Now a
union whose operands all answer the same kind answers it. Found because the by-reference loop rule
below propagated one; measured on its own, 11 false `string|string[]` findings gone (WordPress 9,
Laravel 1, Symfony 1) and nothing else moved on seven projects.

A second cause of the same `string|string[]` is _not_ this one and is still open: a chain of seven
or more `$s = f($s)` whose `f` has a conditional return on `$s` runs out of `FOLLOW_LIMIT`, and the
conditional reader answers both branches for an argument it could not resolve. Ten successive
`$s = str_replace('a', 'b', $s);` and a `strlen($s)` in a `strict_types` file reproduce it with no
loop and no union at all; WordPress's `wptexturize_primes` is the real instance.

## A loop by reference writes the container, and nothing at the header says so

`foreach ($ints as &$item) { $item = 'x'; }` leaves a `list<int>` holding strings, and phrost read
`$ints` past the loop as its declaration. The loop is a write with no assignment to the container
anywhere in the text, so it is found beside the assignments rather than among them:
`Scope::rewritten_by_reference` is a fourth candidate in `local_binding`'s ranking, positioned at
the loop's end, whose value is `PathRoot::RewrittenElements` -- the container read at the header and
every whole write the body made to the reference. The view (`rewritten_elements`) widens the element
by the writes the element does not already admit and keeps the keys; a write that changes nothing
(`$item = $item * 2` over `list<int>`) hands the declared container back untouched, so the common
transform-in-place loop costs no precision. PHPStan reads the same loop as `list<'x'|int>`; a union,
because the list may be empty and a pass may write nothing.

What it deliberately does not read, each a place where the container stays as declared, as before: a
loop inside a branch the read is past (the container on the path around it is not what it was at the
header -- joining the two is `maybe_written`'s job and is not attempted); a read inside the loop; a
body that writes the container itself or `unset`s the reference; a write _into_ the element
(`$item['k'] = 1`), which changes a slot rather than the element's type; writes to the reference
after the loop. A write nothing can name (a destructuring) leaves an `array` that says nothing of
its elements.

"The element already admits it" is asked by `fit_held_strictly`, the compatibility judgement with no
conversion: the file's own mode is right for a call and wrong for a slot, and a weak-mode file
admitted `'x'` into `int`. The strict-`in_array` meet asks the same way now, for the same reason.

Measured: four Laravel tests that convert rows with `$row = (object) $row` and pass them to
`processColumns()`'s `list<array<string, mixed>>` -- true by the declaration (PHPStan keeps a
constant array's shape through such a loop and misses them). One WordPress finding at
`wptexturize_primes`'s `implode()`, whose `string[]` is the `FOLLOW_LIMIT` cut-off described in the
note above, already visible on `main` as the type of `$sentence` after its last `str_replace`.

## `non-empty-string` is a type, and what holds one says so

Until 2026-10-25 every refinement of `string` lowered to `string`, so
`@param non-empty-string $name` took a `''` without a word and only a conditional return's test
(`$query is non-falsy-string`) ever read the word. The decision was to read it the way PHPStan does
at level 7: `string` passed, returned or assigned where `non-empty-string` is declared is reported,
because the empty string is a value of `string` the declaration refuses -- the same reason `bool` is
refused where `true` is declared.

**The representation is two `BuiltinTy` variants**, `NonEmptyString` and `NonFalsyString`
(`truthy-string` is the second's alias), and not a flag beside `string` the way `non-empty-array` is
a flag on `Generic`. The deciding fact is the three spellings a value travels in: a built
`Held::Builtin` (a literal, a cast, a `.`), a written `TypeRefData::Builtin`, an interned
`TypeRef::Builtin`. All three already carry a `BuiltinTy`, so a variant reaches every one with no
fourth shape and no allocation -- the judgement compares two enum values -- where a flag would have
needed a new `Held` arm for the built spelling, which is where most refined strings are born. The
cost is the one §8.1 names for this vocabulary: a reader matching `BuiltinTy::String` alone is blind
to the new two and compiles clean. Every such site was read; the rule is that a _kind_ question asks
`is_string()` or `unrefined()` -- `kind_of_held`, `written_kind` and `stored_kind` widen, so the
guards, the impossible-use checks and a conditional's `is string` answer as they did -- and only the
judgement, the joins and rendering see the refinement. `True`/`False` against `Bool` is the
precedent and was the model for every rule. The codec's two copies of the tag table became one while
adding the tags (cache version 80).

The pieces, each where the concept lives:

- **The judgement** (`compat::builtins`): strings compare by rank (`string` < non-empty <
  non-falsy), rejected below. Weak mode converts a scalar to what it spells -- an int or float to
  digits, so non-empty and not non-falsy (`0` spells `'0'`), `true` to `'1'`, `false` to `''`. A
  `class-string<T>` is non-falsy as a _value_ and still admits any string as a _declaration_, which
  is why it stays a generic over `string`: the base is what the expected side judges. A bare
  `class-string` is lowered as `class-string<object>` for the same reason, rendered `class-string`.
- **The contract** (`TypeTable::refined` and `phrost_hir::refined_type_ref`, the two twins): a
  docblock that is the hint's builtin narrowed refines it, operand by operand inside a union.
  Without it `@param non-empty-string` beside `string` lost to the hint and nothing was ever
  reported.
- **The joins**: `TypeTable::union` and `Held::one_of` fold two strings to the weaker, and
  `without_repeated_scalars` does it across spellings; two containers differing only in a string
  argument are one (`preg_split`'s undecided branches). Rendering folds the pair the constructors
  cannot see (`?string|non-falsy-string`).
- **What the syntax says** (`phrost_lower::string_holding`, one reader): a quoted literal by its
  text, an interpolation or heredoc by the text around its holes, a `.` and a `.=` by the piece
  holding the most, a constant's value the same way. What lowering cannot read the view does:
  `PathRoot::Concatenation` carries the pieces lowering could not type, and `(string) $x` is that
  root too -- `'' . $x`, digits for a number.
- **What the text says** (`quoted_string_holds`): `''` is `string`, `'0'` non-empty, anything else
  non-falsy; a double-quoted escape proves non-empty and nothing more. The contract settling of
  `$query is non-falsy-string` now reads the argument's _type_ first and the literal's text only for
  the one thing a type cannot say -- that a string _is_ empty. One rule where there were two.
- **The guards**: `$x !== ''` and the else of `=== ''` (`Proved::NotEmptyString`, null untouched);
  `$x != ''`, `strlen($x) > 0` (lowered as `NullTestKind::Length`, since `'0'` is one character long
  and still empty to `empty()`) and `is_numeric()` (`Refused::TheEmptyString`, null and `false`
  too); truthiness, `!empty()` and `?:` (`EveryFalsy`, non-falsy);
  `str_starts_with`/`str_ends_with`/ `str_contains` with a literal needle (the needle's holding); a
  loose `in_array` over string literals (their weakest holding, capped at non-empty when one spells
  a number); `@assert-if-true non-empty-string` and `@phpstan-assert`; `array_filter()` with no
  callback (non-falsy elements). The skipped path of `if ($s === '') { $s = 'x'; }` is
  `Otherwise::NotEmptyString`. All through one helper, `with_strings_holding`, which raises the
  strings of a value in any spelling and leaves a `mixed` alone.
- **The stub set**: what the engine always returns non-empty -- `md5`, `sha1`, `hash`, `uniqid`,
  `serialize`, `spl_object_hash`, `realpath`, `getcwd`, `json_encode`, `date`, and eight constants
  -- says so; see the set's README. And `sprintf`/`vsprintf` hold what their format's text and
  widths do (`literal_returns`).

Three bugs this exposed, each older than the type and each the §8.1 shape of a claim lost in
composition: **a contract composed on two nested claims** re-anchored with `about()`, which cut the
nest at its outer claim -- `$x === null || $x === '' || $v->ok($x)` put the null back
(`anchored_on`); **a claim about a slot** resolved where the guard stands read the slot through an
unnarrowed base -- `$row === [] || $row['k'] === ''` handed back the absent key's null
(`through_the_base`); and **a strict `in_array` over a written list** never narrowed, because a list
literal is a record and `element_of_held` refuses a record -- it reads the record's values now,
which also narrows a needle found among enum cases. And `FOLLOW_LIMIT` went from 16 to 24: each
`'' === $s` is a hop, and Symfony's `AnsiUtils::visibleWidth` ran out exactly there (20 was measured
and was not enough).

**`numeric-string` is deliberately not a non-empty declaration.** Lowered as one it made every
`@param numeric-string` a contract a plain `string` fails -- `bcadd()`'s operands, 36 findings on
the reference project -- and whether a string spells a number is a question nothing else here asks.
The numeric strings that must count as holding something do so another way: `(string) $int` by its
operand and `is_numeric()` by its guard. `lowercase-string` and `literal-string` stay `string`;
their non-empty forms are non-empty.

Measured on 2026-10-25 against `main`, sites that appear or disappear (re-renderings of the same
finding excluded): reference project and propertysmart **0 / 0**; BookStack **+4**, Monica **+4**,
Laravel **+11 / -1**, Symfony **+65 / -9**, WordPress **+32 / -33**. Of the 116 added, **95 are
PHPStan's own finding on the same line** (level 7; one BookStack receiver at level 8, a fix of the
composition bug above making `$entity` the `Page` it is). The 21 PHPStan does not report:

- **the stub set is stricter than PHPStan's map** (9): `extension_loaded()` and `class_alias()`
  declare `non-empty-string`, `version_compare()`'s operator a literal union, `listIdentifiers()`'s
  country code `non-empty-string`. True by the declaration.
- **PHPStan's regex and int-range inference** (5): `preg_match()` proving its subject non-empty, the
  pattern's groups typing `$matches`, `substr()` with a positive length -- WordPress's interactivity
  API and typography.
- **`str_replace`/`strtr` preserving non-empty** (2), which PHPStan's extensions read.
- **tests passing `''` on purpose** to a `non-empty-string` parameter to provoke the exception (2).
- **phrost's own models** (3): `$_FILES`'s shape against WordPress's declared non-empty one,
  `$_REQUEST['role']` as `string|array`, and a `$_POST` value `sanitize_key($p) !== $p` proves is a
  string, appended to an `array_filter()` result that is now a list the append folds into.

Every removal was a false positive: 23 WordPress `possibly-false-argument` behind `strlen($x) > 0`
(a `false` measures nothing), the composition fixes, the strict-`in_array` record, and the budget.
Conformance 282 -> 283 of 292 (`regressions_string_narrowing_assert_if_true`), nothing lost.
Coverage: Laravel unchanged in counts with 2 604 answers refined; Symfony +50 accesses resolved and
+8 assignments typed. `check --jobs 1` on the reference project 2.00 s -> 2.00-2.06 s across three
alternated runs each (noise), WordPress 4.32 -> 4.39 s; perf p95 unchanged on every row (diagnostics
13.7 -> 13.9 ms); index memory 110.7 MB both, 66 094 -> 66 679 distinct types.

## `$at > 0` ruled null out and kept the `false`

The relational guard was written and measured for null alone, and `false` compares the same way: a
bool is compared as a bool against everything, and `false` is the least of them. On PHP 8.5.10
`false > x` is false and `false <= x` true for all eighteen operands tried, `null`, `[]` and an
object among them -- exactly the two directions `CompareOp::null_holds_whatever_it_faces` already
answered for null. So `if ($at > 99999999999) { $at = intdiv($at, 1000); }` over the `int|false` of
`strtotime()` reported `possibly-false-argument`, and `return $at > 0 ? $at : NULL;` reported
`int|false|null` against `?int`, both inside a guard PHP makes sound. The claim is now
`Refused::TheSentinels` rather than `NullOnly` -- both sentinels and still nothing about truth,
since `0 > -5` holds.

This is the _three spellings_ form again, with absence as the thing spelled: a rule about "the value
that is not there" written on one of the two sentinels. When a guard rules null out, ask what it
says about `false` before shipping it.

Measured: the reference project loses exactly the two findings reported (`RequestData.php:172` and
`:175`) and gains none, in two A/B pairs taken minutes apart on a tree being edited at the time;
WordPress -6 (`possibly-false-argument` 1673 -> 1671, `false-argument` 43 -> 41, one
`possibly-false-assignment`, one `incompatible-assignment`), Symfony `possibly-false-argument` 2380
-> 2376, Slim, Laravel and BookStack unchanged.

## A flag outlived its subject, and was read off a write that did not reach the test

`$ok = $item !== null; $item = $next; if ($ok) { $item->name; }` was silent. A flag's claim is
resolved where the flag is _tested_, so the non-null proved about the old `$item` narrowed `$next`.
PHPStan reports it. Found by running mago 1.59.0's own analyzer cases through `check` with PHPStan
as the oracle (`issue_2601.php`, `reassigned_subject`). Eleven probes followed: phrost was silent on
all eleven, PHPStan reported seven, and those seven are exactly what phrost reports now.

Both halves live in the one place a flag is read, the `Variable` arm of `proved_about`:

- **Staleness.** A write between the flag's assignment and the test drops the claim if it _may_
  reach the test, not only if it certainly does. A guard's own staleness check asks for certainty,
  and that is right for a guard: it tests its subject where it stands, and the binding join covers a
  write that may have run. A flag is the one way a test outlives the value it tested, so here the
  join is exactly what the claim would wrongly narrow. A write to a value the subject is reached
  through counts too (`$h = $o` for a claim about `$h->source`, via `covers_slot`).
- **Which write.** "Latest by position and nothing about branches" was written down as a decision,
  and it was the bug: `if ($c) { $a = $x; } else { $a = $item !== null; }` read the else arm's write
  while the then arm's reaches the test as readily. Now newest first by reach, back to the first
  write that certainly happened. A literal that cannot test the polarity asked about (`$ok = false`
  before `if ($c) { $ok = ...; }`) is passed over. Two remaining candidates prove nothing, and
  neither does a compound write.

What the second half had also been doing: `$fields ? $fields[0] : 0` in WordPress's PHPMailer
answered `non-falsy-string` from `$fields = $fields[0]` in the _exclusive_ `if` arm above it. `''`
arrives there.

`never` is judged by syntax in this reader, because `Flags` does not carry the reader's answer. A
write in a branch ending in `fail()` therefore counts as reaching the test, which can only drop a
claim and never invent one.

Measured. Of the mago cases, +1 (`issue_2601:164`, as expected). On the corpora:

- Laravel -1: `EnvironmentEncryptCommand.php:124`. Hover said `null $key`, "narrowed by a guard",
  for a key the code had just generated. It now says `string`.
- Symfony -1: `BinaryFileResponse.php:93`, which reported `File|SplTempFileObject` where only `File`
  arrives.
- Symfony +2: `AbstractDumper.php:140-141`, from
  `if ($returnDump = true === $output) { $output = fopen(...); }` and then `if ($returnDump)`.
  PHPStan reports the same two sites as `resource|false`. phrost says "may be null" because it
  cannot tie the flag to the branch that wrote, which is the correlated-branch gap.
- Reference project, WordPress, BookStack, Slim and psl: unchanged.

Coverage dumps: two WordPress answers moved, both from wrong to right; the reference project,
BookStack and Laravel are byte-identical. `check --jobs 1` on the reference project read 2.04-2.15 s
before and 2.09-2.13 s after across three alternated runs. Every perf budget passed (diagnostics p95
14.8 ms), and cold == warm.

The general form: **a remembered test is a test about a value at a time.** Anything that stores a
claim needs the window between storing it and reading it checked, and "the latest write" is a
question about reach, not about position.

## A typed constant's hint won outright, and a hint is the contract only where something can override it

`private const string CERT = __DIR__ . '/certs/server.crt'` read as `string`, so every call handing
it to a `non-empty-string` parameter was a finding. PHPStan reads the value. An untyped constant
already was its value; the hint is PHP 8.3's, and it replaced the value instead of bounding it. The
value now refines a `string` hint the way it already refined a bare `array` one, through
`doc_type_ref`, in `Lowerer::constant_hint_refined`.

The first version refined every typed constant, and a test pinned the reason not to:
`$this->resource::STATUS_OK` reaches whichever class the object is, and a subclass may redeclare a
public constant of an open class as `const string STATUS_OK = ''`. For such a constant the hint is
the contract the redeclaration keeps, and the value describes one class only. The reader does not
tell `self::X` from `static::X`, so the declaration decides for both: the value refines the hint
only where nothing can redeclare the constant -- private, final, in a final class or in an enum.
`bool` is never read off its value, because a flag constant is written to be flipped by editing it.
The `array` arm predates this and is not gated; gating it would cost measured table reads.

**The half not taken, and the measurement behind the refusal.** `$row[self::KEY]` into an
`array{...}` answers the union of every field, typed constant or not, because phrost has no literal
types and the index does not keep a constant's value as text. Before building that, the sites were
counted by hovering the base of every `$x[Class::CONST]` in seven projects. On the reference project
there were 17 such sites and none was a shape. Laravel, BookStack, psl and WordPress (176 sites)
held no shape bases either. Symfony had 7 of 539, all `...<string, mixed>` open shapes where the
union is `mixed` anyway. That makes 0 findings on 7 corpora, against a new kind of indexed data.
Re-open it only with a project where it moves a number.

Measured: psl -5 (exactly the five `self::CERT_FILE`-shaped false positives) and every other corpus
unchanged. Coverage moved 21 answers on the reference project and 3 on psl, every one `string` ->
`non-falsy-string`/`non-empty-string` off a private or final constant. Index memory 111.0 MB both,
distinct types 67 038 both, cold indexing 1.19-1.21 s both, `check --jobs 1` 1.94-1.99 s both, and
cold == warm on psl.

Two things worth knowing next time. The ranking that put this second, _605 typed constants on the
reference project_, counted the **construct**, not the **bug**. Counting findings the bug causes
took one script and reordered the list; do that before ranking, not after. And the cache needed no
version bump: `build_stamp()` already refuses rows any other binary wrote, so `VERSION` is for a
change of stored _shape_ only.

## An identity with a class constant proved nothing, through `?->` or without it

`if ($record?->status !== PreferenceStatus::Requested || $record->baseQuote === NULL) { throw ...; }`
reported `$record->baseQuote` as a possibly-null receiver. So did four more lines in the same new
module of the reference project -- 5 of its 197 findings, all false. The nullsafe arm read
`?->x !== null`, `instanceof` and bare truthiness, and nothing compared with a value. Under it lay a
wider hole: `?Status $s === Status::Draft` did not narrow a plain variable either. PHPStan narrows
both.

The body layer cannot settle it, and says why in `identity_against_a_non_null`: `X::Y` is an enum
case or `const NONE = null`, and only the declaration knows which. So the identity is _named_ --
`Proved::SentinelsRefusedBy { of, other }`, `PathRoot::SentinelsRefusedBy` -- and the reader
resolves `other` and takes off whichever of null and `false` it cannot be. The same claim serves the
nullsafe receiver: a chain cut short is null, so an end that matched a non-null value proves the
receiver was there. Named for the effect rather than as "identical to", because the receiver is
_not_ identical to the constant, and a later reader widening the rule to "narrow to the other's
type" must not apply it there. A literal on the other side is settled in the body as before, now for
the chain too, and so is `in_array($o?->x, [literals], true)`.

**`null_reach_of` answering `Impossible` is not proof.** It answers that for `mixed` as well,
because a check must stay quiet about a value nobody typed. Read as "the constant cannot be null", a
`const mixed ANY` would have taken the null off `$x === C::ANY`. `IndexView::proves_absent` is the
question a guard asks of _another_ value's type, and it excludes the absorbing types. A sabotage of
it fails the view test on exactly that row.

**An instrument note, paid for again.** The first A/B ran the new binary as
`./target/release/phrost` inside `(cd $probe && ...)`. The relative path resolved inside the probe
directory, the binary was not there, the error went into a pipe, and "no findings at all" read for a
minute as the change having silenced a real `null` (`$s === K::MAYBE`, which PHPStan reports).
Absolute paths for every binary in an A/B loop.

Not taken: a _variable_ on the other side (`$v?->prop === $expected`, mago's issue 2591). Each side
of `$a === $b` narrows the other, so resolving one inside the other's guard is a cycle the budget
would cut arbitrarily rather than a rule. The haystack of `in_array` as a variable
(`list<string> $groups`) needs the element type, and `ElementOf` is about the needle, not its
receiver. Neither moved a finding on nine corpora.

Measured: the reference project -5 (exactly the five), Symfony -1
(`if (FunctionReturnType::Value !== $returnType) return;` then `$returnType->name`), seven other
corpora unchanged, and nothing added anywhere. mago's `issue_2599` lost its 7 false positives.
Coverage moved 6 answers, `null=Possible` -> `Impossible`, each one a removed finding.
`check --jobs 1` on the reference project read 1.96-2.80 s before and 1.97-2.24 s after across six
alternated pairs, outliers on both sides on a loaded machine, medians ~2.0 s for both. On an idle
machine, two alternated perf sweeps read diagnostics p95 13.01/13.31 ms before and 13.30/13.43 ms
after, every other row unchanged, and index memory 111.0 MB with 67 038 types for both. A sweep
taken while another project's release build ran read 30.85 ms and called it over budget; §8.2's
warning, met in person.
