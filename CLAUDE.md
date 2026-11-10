# CLAUDE.md — phrost (fast, lean PHP LSP)

phrost is a PHP language server in Rust. The **one rule everything is judged by**: it must be **fast
and resource-cheap** — _not_ "PhpStorm by resources." A change that adds hot-path latency or makes
memory grow with `vendor/` size is wrong by default, however correct the feature is.

## Project state — read this first

A working server, measured on a production Drupal 10 checkout: 30 916 files indexed in 2.7 s at 251
MB RSS, 98.4 % of member accesses in user code resolved and 91.9 % of assignments typed, all
seventeen request budgets met and most of them used at single-digit percent. The previous 141k-LOC /
9-crate attempt (not published) lives at `../phrost-old` and is **postmortem + salvage source
only**; never copy its structure, and never cite it as precedent for how something is done here. Its
fixtures, though, are worth scoring against — see `just corpus`.

The plan is these three documents, and they are authoritative until code contradicts them:

| Doc                       | Owns                                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `docs/01-architecture.md` | crate graph, parser boundary, index boundary, two-tier store, framework seam, why projections are the unit of cost |
| `docs/02-testing.md`      | the eleven test levels — **the most important document**                                                           |
| `docs/03-roadmap.md`      | where this stands, what is next and why, what is refused                                                           |

`docs/comparison.md` is not one of them — it is a measurement, refreshed by `just compare`. Nor is
the `docs/08-*` family, which is the record of bugs already paid for, split by area; §8 says when to
read which.

If code and docs disagree, fix whichever is wrong — but say which one you changed and why.

---

## 1. The load-bearing rules

Everything else follows from these. They exist because the previous attempts violated all of them
and died of it.

### 1.1 Boundaries are the dependency graph, not the docs

`phrost-old` had a correct `ARCHITECTURE.md` saying "`phrost-analysis` must not contain PHP
semantics" — and grew a 28,520-line `phrost-analysis` reaching into `file.parsed.codebase_metadata`.
A rule with no CI check does not hold.

So: a `dependency_rules` test holds an **explicit allowed-edges matrix** for the workspace. mago may
appear only in `phrost-syntax` + `phrost-lower`. `phrost-analysis` may not depend on any store
crate, only on `phrost-view`. Adding an edge is a deliberate allow-list edit with a justification in
the commit — never an incidental `use` during a refactor.

### 1.2 Zero fallbacks

The index either has an answer or returns a deterministic _not found_. There is no "try the index,
else re-parse / FS-scan / fuzzy-match / hardcode." `phrost-old` had **seven** such conditional paths
and they were the entire cause of "it works sometimes."

This is enforced, not requested — but by an allow-list, not by the `deny-fallbacks` cfg this
originally described. A cfg only fires from code that calls it, so it can only catch a fallback
whose author remembered to mark it, which is the author who would not have written one.

What a fallback actually _is_ in a language server is a **read**: something asks the index, gets
nothing, and goes to disk to make up for it. So the reads are enumerated. `no_fallbacks` holds a
file-by-file allow-list with a reason for each, and a new read fails the test until somebody writes
down what it is for. Tests and the CLI are unpoliced — neither is on the path a request takes.

To make the rule survivable, the skeleton must store what fallbacks used to recover — see §5
survivable-data rule.

### 1.3 Minimize the conflict surface

Two people — or two agents — working on different features must not need to edit the same file. This
is the practical half of "a change lives in one place": if every feature edits the same dispatch
match, the same `lib.rs`, and the same expected-output file, then work does not parallelize no
matter how clean the layering looks on paper. Going slower here is faster later.

Concrete rules, in rough order of how much damage they prevent:

- **No shared expected-output file.** Snapshots live inline as `expect![[...]]` next to the test
  that owns them. A single `expected.txt` / `xfail.txt` covering the whole suite is a permanent
  conflict magnet — every change to any behavior rewrites the same lines. phrost-old had exactly
  that, and it is the single worst offender.
- **No central dispatch `match` over features or frameworks.** Register trait objects in a list; the
  generic layer iterates the list. Adding a framework then adds a file, not a match arm that three
  other branches also touch.
- **One feature per directory**, with its own tests. Registration in a shared list is at most **one
  line**, and that line is the only shared edit a new feature is allowed to need.
- **Prefer `pub mod` over a growing `pub use` list** in `lib.rs`. A re-export block that every
  feature appends to is a conflict magnet for zero benefit.
- **A test file per behavior, not per crate.** Two people adding unrelated tests should be adding
  two files.
- **No `utils`.** A shared grab-bag is edited by everyone and owned by no one; helpers belong in the
  crate that owns the concept.

Rule of thumb when reviewing a design: ask what the diff looks like when three people implement
three unrelated features on top of it. If all three diffs touch the same file, the design is wrong
even if each change is small.

### 1.4 Invariants are harness dimensions, not tests

`open == closed`, `cold == warm`, `incremental == from-scratch`, `framework-agnostic` are
**dimensions every fixture runs through**, so one new fixture buys five invariants. `phrost-old` had
`open_vs_closed_parity.rs` as _one file_ instead of a dimension over its 392 fixtures, and had
**no** incremental-vs-scratch check at all — which is why invalidation bugs shipped.
`incremental == from-scratch` needs a canonical `dump-index` renderer; build it in Phase 2, it
cannot be added retroactively.

---

## 2. How to work: parallel agents + model per task

The process every server in this family is worked on by -- delegating to agents with disjoint write
sets, the hazards that leak through worktrees, the gate, measuring, code quality -- is lspkit's,
shared with orthra and imported here whole:

@../lspkit/docs/working.md

What is phrost's own is below: which work goes to which model in this repository, and the recipes
that fit it.

### Model tiers — pick by the risk the work creates, not by how big it looks

- **haiku** — analysis sweeps, code search and summarization, doc edits, fixture conversion,
  mechanical moves and renames, `UPDATE_EXPECT` acceptance passes. Use it for _understanding_ the
  codebase before touching it. Do not burn opus on reading.
- **sonnet** — bounded features, thin LSP handlers, DTO plumbing, tests, single-module refactors.
- **opus** — anything creating a long-lived boundary or touching core semantics: the crate graph,
  mago integration, `phrost-ty`, the lock/concurrency model, cache invalidation, the `phrost-view`
  surface, framework extension-point design, and **all perf/memory tradeoffs**. Also cross-crate
  regression debugging (stale index state, FQN/alias resolution, cancellation, document versioning).

Never use a cheap model for a decision that creates a boundary. Never use an expensive one to read
files.

### Fan-out recipes that fit this repo

- **Understand first.** Parallel haiku readers, one per subsystem, each returning a structured map.
  Synthesize, _then_ edit. This is how you change the right layer once instead of the wrong layer
  three times.
- **Mechanical bulk work → parallel haiku.** Converting the 392 `phrost-old` fixtures (split by
  directory), porting snapshot renderers, generating `test_data` files, cleaning duplicate fixtures,
  doc sweeps. This is the biggest genuine parallelism win in the project.
- **One phase per agent.** The roadmap phases are disjoint by design. Never run two at once.
- **Adversarially verify.** For any perf or boundary claim, spawn an independent verifier prompted
  to **refute** it: "prove this allocation is on the hot path", "prove open and closed return
  different goto results". A single agent's "looks fine" is not evidence.

---

## 3. Performance contract

Budgets live in `crates/cli/src/perf/budget.rs`; **that file is the source of truth**, not this
table. `just perf <project>` measures them against a real checkout and exits non-zero when one is
exceeded, so a claim about latency is checkable rather than argued.

| Request             | p95   |
| ------------------- | ----- |
| completion          | 12 ms |
| completion + commit | 20 ms |
| hover               | 12 ms |
| goto definition     | 12 ms |
| diagnostics         | 30 ms |
| inlay hints         | 12 ms |

