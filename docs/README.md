# phrost — design documents

How phrost is built, how it is tested, and why. phrost is a clean-slate rewrite: an earlier attempt
(`phrost-old`, 141k lines in 9 crates, not published) is where most of the rules below were paid
for, and it is named here only where a rule says what it prevents.

Read in order:

1. **[01-architecture.md](01-architecture.md)** — the crate graph, the parser boundary, the index
   boundary, the two-tier store, the framework seam, and why projections rather than checks are the
   unit of cost.
2. **[02-testing.md](02-testing.md)** — eleven levels. The most important document.
3. **[03-roadmap.md](03-roadmap.md)** — where this stands, measured; what is next, ranked by
   evidence; what is refused and why.

**[comparison.md](comparison.md)** is not one of the three. It is a measurement, refreshed by
`just compare`, and it reports what other PHP servers answer that this one does not.

The **`08-*` family** is not planning either — it is the record of bugs already paid for, which
outgrew `CLAUDE.md` §8 and is now split by area. Read the one covering what you are about to touch,
and add new notes there rather than to `CLAUDE.md`:

- **[08-types.md](08-types.md)** — the type table, `Held`, guards and narrowing, docblock tags and
  templates, the compatibility judgement, and the checks built on them.
- **[08-index.md](08-index.md)** — lowering, the def-map, catalogs and claims, discovery, the disk
  cache, and what a warm start does differently.
- **[08-server.md](08-server.md)** — the LSP runtime, bootstrap, client behaviour, and the
  feature-level bugs that follow from reading a position rather than a tree.
- **[08-performance.md](08-performance.md)** — where a cold start's time and a session's memory go,
  and which remedies were measured and refused.
- **[08-php.md](08-php.md)** — composer, Drupal, the stub set, docblock text, and the formatter
  against the project's own coding standard.

`../CLAUDE.md` is the operating manual: the performance contract, how to work in parallel, the hard
NEVERs, the crate map, and — in §8 — the shapes those bugs recur in, which is what you cannot know
to look up.

## The rules everything else follows from

1. **A boundary is the dependency graph, not the documentation.** `phrost-old` had a correct
   `ARCHITECTURE.md` saying `phrost-analysis` must not contain PHP semantics, and grew a 28 520-line
   `phrost-analysis` reaching into parser internals. A rule with no CI check does not hold, so the
   allowed edges are a test.

2. **Zero fallbacks.** The index either has an answer or returns a deterministic _not found_. No
   "try the index, else re-parse / FS-scan / fuzzy-match". Seven such paths in `phrost-old` were the
   entire cause of "it works sometimes". Enforced by a per-file allow-list of filesystem reads, each
   with a written reason.

3. **An invariant beats a test.** `open == closed`, `cold == warm`, `incremental == from-scratch`,
   `framework-agnostic` are dimensions every fixture runs through, so one new fixture buys several
   invariants.

4. **Measure, then build.** Four instruments — `just corpus`, `phrost coverage`,
   `phrost check --ratchet`, `just perf` — ask four different questions, and the top of a ranked
   list beats an idea about what matters.
