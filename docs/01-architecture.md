# Architecture

What the crate graph is, where the boundaries run, and why each one is where it is. Written against
the code as it stands — every number here was measured, and `just` re-measures all of them.

The one rule everything is judged by is in `CLAUDE.md`: **fast and resource-cheap**. A change that
adds hot-path latency, or that makes memory grow with `vendor/` size, is wrong by default however
correct the feature is.

---

## 1. The crate graph

Dependencies point downward. Nothing reaches back up, and the matrix is a test — see
`crates/tools/checks/src/dependency_rules.rs`.

```
phrost-cli            the checker, the perf sweep, the coverage sweep
phrost-lsp            protocol only: dispatch, version guards, cancellation, scheduling
phrost-analysis       features — goto, hover, completion, diagnostics, actions
phrost-view           the ONLY index API a feature may see; read-only projections
phrost-index          the two-tier store, the disk cache, framework contracts
phrost-ty  phrost-sema  phrost-defmap    types / member tables / FQN -> DefId
phrost-body           what a body says: paths, guards, calls, occurrences
phrost-hir            ====== BOUNDARY: no syntax exists above this line ======
phrost-lower          the only crate that knows the shape of the mago tree
phrost-syntax         the only crate that knows mago exists
phrost-twig           the only crate that knows Twig's syntax; a leaf, read by the view
phrost-framework      typed contributions; the only place a framework is named
phrost-vfs  phrost-project  phrost-stubs
phrost-model          ids, Name, TextRange, Visibility — and nothing else
phrost-intern  phrost-testkit  phrost-harness
```

Below all of it, outside this repository: **lspkit**, the base shared with orthra, a git dependency
pinned by `rev`. `lspkit-arena`, `lspkit-memsize` and `lspkit-heap` were `phrost-arena`,
`phrost-memsize` and `phrost-heap`. `lspkit-runtime` is the panic containment, write poisoning and
buffered transport `phrost-lsp` wraps the server in. `lspkit-arch` is the engine the
`dependency_rules` and `no_fallbacks` checks run on; the rules they enforce are phrost's and stay in
`crates/tools/checks`. lspkit knows nothing about PHP, and `dependency_rules` polices which layer
takes which of its crates.

`phrost-syntax-features` is the one legal exception: syntax-level features that genuinely need
tokens and trivia — semantic tokens, folding, formatting, the mago linter wrapper. It may depend on
`phrost-syntax` and may **not** depend on `phrost-view` or above.

### Sizes, so the shape is checkable

Roughly 85 000 lines of Rust. Code and tests split like this:

| crate                    |   code |  tests |
| ------------------------ | -----: | -----: |
| `phrost-view`            |  8 333 |  2 136 |
| `phrost-body`            |  4 977 |  2 327 |
| `phrost-analysis`        |  4 974 | 18 439 |
| `phrost-lower`           |  4 719 |  2 215 |
| `phrost-index`           |  4 143 |  2 249 |
| `phrost-lsp`             |  3 796 |  2 707 |
| `phrost-hir`             |  3 288 |      — |
| `phrost-syntax-features` |  2 362 |  2 293 |
| `phrost-framework`       |  2 344 |      — |
| everything else          | ~7 000 | ~2 500 |

The arena, memsize and heap crates, and the server's panic containment and transport, about 1 600
lines between them, moved to lspkit and are counted there.

The number to keep watching is `phrost-analysis`: the previous attempt died with a 28 520-line one
that had grown PHP semantics of its own. Ours is 4 974 lines of code against 18 439 of tests, which
is the shape a feature crate should have — a feature is a projection consumed and a DTO produced,
and everything hard happens below it.

`phrost-view` is the crate to watch next. It is the largest by code because it is the whole index
API, and four of its files are past the 600-line rule in `CLAUDE.md` §7: `resolve.rs` (1 501),
`locals.rs` (1 408), `symbols.rs` (1 092), and `lower/docblock.rs` (1 436) beside it. Splitting them
by responsibility is overdue.

---

## 2. The parser boundary

### Two crates know mago exists

`phrost-syntax` wraps the parser and the lexer. `phrost-lower` walks the tree and produces
`FileHir`. Nothing else names a mago type, and `dependency_rules` enforces it.

This is not tidiness. mago 1.45 moved `ast` → `cst` and made the tree arena-bound
(`Program<'arena>`); the previous attempt pinned `=1.26.0` with an ignored lockfile, floated to 1.29
on a fresh checkout, and would have needed a rewrite to upgrade. The boundary is what makes that a
two-crate job.

### The arena dies immediately

Parse into a per-file bump arena → lower to an owned `FileHir` → **drop the arena**. `FileHir` has
no lifetime parameter. Nothing above `phrost-lower` can hold a parse tree even by accident, which is
what keeps memory bounded by what is _stored_ rather than by what was read.

### The docblock is a second language

PHP's type language and PHPDoc's are not the same, and mago parses both. `phrost-lower/docblock.rs`
is the only place the second one is read, and it lowers into the same `TypeRefData` a native hint
produces — so nothing above has two type languages to consume.