Measured baseline on the Drupal corpus (30 965 files, 1 711 of them user code), for comparison
rather than as a target: a cold server ready in 1.6-2.3 s at ~193 MB (265-280 before
`lspkit_heap::Heap` took large blocks from the system itself), a warm one in 0.5-0.6 s at ~143 MB,
20 open files +1.7 MB, and p95 of 0.06-0.22 ms on goto/hover/completion over ~3 200 real
member-access sites, with diagnostics at ~12.5 ms p95 over 20 files -- measured with the checks the
project's `phrost.toml` switches on, which is what the editor runs and what `perf` ignored until
2026-09-25 (the default set alone reads 16 ms; see `docs/08-performance.md`). A warm start's 0.5 s
splits four ways, and `phrost index --warm` prints each: setup ~90-130 ms, the stored index ~140 ms,
discovery ~160-250 ms, indexing ~80 ms with every row reused -- so the two walks are half of it and
the cache a quarter. `phrost check` over the project's 1 749 files is 0.3 s on eight threads and 2.0
s with `--jobs 1`, which is the quiet sample a change to a check is decided by. Roughly half of the
serial figure is the syntax lint pass, which re-parses each file and which the checker now runs by
default -- the four categories that project's own `phrost.toml` asks for, so that CI and the editor
report the same file the same way. Measured on Symfony, where nothing is configured and the default
is two categories, it is 9.54 s against 11.40 s. The headroom is the point -- a change that eats it
is visible long before it reaches a budget.

Run the sweep on an idle machine. A concurrent `cargo build` inflates bootstrap from 2.4 s to 3.7 s,
which reads exactly like a regression.

**Memory rule:** RSS must be dominated by _user-code skeleton + a handful of open files_, never by
whole-vendor analysis. If RSS scales with `vendor/` size, that is the bug. (Known deliberate
exception carried over from phrost-old: the composer **classmap** is loaded fully, because non-PSR-4
classes like Drupal's root `\Drupal` are otherwise unresolvable. ~1-2 MB for Drupal.)

Measure before and after any change touching the index, parsing, the LSP runtime, or a hot path, and
report the p95 and RSS deltas. "It compiles" is not "it's fast." A perf-touching change with no
measurement is incomplete.

**Run perf sweeps sequentially.** Concurrent server instances cross-kill each other and produce
phantom `indexed=0` flakes that look like server bugs and are not.

---

## 4. Definition of done

1. `cargo check` + relevant `cargo test` green, `cargo fmt` clean.
2. New behavior has a snapshot test at the **layer that owns it** (see §6) — not only end-to-end.
3. Snapshots are of the **whole result**, never a substring match. A substring assertion lets ten
   junk completion items through, which is exactly how `phrost-old` hid regressions.
4. Hot path / index / parse touched → perf measured, deltas reported.
5. Resolution touched → `open == closed` parity confirmed.
6. Invalidation touched → `incremental == from-scratch` confirmed.

---

## 5. Hard NEVERs

- Never let mago types escape `phrost-syntax` / `phrost-lower`. mago 1.45 moved `ast` → `cst` and
  made the tree arena-bound (`Program<'arena>`); phrost-old pinned `=1.26.0` and would need a
  rewrite to upgrade. That is the whole reason this boundary exists.
- Never store a parsed tree in a long-lived structure. Parse into a per-file bump arena → lower to
  owned `FileHir` → **drop the arena**. `FileHir` must have no lifetime parameter.
- Never eager-analyze vendor or stubs, and never retain full analysis for closed files.
- Never add a fallback / fuzzy / FS-scan branch to mask a missing index answer.
- Never duplicate strings or parallel maps keyed by the same string. Intern names.
- Never put framework-specific branches (Drupal/Laravel/Symfony) in generic handlers, in parsing, or
  in `phrost-view`. They go behind typed contributor traits in `phrost-framework`.
- Never write a custom PHP parser / docblock parser / type engine while mago provides it.
- Never ship a perf-relevant change without a measurement.
- Never run two roadmap phases, or two agents with overlapping write sets, at once.
- Never suppress a `?T where T expected` diagnostic to reduce noise. Strict null-safety is
  deliberate; the way to keep it liveable is the zero-diagnostics corpus ratchet, not leniency.

### Survivable-data rule

A member's `definition_range` must come from a structure that survives **both** compaction **and**
disk-cache restore. Snapshot byte offsets do not — closed and cached files have empty text and yield
`0:0` garbage. This is what makes "no fallbacks" safe instead of fatal.

---

## 6. Crate map (target — dependency direction is downward)

```
phrost-lsp            protocol only: dispatch, version guards, cancellation, scheduling
phrost-cli            the checker, the perf sweep, the coverage sweep
phrost-analysis       features (goto/hover/completion/...) — no LSP types, no PHP semantics
phrost-view           the ONLY index API features may see; read-only projections
phrost-index          the two-tier store, the disk cache, framework contracts
phrost-ty  phrost-body  phrost-sema        types / bodies / hierarchies + member tables
phrost-defmap         FQN -> DefId, import scope, aliases, class_alias
phrost-hir            ====== BOUNDARY: no syntax exists above this line ======
phrost-lower          the only crate that knows the shape of the mago tree
phrost-syntax         the only crate that knows mago exists
phrost-twig           the only crate that knows Twig's syntax; a leaf, read by the view
phrost-framework      typed contributions; the only place a framework is named
phrost-vfs  phrost-project  phrost-stubs
phrost-model          ids, Name, TextRange, Visibility — and nothing else
phrost-intern  phrost-testkit  phrost-harness
```

Below all of it, outside this repository: **lspkit** (`../lspkit`, a git dependency pinned by
`rev`), the base shared with orthra. `lspkit-arena`, `lspkit-memsize` and `lspkit-heap` were
phrost's own; `lspkit-runtime` is the protocol `phrost-lsp` is written against
(`lspkit_runtime::protocol`) and `serve_stdio`, with the panic containment, write poisoning and
buffered transport; `lspkit-fixture` is the fixture syntax `phrost-testkit` reads; `lspkit-arch` is
the engine `dependency_rules` and `no_fallbacks` run on, while the rules themselves stay here. The
message loop under `phrost-lsp` is lspkit's own -- a task per request, `$/cancelRequest` that stops
one, notifications finished in order, panics answered as errors -- and no crate here names
`ls-types` or a framework: lspkit's `edges::KIT` refuses it, so the loop is lspkit's to change. A
change there lands here as a `rev` bump with this repository's gate run -- `just try ../phrost` and
`just bump` in lspkit, `../lspkit/CLAUDE.md` §2.

`phrost-syntax-features` is the one legal exception: syntax-level features (semantic tokens,
folding, formatting) that genuinely need tokens and trivia. It may depend on `phrost-syntax` and may
**not** depend on `phrost-view` or above.

Naming follows `docs/01-architecture.md` §6 (`*Id`, `*Ref`, `*Data`, `*Facts`, `*View`, `*Store`,
`*Query`, `*Builder`, `*Signature`, `*Resolution`, `*Catalog`, `*Contributor`, `*Site`, `*Scope`).
Don't invent a new family unless nothing fits — and don't force a bad fit into an old one.

## 7. Code quality

lspkit's `docs/working.md` holds the rules every server keeps (imported in §2): `forbid(unsafe)`
through inherited workspace lints, `panic = "unwind"`, `pub(crate)` by default, files under ~600
lines, one feature per directory, no `utils`, tests beside the code, comments that keep their
measurements. `lspkit_arch::baseline` checks the settings among them. What is phrost's own:

- Nothing in this repository may write `unsafe`, and nothing may depend on `lspkit-heap` -- the one
  crate that does, in lspkit -- but the server and the CLI, which `dependency_rules` tests.
- Avoid `pub(in ...)`.
- Prefer methods in `impl` blocks over free functions, even for helpers that only serve one type.

## 8. Gotchas already paid for (do not rediscover)

The record outgrew one section. What stays here is what you cannot know to look up: the shapes that
recur across every layer, the instruments and how they lie, and the hazards of working in this repo
at all. The **case notes** — a hundred and eighteen of them, each a bug paid for once — live in five
documents by area, and you are expected to read the one covering the area you are about to touch:

| Read before touching                                                                     | Document                 |
| ---------------------------------------------------------------------------------------- | ------------------------ |
| the type table, `Held`, guards, narrowing, docblock tags, templates, `compat`, any check | `docs/08-types.md`       |
| lowering, the def-map, catalogs, claims, discovery, the disk cache, warm starts          | `docs/08-index.md`       |
| the LSP runtime, bootstrap, client behaviour, completion, rename, highlight, tokens      | `docs/08-server.md`      |
| a hot path, memory, the allocator, parallel indexing, any perf claim                     | `docs/08-performance.md` |
| composer, Drupal, the stub set, docblock text, the formatter                             | `docs/08-php.md`         |

Add a new note to the document its area names, not here. Something belongs _here_ only if it would
have saved time for someone who did not yet know which area they were in.

### 8.1 The forms that recur

Nine shapes account for most of the record. Each has bitten three times or more, and the third
instance was never recognised as the same bug as the first. When something is wrong in a way that
matches one of these, go looking for the other instances rather than fixing the one in hand.

- **Two constructors of one rule.** One rule lives in two places and only one of them learns the new
  case. The tell is a **divergence**: one half answers and the other does not, and the boundary runs
  along a spelling or a place — `Held::one_of` against `TypeTable::union`, the contract reader
  against the generics substitution, `local_binding` against `bound_names`, the local's join against
  the slot's, two readers of a return tag, the text scanner against the resolver. An A/B matrix
  finds it. **Price the form, not the catch**: when the same shape recurs, the fix to reach for is
  the one that removes the shape — usually one seam both halves go through.
- **Three spellings of one thing.** A type arrives as `Held::{Builtin, Written, Stored}`; a union as
  `Held::OneOf`, an interned `TypeRef::Union` and a `TypeRefData::Union`; a name as `\Foo`,
  `namespace\Foo` and `Foo`. A rule written on one arm governs that arm and nothing else, and reads
  as covered because that arm has tests. The question is never whether the other spellings exist —
  it is **which helper already knows all three** (`union_operands`, `scalar_named`,
  `NameAnchor::of`).
- **Two spellings collapsing into one.** The mirror of the above, and worse: `list<V>` and `V[]`
  both interned to one `Generic{array,[V]}`, so the distinction was gone before any reader existed
  and no care above lowering could recover it. Fix where the collapse happens, and when two
  spellings genuinely do collapse, **render the weaker claim**.
- **A declaration on the _input_ side is a lower bound.** An interface's parameter (PHP is
  contravariant), a `@param` docblock, a by-reference parameter's hint. Four notes now say this; the
  next reader of one should assume it is a fifth before assuming it is new. A return is the other
  direction — it may only get more precise.
- **A reduction that makes an answer look _more_ specific hides every rule behind it.** The join
  reduced `?Row` beside `Row` to `Row`, on the grounds that both name one class -- so four rules
  about what the path that skipped a write proved could each answer "nothing" and never be noticed,
  because the operand they failed to narrow was then dropped. Nothing fails, and the answer reads
  like precision. The tell is a _confident_ answer where the code is not confident; the way to find
  the rest is to fix the reduction and read what appears. Full note in `docs/08-types.md`.
- **A degenerate form a constructor does not fold out.** A nested union, a `?T` operand, a repeat.
  Every narrowing works by taking a union apart and keeping the operands that survive, so an operand
  that is itself a union answers nothing about its own class or kind: the guard runs, returns a
  type, and is powerless. Nothing fails. When a layer starts _building_ a type rather than pointing
  at one, its degenerate-form list is the first thing to revisit.
- **A prefilter the index can hold (the `NeverReturns` shape).** When something expensive is asked
  constantly and answers the same way almost always, look for a fact the index already holds that
  rules the question out, before accepting the cost as a ceiling. Full note below; the shape has
  since paid for `declares_never_returning`, `assigned_properties` and the completion letter mask.
- **A catch-all answering a question its inputs could answer better.** Count the inputs first — a
  closed list of fourteen is not a fallback, it is a table. The converse holds too: adding a variant
  to a vocabulary closed only by convention (`BuiltinTy`) compiles clean and changes behaviour
  silently everywhere, so treat it as a semantics change needing the ratchet.
- **A property of the tree recomputed per position.** Split the rule: the half the tree settles
  (which writes a body holds, which branch each sits in, which conditions guard which spans) is
  derived once per revision; the half the position settles is applied per candidate. Where the tree
  genuinely cannot settle something, keep it _as the question_ (`Leaving::Ask`) rather than
  hard-wiring an answer.
- **A walk that matches roots by _shape_ stops at the first shape nobody added.** Make the match
  exhaustive and name every arm that is not a match, rather than falling through a `_` — a tenth
  variant must not answer "not one of these" quietly.

Two more that are about _where_ a bug can hide rather than what it is. **Only a driven `serve` sees
a warm start**: `phrost check` and `phrost query` are both cold, so a matrix of thirty probes
through them can pass while the thing a person uses is broken — `tools/lspclient.py` exists for
exactly that. And **a rule can be right and still buy nothing because a different rule is the
blocker**: counting what a rule would fix is only meaningful once everything between it and the
answer is known to work, so a zero often means "something else is also missing".

Three of these are kept whole here rather than in an area document, because each is the note the
shape above it was generalised from and none of them belongs to one layer.

- **`NeverReturns` used to be a resolution, and the fix was a filter rather than a budget.** The
  closure ran a full `resolve_path` at the whole `FOLLOW_LIMIT` to learn one bit, so calling it from
  a walk that runs per branch per lookup took diagnostics p95 from 13.5 ms to 51.5 ms -- 21 ms past
  budget, every test still passing. The way out is that the question is _exactly_ decidable from the
  callee's **name**: a call can only leave for good if the thing it names is declared `never`
  somewhere, so `ProjectSkeleton::declares_never_returning` answers no without resolving anything.
  Measured on 1662 real files: **4579 asked, 11 resolved** -- 99.76 % of the resolutions gone, and
  what is left scales with how much a project uses `never` rather than with how much code it has.
  The lesson generalises past this closure: when something expensive is asked constantly and answers
  the same way almost always, look for a fact the index can hold that rules the question out, before
  accepting the cost as a ceiling. What is _not_ generalisable is the size of the win -- do not read
  "four names on a Drupal checkout" as a property of PHP; a codebase built around `never`-returning
  guards puts more names in the list and resolves more often, and is still better off.

- **Two diseases share one symptom, and they need different instruments.** _Two constructors of one
  rule_ shows up as a **divergence**: one half answers and the other does not, and the boundary runs
  along a spelling or a place -- written against interned, `T[]` against `array<K, V>`, the prefixed
  tag against the plain one, the contract reader against the generics substitution. An A/B matrix
  finds it, and that is what found all four. _Zero constructors_ -- a rule with nowhere to live at
  all -- shows up as **uniform wrongness**: `?` inside a union arrived identically from all nine
  places that built a nullable by hand, so A/B compares two wrong answers and agrees. What finds
  that one is reading the text of an answer that "works". Twice this session: `string|?int`, which
  looked cosmetic and was an inference bug, and a `T[]` that leaked into the render. So when a
  comparison keeps agreeing, stop comparing and read.