What a docblock says beyond a type is a **contract** (`ContractData`): a conditional return, a
template bound to an argument, an `@assert` about a receiver. Those are read at lowering because
that is the only place the docblock's own text is available, and they are answered at a call site
because that is the only place the argument is.

---

## 3. The index boundary: `phrost-view`

A feature sees `IndexView` and nothing else. It cannot reach a `FileSkeleton`, a `MemberTable`, or a
`TypeTable`, and it has no way to parse anything.

A view is built per request and dropped when the request is answered. That is what makes its memos
safe: flattening a member table is pure, caching it is safe at any lifetime, but the _invalidation_
is not — and a cache that outlives an edit is the bug class that made the previous attempt feel
unpredictable. A view cannot survive an edit, so it cannot go stale.

Four memos live there, and each earns its place with a measurement:

| memo           | why                                                                              |
| -------------- | -------------------------------------------------------------------------------- |
| `flattened`    | one pass asks about `$this` hundreds of times; each ask re-walked the hierarchy  |
| `declarations` | finding a class's declaration is a scan of the file's classes, asked per `new`   |
| `resolved`     | two whole-file projections resolve the same expressions; 6.2 → 8.7 ms without it |
| `accesses`     | four diagnostics ask for every member access in the file; 12.4 → 10.3 ms p95     |

### Projections are the unit of cost

This is the thing to know before adding a check or a feature. A view exposes **whole-file
projections** — `member_accesses`, `call_sites`, `name_references`, `locals_never_read`,
`documented_declarations`, and a dozen more. A diagnostics pass runs fifteen of them.

The cost of that pass is **the number of distinct projections, not the number of checks**. Four
checks share `member_accesses` and now pay for it once. A new check that reuses an existing
projection is nearly free; one that adds a whole-file walk is not, and has to say so in its commit
with a measured delta.

---

## 4. The two-tier store

### Tier A — `ProjectSkeleton`

Every file in the project, declarations only. Classes, members, functions, constants, imports,
namespaces, and the types they declare. No bodies, no expressions **retained**.

It answers every cross-file question: what does this FQN resolve to, what members does this class
have, what does this method return. It is what the disk cache stores, and it is why RSS is bounded
by _user-code declarations_ rather than by `vendor/` size.

**A declaration that is not at a file's top level is still a declaration.** Two shapes, reached two
different ways, and the difference is what decides who pays for them.

`new class { ... }` is an _expression_, so only lowering the body finds it -- and it exists nowhere
else, so a declaration-only lowering leaves the index with no facts about it at all: `$this` inside
its methods names a class the catalog has never heard of, and every member reached through it is
unresolvable. So ingest lowers bodies **for `SourceOrigin::User` only**, harvests the declarations
it finds, and drops the bodies with the HIR before the function returns. Vendor and stubs are
untouched by this, which is what keeps the memory rule intact -- an anonymous class in `vendor/` can
only ever be named from inside its own file, and nobody puts a cursor there.

`if (!function_exists('dump')) { function dump(..) {} }` and
`function outer() { function inner() {} }` are _statements_, so the declaration walk reaches both by
descending through control flow and through function-like bodies -- no body IR, no origin
restriction, and symfony's `dump()` is in `vendor/` where the first rule would never have looked.
What that walk records beside them is that PHP reaches them only when the code around them runs,
because a guarded declaration means _only if nothing else declared this_: `ProjectSkeleton::rank`
puts one below every unguarded declarer, the bundled stubs included, or 113 polyfills on one real
checkout would replace PHP's own signatures.

The cost, measured on a 30 596-file checkout: bootstrap +2.6 %, RSS +2 MB, both bounded by user
code.

### Tier B — `OpenSet`

Bodies, for open buffers only. Statements, expressions, guards, calls, occurrences. Everything a
position query needs and nothing a cross-file question does.

This is the constraint that decides what is answerable. "What does this method return when its
declaration says nothing" needs its body — which does not exist for a closed file — so it is not a
feature we have, and adding it would mean either keeping every body (memory) or answering
differently for open and closed files (`open == closed`).

The one case that survives is an **arrow function**: its body _is_ one expression, already lowered,
in the file being read. Nothing to walk, no branch to choose. See `callback_held`.

### Configuration, which is neither tier

Framework contracts, autoload source roots, the house style. Installed once by whoever read
composer's map and the framework providers, and deliberately _not_ carried by
`ProjectSkeleton::adopt` — a loaded index claimed keys into its own table, and the contracts that
say which attribute is a key are configuration and live here.

### Vendor

Indexed as declarations like everything else, never body-lowered, never eagerly analysed. The one
deliberate exception is composer's **classmap**, loaded whole because non-PSR-4 classes like
Drupal's root `\Drupal` are otherwise unresolvable — about 1–2 MB on a Drupal checkout.

---

## 5. The framework seam

The previous attempt let Drupal knowledge leak into generic features, and the consequence was that
Laravel could not be added without editing them. Here a framework is one trait with defaulted
methods, and `crates/framework/src/providers/` is the only place any framework is named.