- **Two resolvers, each complete for its own kind and blind to the other's, is a _form_ -- price the
  form, not the catch.** `@return TDefault|TInput` on Symfony's `InputBag::get` needs an argument's
  type for one operand and the receiver's `@var InputBag<string>` for the other; the contract reader
  holds the call and the generics substitution holds the receiver, and a union spanning both
  resolved in neither. Counting instances argued against fixing it -- one method in a 300 MB vendor
  tree, three findings on the reference project -- and counting instances was the wrong measure: the
  same form had been met four times already (element half against key half, written refiner against
  interned, two union constructors, two readers of a return tag) and every fix had treated its
  instance. The seam is one field: `CallFacts::receiver`, so both halves of what settles a return
  arrive in one place. When the same shape of bug recurs, the next fix to reach for is the one that
  removes the shape.

### 8.2 Instruments, and the ways they lie

§9 names the four instruments and §3 the budgets. These are the times one of them was believed and
should not have been.

- **`coverage` is blind to a function that does not exist.** It counts member accesses, so 66 calls
  to `t()` reaching nothing at all scored 100 % -- and stayed at exactly 97.8 % when indexing the
  files that declare it fixed them. The instrument answers "what does a receiver resolve to", and a
  free function has no receiver. A gap in free functions, constants, or anything else reached
  without a `->` has to be found some other way; the tell here was an `undefined-function` probe
  returning 79 findings that were all one of six names.

- **`rss after bootstrap` is not a load-immune number and 247.5 MB is not a constant.** Measured on
  one unchanged tree while other work built: 246.5-253.8 MB, with the before and after bands of a
  change that stores nothing overlapping completely. An RSS delta under about 6 MB on a busy machine
  says nothing at all. `phrost index --memory` reports the index heap with a per-row breakdown and
  is deterministic -- compare that, and use RSS only for an order-of-magnitude claim. On macOS the
  figure is the footprint since lspkit 0.1.1, read after the bootstrap's `release_freed`; `ps`'s RSS
  there counts pages already given back and never falls.

- **The perf sweep's `diagnostics` row is a knife-edge, and §3's warning understates it.** The same
  binary measured 41.32 ms, 29.39 ms and 23.42 ms against a 30 ms budget in three runs minutes
  apart, with maxima swinging four-fold across rows the change could not have touched. A concurrent
  build inflates every p95, not just bootstrap, and diagnostics sits close enough to its budget to
  flip. For a change on the diagnostics path, `phrost check <project>` wall time over the whole
  project is a far larger sample and much less noisy -- decide with that, then run `perf` for the
  record.

- **A probe whose two arms differ in two things cannot name either.** A report from outside had
  `@param mixed $ignored` + `class-string<T>` working and `@param Node|Node[] $nodes` +
  `class-string<T>` failing, and concluded the neighbouring union was the cause. The two arms also
  differed in the _return_ (`@return T` against `@return ?T`), and a three-by-three matrix over both
  axes settled it in one run: the neighbour was innocent and the return decided. Cheap to build, and
  it turns a wrong cause into the right one before any code is written.

- **Run `check` over the vendored stubs; it is a corpus nobody wrote for you.** 48 findings on
  `third_party/php-prelude`, 46 of them genuine defects in mago's set (bare `#[TentativeType]`, a
  relative `#[Mago\AvailableSince]` inside a namespace, an incomplete `ds.php`, a redeclared
  `final`) and 2 of them phrost's -- the enum/`UnitEnum` false positive and the template parameter
  recorded in `docs/08-types.md`. Declaration-only code with no bodies exercises a different half of
  every check, and the stub set is the only such corpus that ships with the repo. A local edit is no
  longer silently reverted -- `third_party/php-prelude-upstream` holds the set exactly as published,
  `just stubs-diff` is the whole delta and a test pins its shape -- so fix what is worth fixing and
  record _why_ in the set's README, which is the half a diff cannot carry. Still report upstream:
  they fix the same defects (1.50.0 taught `preg_replace` to preserve array keys), and a fix taken
  there is one we stop carrying.

- **A sampler that reaches into generated output makes the probe it picks unfindable, and the
  timeout it then waits out is recorded as a measurement.** `compare-lsp` reported
  `phrost bootstrap 601.1s` against intelephense's 2.4 on a checkout where the perf sweep clocks
  phrost at 1.6-2.3 s the same afternoon. 601 is the 600-second warmup plus change: the tool takes
  its readiness probe from the _last_ file of the sample, the sample reached into
  `docroot/sites/default/files/php/twig/`, and the probe was a `__TwigTemplate_<hash>` class that no
  server indexes and phrost is right not to. Asked for it directly: forty attempts, zero hits. Two
  failures wearing one number. The sampler's own note already says "a tool's cache is not source"
  and lists `.phpstan` and friends -- a Drupal public files directory is the same thing wearing a
  different name, holding compiled Twig and aggregated assets. And a settle loop that gives up
  silently turns a bad probe into a plausible-looking latency, which is worse than hanging:
  `docs/comparison.md` is refreshed from this. `is_dependency_dir` now matches `sites/*/files` by
  _shape_, since `files` alone is a name any project may use for its own code. The general form is
  the one §9 already records about these three tools: **an instrument that samples a checkout has to
  agree with the server about what the project is** -- and "is" now covers output nobody wrote, not
  only code nobody owns.

- **An instrument that understates is as wrong as one that flatters, and both were found in one
  document.** `docs/comparison.md` sampled `.phpstan/cache` -- 11 272 of 25 147 non-vendor PHP files
  on the reference checkout, **45 % of the sample space**, drawn first because the stride runs over
  a sorted list and a dot sorts early. Those files are `<?php return [...];` blobs with no
  declaration, so every count was diluted and the outline row was dominated by them; the tell was
  arithmetic, 2.4 outline entries per file where a 339-line class yields 20. Separately,
  `capability-audit.py` declared `semanticTokens.tokenTypes: []` and reported **0/20 for a
  capability that returns 659 tokens** -- a server may send only the kinds a client says it can
  render, so the tool measured an absence it had asked for. A blind sampler is right about _source_
  and wrong about generated output; and a client stub has to declare what a real client declares, or
  it measures itself.

- **A request that arrives before bootstrap finishes is refused with `-32801`, and reading that as a
  null answer makes a working server look dead.** `tools/lsp-smoke.py` retries past it and says so
  in a comment; a probe written without that retry reported hover returning nothing for a class, a
  local, a service id and a route at once -- which is not a bug shape any single feature has, and is
  the tell. `Server.request` hands back the whole envelope, so the check is
  `r.get("error", {}).get("code")` before touching `result`.

- **`capability-audit` understated on ten independent counts, and every one was a rule it applied
  without something the server knows.** Run on the reference checkout it scored `documentHighlight`
  17/20, `documentFormatting` 19/20, `references` 19/20 and `typeDefinition` 19/20; run on a second
  project, `references` 10/20 and `documentFormatting` 9/13. **Every one of those was the tool.**
  After the fixes: thirteen capabilities at 20/20 answered and useful, and the only rows below are
  `codeAction` at a bare cursor -- whose provoked twin is 20/20, which this section already records
  -- and `implementation`, which has eight sites and gets all eight. The ten, because the shapes
  recur: a **variable's scope** -- `$app` written six times in a file is six _different_ variables
  in six methods, so highlighting one is the only correct answer for `FreezeAction::isEnabled` whose
  body is `return TRUE;`, and for an _interface_, which has no bodies at all. A **namespace
  segment** -- `rg -w AttachmentType` matches `Drupal\...\AttachmentType\...`, so every file
  importing from that namespace counted as referencing the class: 27 files where 6 is right. A
  **quoted word** -- `'AttachmentType'` as somebody else's array key. A **dependency directory** --
  `--glob '!vendor/**'` is not the rule, `lspclient.is_dependency_dir` is, and §9 already records
  that these tools had that blind spot. A **method name**, which bounds nothing at all: every class
  writes `__construct`. And three shapes of **data mangled as code** -- a heredoc body, a string
  literal holding `,` or `.`, and a single-quoted string that runs past its line, which is how that
  project writes SQL. A formatter cannot put back spaces the test took out of a string. Three more
  came out of running it on a _second_ project, which is the other half of the lesson -- one
  checkout's idioms do not exercise a scorer. A **hint that repeats its argument** is one phrost
  deliberately does not draw (`f($id)` gets no `id:`), so a file of eight `queryOne($sql, $params)`
  calls scored as a capability answering nothing. A word inside a **quoted path** --
  `require_once __DIR__ . '/DatabaseRepository.php'` -- has a `/` before it and a `.` after, so no
  lookaround reaches it and only reading the file does. And a **continuation line of a multi-line
  string** is data on every line it covers, escapes included: `("What was your first pet\'s name?")`
  inside single-quoted SQL closed the string four lines early and handed the rest back to the
  squeezer. The general form is the one §9 states for the samplers and is worth stating for the
  _scorers_ too: **an instrument that judges an answer has to know what the answer is about.** Most
  of them are one question -- is this occurrence of the word the same thing as that one -- which is
  precisely the question the server exists to answer, so a scorer that answers it with `rg` is
  grading the server against a worse version of itself. And a score that only ever falls short is
  indistinguishable from one that is measuring the wrong thing: four capabilities sat one or two
  short for long enough to read as a standing shortfall, and none of them was.

- **A usage message whose text is an example of _output_ reads as output, and repeating it three
  times made it look stable.** `warm-start-probe.py` takes a root **and the files to compare**; run
  with the root alone it exits with a line from its own docstring -- which is a sample finding,
  `hover 166:58: cold "..." / warm '(nothing)'`. That was read as a real `cold == warm` violation,
  chased through two controlled reproductions that both disagreed with it, and then _confirmed_ by
  running it three more times and getting the identical line. The identity was the tell and it was
  read backwards. A nondeterministic instrument that returns the same bytes on every run is
  returning a **constant**, not a stable measurement -- so repetition is evidence _against_ a
  finding there, not for it. Run properly the same probe answers `cold == warm over 3 file(s)`
  across 45 diagnostics and 118 literal hovers. Same family as the `sed` note below and the
  `rg -r`/`rg -e` pair above: three of the four instrument notes in this section are about reading a
  tool's own text as data.

- **`sed -n '/Actual/,/^----/p'` prints nothing, because the first `----` is the _opening_
  delimiter.** expect-test writes `Actual:` then `----` then the content then `----`, so that range
  ends where it starts. Cost an hour of bisecting a fixture that was correct the whole time, with
  each step "confirming" that one added statement silenced every diagnostic in the file. Use
  `awk '/Actual/{f=1} f'`, or read the file. A misread instrument is indistinguishable from a bug
  until the instrument is checked, so when a bisection keeps saying something impossible, suspect
  the reader before the ninth hypothesis.

- **`git checkout <file>` to undo a temporary sabotage throws away the work beside it.** The memory
  note says not to and I did it anyway: a `git diff > /tmp/x.patch` taken _before_ the sabotage,
  then `git checkout` and `git apply`, silently restored nothing when the apply failed against a
  `cargo fmt` that had run in between. The tell was a measurement that came back _better_ than the
  unmodified baseline -- 0.38 ms where the change had measured 1.79 ms -- which is not a thing an
  A/B can produce and is always the instrument. To stub something out for one measurement, edit it
  back by hand.

- **Normalising half of a presentational difference is worse than normalising none of it.**
  `compare-lsp` compared completion labels after stripping a trailing `()` and a leading `$`, and
  every method the other server labelled with its signature -- `t($string, $args = ...)` against our
  `t` -- counted as one only they offer. Reported gap: **968 distinct labels**. A direct probe of
  one real site returned **nine items from each server, the same nine**, seven scored against us.
  The honest figure is 201, and what it is mostly made of is magic methods -- **which this note
  called a position, and which were a gap.** The argument was that PHP calls them rather than a
  person; the counts refute it for four. `->__toString()` is written by hand 51 times in Drupal core
  and 59 in Symfony, `->__invoke()` 33 and 18, `->__construct()` and `->__serialize()` seventeen and
  fifteen. They are offered now and ranked last, which is what the crowding complaint behind the old
  rule actually asked for. Worth keeping as written _and_ corrected: a normalisation bug and a
  position taken on a user's behalf can hide in the same number, and only one of them was the
  instrument's fault. Half-normalisation is worse than none because none is visibly unusable, and a
  number that looks plausible gets ranked and worked from.

- **A constant changed while measuring can ship, and the doc-comment beside it is not a test.**
  `MOST_WORKERS` went 4 -> 1 in a commit about the cache writer, whose author had set it to isolate
  one worker's memory and not set it back; the comment above it still argued for four, and every
  start for two commits ran the serial path -- a cold server ready in 5.9 s where 2.4 s was the
  measured figure. Nothing failed. When a commit's measurement was taken under a temporary setting,
  `git diff` the constants before writing the message.

- **`rg -r` is _replace_, not recursive.** `rg -rn <pattern>` rewrites every match to the
  replacement before printing, so a search for `a_class_exists_guard_does_not_widen` reported the
  function name with the matched half cut out -- and the conclusion was that a python edit had
  mangled a test. rg is recursive by default; `-r` is the one letter that makes it lie about what is
  in the file.

  **Written down and hit three more times in one session**, which is worth more than the note: once
  reading an enum whose variants came back as `pub n {`, once searching for a class that came back
  as `class n extends`, and once verifying a dead-code finding. Reading a note does not stop a
  habit, so the habit has to change: `rg` needs no flag to recurse, so any `-r` in a search is the
  bug. Its neighbour is the same shape and cost the same session an hour -- a substring search said
  a reported-dead method had four callers, and they were `attachFileToOrder` and
  `attachFileCreatesOrderUploadRecord`. When a grep contradicts an index, the grep is the thing that
  has not been checked.

- **A binary's mtime says nothing about which dependency set built it, because cargo hardlinks it
  from a cached artifact.** Reverting a `Cargo.toml` pin and rebuilding printed `Finished in 0.27s`
  with no `Compiling` line and left `target/release/phrost` carrying the _older_ version's timestamp
  -- which reads exactly like the stale-binary trap below and was not one: cargo still had both
  dependency sets' artifacts in `target/` and simply relinked the right one. Deleting the binary and
  rebuilding changed nothing, because the hardlink came back with the same mtime. Neither mtime nor
  a `du`-style check can settle it. What can is **compiling from source**: run the one test whose
  answer differs between the versions, since `cargo test` cannot hand back an artifact that does not
  match the tree. The measurement being checked -- two reprint trees to diff -- was valid the whole
  time, and three commands went into doubting it.

- **Restoring a sabotaged file by `mv`/`cp` from a backup makes cargo reuse the sabotaged build.** A
  backup keeps its _original_ mtime, so putting it back hands cargo a file older than the artifact
  and nothing rebuilds: the A/B's "after" is really the "during". Measured as a fix that had just
  been shown to work reading as broken, with the arm plainly present in the source. Same family as
  the `git checkout` note already here, and cheaper to hit -- edit the text back by hand, or `touch`
  the file. And when comparing a corpus score against one written down, check the **denominator**:
  343/364 is not a regression from a 345 recorded against 360 fixtures.