```rust
pub trait FrameworkProvider: Sync {
    fn id(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool;

    // Everything below is defaulted, so a provider says only what it knows.
    // What is read from the project's files comes through one object, made once per root, so
    // that four questions cost one walk of the tree.
    fn project(&self, root: &Path) -> Box<dyn FrameworkProject>;
    fn file_extensions(&self) -> &'static [&'static str];
    fn non_null_returns(&self) -> &'static [NonNullReturn];
    fn argument_bindings(&self) -> &'static [ArgumentBinding];
    fn forwarded_arguments(&self) -> &'static [ForwardedArgument];
    fn attribute_keys(&self) -> &'static [AttributeKey];
    fn closed_catalogs(&self) -> &'static [&'static str];
    fn documented_elsewhere(&self) -> &'static [DocumentedBy];
    fn dispatched_names(&self) -> &'static [DispatchedName];
    fn format_style(&self) -> Option<&'static str>;
}

pub trait FrameworkProject {
    fn autoload_rules(&self) -> Vec<AutoloadRule>;
    fn procedural_sources(&self) -> Vec<PathBuf>;
    fn catalog_places(&self) -> Vec<CatalogPlace>;
    fn catalog_entries(&self) -> Vec<CatalogEntry>;
}
```

Three rules make it hold:

1. **What a provider returns is data, not behaviour.** A `&'static` slice of facts about the
   framework's own source, or a catalog built from files at bootstrap. Never a callback into a
   feature.
2. **Catalogs are part of the index, not a filesystem scan per request.** Built at bootstrap and on
   `didChangeWatchedFiles`. No catalog entry means a deterministic _unknown_, the same way every
   time — which is root cause #1 of the previous attempt's instability, removed.
3. **A provider cannot reach a feature.** `phrost-framework` depends on `phrost-model` and nothing
   above it, so a framework cannot help itself by editing a feature.

The acceptance test: adding Symfony should be a new directory under `providers/` plus fixtures, and
one line in `providers/mod.rs`. Nothing under `analysis/`, `view/`, `sema/` or `ty/` should change.

`documented_elsewhere` is the clearest example of the seam working, and of what happens when the
same fact is read twice. A documentation rule demanding `@param` reported Drupal's own standard as a
defect 310 times, because `Implements hook_form_alter().` _is_ the whole docblock the standard asks
for. The generic rule stayed generic; Drupal answered one string.

Then the sentence turned out to be a _cross-reference_, and the index already held what it points
at. So the provider now answers a `DocumentedBy` — the words the claim opens with, what the name it
makes starts with, and the catalog of names an implementation may be prefixed with — and three
generic capabilities read it: hover follows the reference and shows the hook's own documentation,
goto treats any name the convention marks as a target (`@see hook_theme()` included), and
`misnamed-implementation` reports a claim the framework's dispatch could never honour. Not one of
them names a framework, and a project with no provider pays an emptiness check.

`dispatched_names` is the same seam answering the half of that question the index could not. A
framework's own extension points are declared in files the index reads; a _project's_ own are
declared nowhere at all -- `$this->moduleHandler->invokeAll('notifications_alter')` is the entire
statement that the name exists. So the call is indexed as the declaration, which is the one catalog
here filled from a body rather than from a declaration, and it brings two constraints with it. Only
a project's own code contributes, because only its bodies are lowered -- and that is the right
boundary, since what a framework dispatches is documented and what a project dispatches is not. And
the _receiver_ has to be checked, or `alter` on any service in the project would declare an
extension point: the index records the receiver as the file spells it and `phrost-view` resolves it,
because resolving one at ingest would make the answer depend on the order files were read.

---

## 6. Naming

`*Id`, `*Ref`, `*Data`, `*Facts`, `*View`, `*Store`, `*Query`, `*Builder`, `*Signature`,
`*Resolution`, `*Catalog`, `*Contributor`, `*Site`, `*Scope`, `*Row`, `*Held`.

- `*Data` is what an arena stores. `*Row` is what the skeleton stores.
- `*Facts` is a read-only bundle handed to one computation.
- `*Site` is a place in a body: `CallSite`, `MemberSite`, `ArgumentSite`.
- `Held` is what the view believes a value holds — the one type the resolution layer speaks in.

Don't invent a new family unless nothing fits, and don't force a bad fit into an old one.

---

## 7. Determinism as an invariant

Every answer is a function of the index, and the index is a function of the files. There is no "try
the index, else re-parse / FS-scan / fuzzy-match". Seven such paths in the previous attempt were the
entire cause of "it works sometimes".

This is enforced by an allow-list rather than by a flag: `no_fallbacks` holds a file-by-file list of
every filesystem read with a reason for each, and a new read fails the test until somebody writes
down what it is for. Tests and the CLI are unpoliced — neither is on the path a request takes.

What makes the rule survivable is the **survivable-data rule**: a member's `definition_range` must
come from a structure that survives both compaction and a disk-cache restore. Snapshot byte offsets
do not — a closed or cached file has empty text and yields `0:0` garbage.