- **A ratchet reading is only as good as the binary that produced it, and "I rebuilt" is not the
  same as "I rebuilt after the last edit".** A keyed-array-literal change was measured at WordPress
  3 946, committed on that number, and read 3 950 an hour later with the source reverted to exactly
  what had been committed -- cold, cache cleared, twice. The four extra were real false positives
  that had been in the commit all along; the 3 946 came from a build taken _before_ the last fix of
  that session, and every step after it was reasoned from a number that never described the code.
  The cost was not the four findings but the hour: two further changes were designed to remove them,
  one made them worse, and both were blamed on the wrong thing, because the baseline they were
  diffed against was fiction. Two habits follow. **Take the ratchet from a build you started after
  the final edit, in the same command**, not from one earlier in the session. And when a revert does
  not restore the number it is supposed to, the number is the suspect before the revert is --
  `touch` the files and rebuild with the output visible rather than filtered to `^error`, which
  hides whether anything compiled at all.

- **The ratchet existed, was documented as load-bearing, and had never been armed -- so every A/B
  this project ever did was rolled by hand.** `phrost check <p> --ratchet` and `just ratchet` have
  been there all along; what was missing is `phrost-baseline.toml`, which no checkout carried.
  Without it the fourth of §9's four instruments answers nothing, and the gap is silent: the flag
  reports "no baseline" and the habit becomes running the check twice and reading two totals. That
  is how a stale release binary came to be diffed against a number it never produced, and how a
  commit shipped four false positives that a one-command ratchet would have named. All five corpora
  are armed as of 2026-09-13. Arm a new one with `--accept`, which writes the file beside the
  project -- **beside, deliberately**: a baseline taken on one checkout says nothing about another,
  and it has to travel with the code it counts. After a change that is _meant_ to move a number,
  `--accept` again and read the diff; a code nothing agreed to fails the same way a rise does, which
  is what makes a new check a decision rather than an accident. Verified with a control, because an
  instrument that only ever says "held" is indistinguishable from one that is not running: a count
  lowered in the file exits 1 and names the code `ROSE`, a code deleted from it exits 1 and names it
  `NEW`. And read the exit code of **phrost**, not of the `tail` it was piped into -- that mistake
  read as "the ratchet does not fail" for a whole minute.

- **The reference project is somebody's working tree, not a fixed corpus, and a count that moved
  without a cause is that before it is a bug.** `unused-declaration` measured 131 at the top of a
  session and 123 an hour later, across a change that touched only a message string. Both
  stale-binary traps above were checked and neither applied; three runs agreed at 123, cold and
  warm. Seven findings had gone, and the seven were `LabelFormatter::formatDate`,
  `AddressMatcher::batchMatch` and five more -- **deleted by the user, because the check had
  reported them**. The methods are not called anywhere any more because they do not exist anywhere
  any more. So a baseline written down on Tuesday is not a baseline on Wednesday, and the
  confirmation is one command: `git -C <project> status` before concluding anything from a delta.
  **And there is a tell that settles it without the command.** Comparing a cold `check` against
  `check --warm` on that project read as twelve differences -- the `cold == warm` invariant broken,
  which is the one failure that would stop a release -- and among them was the _same finding at a
  different line_, `AccountResource.php:159` against `:154`. A five-line shift is not something any
  server bug produces; it is the file having been edited between the two runs. Re-taking the cold
  run in the same minute gave **0 differences**, and so did all four fixed corpora. A finding that
  moved _line_ rather than appearing or disappearing is the text changing, every time. The same
  property is what makes the project worth measuring against -- a corpus nobody edits cannot show a
  check being _acted on_ -- and the ratchet has to be re-taken rather than trusted. Related and
  cheaper to hit: a `rg -c ... | paste -sd+ | bc` pipeline prints an empty line for zero matches
  rather than `0`, so a name with no hits and a name whose search was malformed look identical.

- **Four framework corpora cannot price an application idiom, and one of them is where the user
  reads output.** The collector-and-bail shape -- `$errors = []`,
  `if ($x === NULL) { $errors[] = ...; }`, `if ($errors !== []) { return ...; }` -- appears
  **nowhere** in WordPress, Laravel, Symfony or Slim and three times in one method of the reference
  project, because a framework replaces it with constraint objects and an application writes it by
  hand. Priced by corpus frequency the narrowing that reads it is worthless; priced by what a person
  sees on their own project it is 3 of 111 findings and the one they wrote a comment in the code
  about. So when a reported false positive measures near zero on the corpora, check whether the
  corpora are the wrong instrument for that _kind_ of code before refusing the work -- and say which
  instrument the number came from. Two things found while measuring around it. **A whole entry point
  can be the second constructor of a rule**, and one was: an assignment checked against a
  `/** @var T */` reached the judgement through `fit_held_against_written` rather than
  `fit_written_value`, for the good reason that its expectation is an index into the file's own
  arena, and so was blind to _both_ written-value rules for a year. Closing it (`Expectation`, one
  rule reached by two spellings of the expectation) moved **no finding on five projects**, and that
  is the right answer rather than a dead rule -- every real `@var`-and-cast site in 35 000 files is
  a cast that _agrees_ with its annotation, which is what the idiom is for. When a rule is missing
  at one site, look for a second entry point before looking for a missing case. Closed the same way:
  `PathRoot::CallbackParameter` read "the first argument that _has_ an element type" as "which
  argument _is_ the container", so `array_map($cb, array_slice($timestamps, 0, 3), [500.00])` walked
  past a container that says nothing about its elements and typed the callback's first parameter
  `float`. Different questions, and `holds_elements` is the one that was wanted.

- **Typing something new makes the checks that were silent about it speak, and that is the feature
  working.** Reading a Drupal field's kind took `check` on the reference project from 0 findings to
  11, all `possibly-false-return`: `return reset($value);` out of a `: ?array` hands back `false`
  for an empty array, and the code guards with `$field->isEmpty()` above -- sound, and nothing
  connects a guard on the _list_ to `reset()` on what `getValue()` made from it. PHPStan reports the
  same eleven. Worth expecting rather than treating as a regression: a precision gain arrives as
  findings on code that has been running for years, and the ratchet will read it as a loss.

### 8.3 Agents, worktrees and the build

The hazards every server shares -- a patch path per agent, the one `git stash` every worktree writes
to, `UPDATE_EXPECT` under nextest and inside a worktree, one cargo command at a time -- are in
lspkit's `docs/working.md`, imported in §2. These are phrost's own.

- **Pin mago with `=` and commit `Cargo.lock`.** phrost-old used `mago-*="1.26.0"` with an ignored
  lockfile; a fresh checkout floated to 1.29 and broke typed class constants. phrost is a binary —
  the lockfile belongs in git.

- **A prose mention of `phrost_framework::` below `crates/framework/` fails `one_project`.** That
  check is a text scan for the literal, so a doc-comment cross-reference trips it exactly as a `use`
  would, and the failure reads as an architecture violation. Name the seam in prose, not by path.

- **`tests::scaling::re_indexing_one_file_does_not_get_slower_as_the_project_grows` is a wall-clock
  ratio and is not load-immune, whatever its own docs say.** It compares two probes seconds apart
  and assumes machine load is steady between them, which it is not while other agents build. Three
  agents lost time to it. Re-run it alone before believing it, then alone on the reverted tree.

### 8.4 Fixtures, probes and snapshots

§4 requires a snapshot at the layer that owns the behaviour. These are the ways a fixture can be
green and prove nothing — three of them nearly shipped as coverage in a single session.

- **An incremental fixture's `Step::write` path takes no leading slash.** The fixture header writes
  `//- /src/Repo.php`, but `FixtureSpec` strips the slash, so a `write` step spelled `/src/Repo.php`
  does not edit that file -- it adds a _second_ one. Nothing fails: the snapshot records both and
  the test stays green while proving nothing about invalidation, which is the one thing it exists to
  prove. Same root as `world.file_id("/app.php")` panicking; the header needs the slash and every
  lookup after it does not.

- **One incomplete statement per fixture.** A completion marker leaves a dangling `->`, which does
  not parse; mago recovers, but statements _after_ the break are dropped. Two markers in one file
  therefore test the first and silently lose the second, and the failure reads as the feature not
  working. Put the second in another method or another file. This has cost time twice.

- **A test whose fixture does not match its own name pins a gap instead of a rule.**
  `a_run_stops_where_the_tree_can_no_longer_tell_a_declaration_apart` was written with an `echo` and
  not a declaration, so it locked in the bug and read as coverage. The reporter who found the gap
  wrote all eight of their probes with `echo` and nearly concluded the whole check was dead; ours
  used assignments, which is why it survived. When a snapshot is accepted for a shape the test name
  does not mention, that is the moment to notice.

- **`check`/`check_framework` render the diagnostics of the fixture's _last_ file, so a negative
  control placed last pins its own emptiness.** The doc comment says so in one line and I did not
  read it. A test for "a binding on a trait reaches every class that uses it" was written with the
  subject second and the control third, `UPDATE_EXPECT` accepted `expect![""]`, the test passed, and
  it read as the feature working when the feature had not run at all. Third time this session an
  accepted-empty snapshot nearly shipped as coverage. Two rules that follow: **one subject per test,
  last in its fixture**, and an `expect![""]` that arrives from `UPDATE_EXPECT` is a result to
  _explain_ before accepting -- an empty expectation is the one snapshot that can be right for the
  wrong reason. What found it was an A/B over the two things the probe conflated (class-vs-trait,
  and which catalog), which is the note above about a probe whose two arms differ in two things --
  applied to my own probe this time.

- **Two probe files in one directory that declare the same class name make every probe lie.** Five
  scratch files each opening with `class Bundle {}` is five claimants on one name, and a catalog
  refuses to rank two -- so the class resolves to **nothing** and every arm of the matrix reads as a
  missing feature. The tell is a probe that worked in the first file and stops working when a second
  is added beside it, and the confirmation is one line: `phrost check <scratch dir>` reports
  `undefined-method` on the probe's own calls. Same root as `docs/08-php.md`'s note about Drupal's
  `core.api.php` teaching the annotation syntax it was being scanned for -- a duplicate claim is
  silence, not a warning. One case per directory, each with its own `composer.json`, and check the
  probe compiles clean before believing a single row of it.

- **A `@name@` fixture marker collides with a YAML container reference, and the format's own docs
  say it cannot.** They read: "PHP does use `@` -- error suppression, and docblock tags -- but both
  need a second `@` on the same line with a bare identifier between, which does not occur." True of
  PHP and false of `arguments: ['@mi@wired@ne.repo']`, which reads as a marker called `mi` and fails
  with "fixture has no marker `wired`". Past a dot it is safe, because the marker reader refuses a
  name holding one -- so put the marker after a dot of the id, or write the id with one. The general
  point is that a format's stated safety is scoped to the language it was designed against, and a
  fixture in a second language is outside it.

- **A snapshot whose rendered lines begin with padding fights `cargo fmt` forever.** expect-test
  infers how much indentation to strip from the accepted block; leading spaces in the _content_ make
  that ambiguous, so fmt re-indents, the test fails, `UPDATE_EXPECT` re-accepts, fmt re-indents. It
  looks like an unstable snapshot and it is a right-aligned column: `format!("{at:>4} ")` in the
  renderer. Render the first character of every line as content.

- **A fixture has no composer _map_, so a check that reads one is dead in every scratch project.**
  PSR-4 rules come from `vendor/composer/autoload_psr4.php` -- what composer **dumped** -- and not
  from a `composer.json`'s `autoload` block, so a hand-made project with a correct `composer.json`
  and no `vendor/` has _no rules at all_. A new check keyed on them therefore reports nothing there
  and reports correctly on the real checkout, which is the opposite of the usual direction and reads
  as the check being broken. Two consequences: state the rules in the harness the way a framework
  contract is stated (`World::set_namespace_rules`), and when a scratch project disagrees with the
  reference project, suspect the _setup_ before the code. Related and found in the same hour: a
  file's stored path is **relative to the project** and the autoload map's directories are absolute,
  so `path.starts_with(root)` answers no for every file -- silence again, and from a mismatch no
  type can catch.

- **A `check`-style CLI must mirror the server's setup** (install the autoload map, index stubs) or
  it over-reports phantom false positives and you will chase them for a day.

- **A harness that treats silence as failure scores a correct silence as a miss, and the fixture
  reads as a server bug.** `signature_help/no_result_outside_call` asked for _no_ signature help and
  failed for a year with "no signature help" -- `check_signature` returned that before reading the
  fixture's negative expectation, where `check_hover` reads its own first. `phrost query` at the
  same position said the answer was right, which is the tell: when the CLI and the suite disagree
  about one fixture, suspect the suite's reading of the _expectation_ before the server.

- **A test that lists a menu with no `only` filter pins every kind added later, and a fixture that
  gives two fixes one class makes one of them lie.** The doc-tag list tests asked for every kind, so
  `source.fixAll` landing on the menu broke them for saying something true; a list test asks for the
  kind it is about. And the first fix-all fixture had the unresolved class in a signature the
  docblock fix was documenting, so the `@return` was rendered against a name the import fix was
  about to change -- two fixes in one file where one changes what the other should have said. Not a
  bug in either, but a snapshot that would have pinned a wrong tag as coverage; keep one problem per
  site. Related, and the reason the whole-file fix keeps only _preferred_ actions from offers that
  opt in (`Offer::unattended`): a fix that changes what the code does -- the nullsafe operator -- is
  a correct quick fix and the wrong thing for a save to apply unread.

## 9. Tooling

- `rg` instead of `grep`, `fd` instead of `find`.
- `just` is the task runner: `just test`, `just lint`, `just codegen`, `just deny`, `just pr-ready`.
- `cargo nextest run` for tests; `UPDATE_EXPECT=1` to accept snapshots.
- **One cargo command at a time.** A background `cargo` beside a foreground one races on the build
  lock and leaves `target/` with missing `.rmeta` files, which then needs `cargo clean` anyway. Run
  `cargo clean` when `target/` has grown and builds start taking minutes — a slow build is what
  turns a small task into a long one.
- Four instruments, four questions. The first needs a corpus this repo does not ship -- three
  sibling checkouts have been used and **none of them is on this machine now**, so `just corpus`
  answers by refusing rather than by scoring; the numbers it produced are in `docs/03-roadmap.md`
  and cannot be re-taken until one is checked out again. The other three run against projects that
  are present. `just corpus <dir>` (what somebody else's fixtures say), `phrost coverage <project>`
  (how much of real code the index understands), `phrost check <project> --ratchet` (did a change
  add a false positive), `just perf <project>` (did it cost anything). Measure, let the data rank
  the work, build the top, re-measure with all four.
- A sixth, for the question "are we worse than PHPStan":
  `tools/phpstan-diff.py <project> [--level N]` runs the project's own vendored phpstan and phrost
  over the same checkout and prints PHPStan's rules with what phrost says at the same sites, by
  check name rather than by count -- a count invites reading a coincidental `doc-missing-throws`
  overlap as coverage. Ask it at the level the project _uses_ and at the level above; do not ask it
  at 9, whose extra findings are one position about `mixed` rather than a set of rules. It refuses a
  run whose `errors` list is not empty, because a phpstan worker that dies on the default 128 MB
  reports zero findings over zero files and reads exactly like a clean project. Measured once, on
  the reference checkout: level 5 **0 against 0**, level 8 **2 804** of which 2 233 are docblock
  precision and 536 are about types, level 9 a further 2 777 that all say `mixed`. Of the 536,
  phrost's type checks catch none -- and about a fifth are suppressions this repo makes deliberately
  and re-verified against the installed core while measuring. In the other direction phrost reports
  two whole categories PHPStan has no rule for at any level: what a docblock fails to say about what
  a call can throw, and what a project declares and nothing reaches.
- An eighth, for the question the finding lists cannot answer -- "did any _answer_ move":
  `phrost coverage <project> --dump <file>` writes every member access's resolved class and
  null/false reach and every assignment's type, sorted, and `diff` over two builds' dumps shows each
  one that changed. Deterministic (237 956 lines on Laravel, byte-identical across runs). Use it for
  any change that could alter a type where no check speaks -- the finding-list A/B is blind there.
- A ninth, for the question every other instrument is a proxy for -- "would it have caught the bug":
  `tools/would-have-caught.py <project> <commit>` clones the checkout twice (APFS `cp -c`, so
  seconds and no disk), puts the commit's files at their state before and after it, and prints the
  findings the commit made go away. Run over a project's own fix commits, a fix phrost said nothing
  about is where the next check comes from -- the reference project's `label(): string` returning
  the `?? NULL` it wrote is how the written-null rule arrived. Clones rather than links, because the
  server's walks do not follow a symlink (a Drupal `web -> docroot` would index twice), and a linked
  `docroot/core` reads as a core with no entity types.
- A tenth, for "goto answers here -- does everything else":
  `tools/where-goto-answers-the-rest-do.py <project>` asks completion, hover and references at every
  word goto answers, strings and comments included, and groups the misses by what stands around the
  word. The other oracles read their sites off masked text, so this is the one that sees a callable
  pair, a catalog key or a `@param`; its first runs found the bugs in `docs/08-server.md`'s note
  about masked text.
- An eleventh, for "does the formatter change what the code _means_": `phrost reprint <dir> <out>`
  then
  `php tools/reprint-means-the-same.php <project vendoring nikic/php-parser> <out>/before <out>/after`.
  It parses both trees with PHP's own rules -- heredoc indentation, escapes -- drops positions and
  comments, normalises the respellings the formatter makes on purpose, and names every file that
  computes something else or no longer parses. Its first run over five corpora found five changes of
  meaning that `phpcs` could not see and `php -l` saw one of; `formatting-preserves-the-code.py`
  asks whether a reprint parses and settles, this asks the question format-on-save actually depends
  on. Run it over every corpus after any formatter change.
- A twelfth, for "does a buffer mid-edit still get an answer":
  `tools/mangled-buffers-answer-without- failing.py <project> [--files 40] [--seed N]` cuts, holes
  and half-types each sampled file and asks every capability at random positions, reporting any
  internal error and anything slower than 200 ms. Every other instrument asks about whole files, and
  a broken buffer is what an editor actually sends. Its first runs found inlay hints slicing past
  the end of a repaired parse and two `prepareRename` walks of half a second each -- none of which a
  whole-file tool could reach. Run several seeds after any change to lowering, a reader of ranges,
  or a walk over the class table.
- A thirteenth, for "how does phrost rank among PHP engines on questions nobody here chose":
  `python3 tools/typing-conformance.py <clone of zonuexe/php-typing-conformance>`. 250 files, each
  stating inline what a correct engine reports, stays quiet about and accepts, with the stored
  results of twelve engines (PHPStan, Psalm, Mago, phpantom, intelephense, ...) already in the
  clone. It installs `tools/typing-conformance/PhrostChecker.php` into the clone, runs the suite for
  phrost alone, and prints every engine's pass rate and phrost's failures with whether PHPStan and
  phpantom pass them -- a test both fail is usually a position, not a gap. First run 2026-10-11:
  86.3 %, between Mago (87.1) and Psalm (81.5), PHPStan at 93.6 and phpantom at 93.2.
- A fourteenth, for "would an editor show it, and does accepting it write it":
  `python3 tools/a-half-typed-name-is-shown-and-accepted.py <project> [--files 40] [--limit 600]`
  cuts every name a file writes in half, sends the buffer that way and asks completion at the cut.
  Three questions the code itself answers: whether the name is offered, whether a client keeps it --
  by the _word_ at the cursor (Zed, whose PHP counts `$` in one) and by the text the edit _replaces_
  (VS Code and what follows it) -- and whether applying the edit writes what was there. Every other
  completion tool asks with the file whole and reads labels, so none could see a list the server
  answered and the editor emptied: `$ap` sent as `filterText: "app"` hid every variable in Zed for
  three releases. Its first runs also found `use ($al` accepted as `use ($$alpha)`, and the MongoDB
  extension's `RuntimeException` offered ahead of the one the file imports. An in-process test
  cannot see any of this -- what a client does with an answer is about the client.
- A seventh, and the only one that cannot be argued with: **`php` itself is on this machine**
  (8.5.10). For any question about what the _language_ does -- what an error says, which write PHP
  refuses, whether a hook makes a property backed -- write four lines into the scratchpad and run
  them, before reading an RFC and long before writing a check. It settled the whole property-hook
  rule in two minutes and refuted the reading a careful pass over the RFC had produced; see
  `docs/08-php.md`. A check that claims a runtime error and has not been run against the runtime is
  a guess.

- A fifth, for the formatter only: `tools/drupal-style.sh <dir of user code>` reprints every file
  beside itself (`phrost reprint`), runs the project's own `phpcs` over both trees and reports the
  errors the reprint _introduced_, by sniff. A coding standard has an executable and cannot be
  argued with; the delta is the formatter's account, because a project's existing findings are not
  ours. Needs a phpcs with the Drupal standard -- a Drupal project vendors one at
  `vendor/bin/phpcs`, and `PHPCS=` points elsewhere.
- Commit your own work. One commit per coherent step, present-tense subject line describing the
  behavior change, not the files touched. Never commit red: `just pr-ready` (or the equivalent
  `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test`) must pass first. Opening a PR is
  still the human's call.

- **Three measurement tools called `vendor/` the whole of somebody else's code, which is the same
  blind spot the server had.** `SKIP_DIRS` in `compare-lsp.py`, `capability-audit.py` and
  `session-cost.py` skipped `vendor`, `node_modules` and the like -- and Drupal installs its core at
  `<docroot>/core` and its contributed modules at `<docroot>/modules/contrib`, composer-installed,
  gitignored and outside `vendor/` entirely. 11 256 files on one real checkout, every one of them a
  candidate the samplers drew from. What that costs is not noise, it is the wrong subject.
  `capability-audit` scored **references 0/20** -- it asked at three _core_ declarations, the
  project's own code writes none of those names, and answering nothing was correct. Driven at a core
  interface the project does use, the same request answers **136 references across 45 of its
  files**. And `compare-lsp` drew four of its seven remaining goto gaps from `docroot/core` and
  `docroot/modules/contrib`. `lspclient.is_dependency_dir` is the one rule now, mirroring
  `FrameworkProject::dependency_paths` and, for the docroot, the `lib/Drupal.php` marker
  `providers/drupal/layout.rs` probes for -- a directory named `core` is an ordinary name until that
  file is in it. Duplicated from the server rather than asked for, because a tool reaches it over
  stdio and no request answers "whose code is this". Confirmed after: the same audit over the same
  project reads **references 20/20** and every other capability 19-20/20, where `codeAction 5/20` is
  a cursor landing somewhere nothing is offered and its provoked twin answers 20/20. The general
  form is worth more than the fix: **an instrument that samples a checkout has to agree with the
  server about what the project _is_.** Both had the same wrong answer for the same reason, and the
  server's was fixed six hours earlier in the same session without anyone thinking to look at the
  tools.
